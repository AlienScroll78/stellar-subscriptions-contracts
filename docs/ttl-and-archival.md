# State archival and TTL

Soroban does not keep storage entries alive forever. Every entry has a
time-to-live (TTL), measured in ledgers, and once it runs out the entry is
archived (evicted) and can no longer be read without being explicitly
restored. All three contracts (`registry`, `plan`, `subscription`) bump the
TTL of every entry they touch so that normal use keeps data alive, and this
document explains the constants involved and what happens if nobody touches
a subscription for a long time.

## Instance vs persistent entries

Each contract's `storage.rs` uses two kinds of entries, both bumped with the
same constants:

```rust
use common::DAY_IN_LEDGERS; // contracts/common: 17_280

const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;
```

(the subscription, registry and plan contracts all import `DAY_IN_LEDGERS`
from `contracts/common`, so the day length is defined once. The instance
constants are set per contract; the values above are the subscription
contract's, which is the one that must stay live between charges.)

- **Instance storage** holds data there is only one of per contract:
  `Admin`, `NextId`, linked contract addresses, `Writers`, `Stats`. It is
  bumped to live 30 days (`INSTANCE_BUMP_AMOUNT`) whenever
  `Admin`, `NextId`, linked contract addresses and `Writers`. It is
  `Admin`, `NextId`, linked contract addresses, `Writers`. It is
  bumped to live 7 days (`INSTANCE_BUMP_AMOUNT`) whenever
  `bump_instance(env)` runs, once its remaining TTL falls to
  `INSTANCE_LIFETIME_THRESHOLD` (29 days) or below. The 30-day bump is what
  lets a deployment with only monthly subscriptions go a full billing cycle
  without a contract call and still accept the next `charge`.
- **Persistent storage** holds per-id records that must survive
  independently of each other: a `Subscription`, a `Plan`, a
  `SubscriptionRecord`, and the index entries (`BySubscriber`, `ByMerchant`,
  `AllSubscriptions`, `AllPlans`, and the registry's `Stats`). Each is bumped to live 30 days
  `SubscriptionRecord`, the registry's aggregate `Stats` (moved here so a
  quiet registry does not lose its counters with the instance entry), and the index entries (`BySubscriber`, `ByMerchant`,
  `AllSubscriptions`, `AllPlans`). Each is bumped to live 30 days
  (`PERSISTENT_BUMP_AMOUNT`) via `bump_persistent(env, key)`, once its
  remaining TTL falls to `PERSISTENT_LIFETIME_THRESHOLD` (29 days) or below.

`DAY_IN_LEDGERS = 17_280` assumes a 5-second average ledger close time
(`86_400 / 5`).

## Why every read and write bumps TTLs

Every function that reads or writes a persistent entry calls
`bump_persistent` as part of that access:

- `read_sub` / `write_sub` (subscription), `read_plan` / `write_plan` (plan),
  `read_record` / `write_record` (registry) each bump the single entry they
  touch.
- `read_index` / `push_index` (subscription, plan) and `read_all_ids` /
  `push_id` (registry) bump the index entry they read or append to.

This means the TTL clock restarts on the *entry*, not on a per-subscription
basis across all its related entries. `charge` is the one entry point that
must bump more than its own record: it bumps `Sub(id)`, the subscriber's
`BySubscriber` index and the merchant's `ByMerchant` index (via
`touch_index`, which only extends an index that already exists), and the
registry's `Record(id)` when linked. Without that, a subscriber who is charged
for years but never calls the contract would let `BySubscriber` archive while
its records stay live, breaking `get_by_subscriber`, `cancel` and
`refresh_allowance`. Entry point functions that touch several of these keys
in one transaction bump all of the keys they go through.

### The `refresh_allowance` bump chain

`refresh_allowance` is the one entry point that deliberately walks *every*
subscription belonging to a subscriber, so it is worth tracing exactly which
keys get bumped:

1. `refresh_allowance` reads the subscriber's `BySubscriber` index
   (`read_index`), which bumps that index entry.
2. For each id in the index it calls `allowance::sync`.
3. `allowance::sync` calls `outstanding` to compute the accrued allowance.
4. `outstanding` calls `read_sub(id)` for that subscription.
5. `read_sub` calls `bump_persistent(env, &SubKey::Sub(id))` before
   returning the stored `Subscription`.

So a single `refresh_allowance` call extends the persistent TTL of the
`BySubscriber` index entry *and* of every `Sub(id)` entry listed in it - not
just the subscription whose allowance happened to change. This is the
intended behaviour: a subscriber refreshing their allowance is a sign of
activity for all of their subscriptions, and the refresh keeps the whole set
alive together.

The chain was traced and covered by a test asserting that after
`refresh_allowance` each `Sub(id)`'s persistent TTL is extended; no missing
`bump_persistent` call was found in the chain, so no code change was needed.
The only caveat is the one described above: subscriptions are only reached
through the `BySubscriber` index, so if that index entry itself has already
been archived, `refresh_allowance` cannot enumerate them and fails before any
`Sub(id)` is bumped.

Instance-level data is bumped the same way, via `bump_instance`, inside the
contracts' entry points that touch instance storage.

## What happens if nobody touches a subscription for a long time

If a `Subscription` entry's TTL reaches zero before anything reads or writes
it again, the entry is archived. From that point:

- Any call that would read it (`get_subscription`, `is_chargeable`,
  `remaining_cap`, `charge`, `cancel`, `pause`, `resume`,
  `refresh_allowance`) fails, because Soroban cannot return an archived
  entry's value without it first being restored.
- The subscription is not deleted and its data is not lost. It is simply
  inaccessible until restored.

Because `interval_ledgers` can be set far below 30 days, a subscription that
is being charged regularly keeps its own TTL extended far past the point
where archival would ever be reached. The scenario this section describes
applies to a subscription nobody has charged, cancelled, paused, resumed or
even queried for 30 days straight - which the contracts cannot prevent, since
they have no scheduler and only extend a TTL as a side effect of some
address calling in.

The `BySubscriber` / `ByMerchant` index entries for a subscriber or merchant
with no activity for 30 days are equally subject to archival, independently
of the `Sub(id)` entries they list.

## The registry's `AllSubscriptions` history

The registry keeps one persistent vec, `AllSubscriptions`, of every
registered subscription id. It is bumped only by `register_subscription`. If
no subscription is registered for the 30-day persistent window, the vec
archives and `get_all_subscriptions` returns an empty result. The next
registration then starts a new vec, so the earlier history is missing from the
registry's view until it is restored.

Mitigation: `keep_alive()` on the registry extends the TTL of
`AllSubscriptions` and the instance storage. It is permissionless and changes
no data. Call it at least every 20 days, for example from a scheduled job. The
test `history_survives_a_quiet_period_when_kept_alive` in
`contracts/registry/src/test.rs` covers a 60-day quiet period with that
cadence.

## Restoring an archived entry

Soroban allows an archived persistent entry to be restored, for a fee, as
long as it is within the network's maximum entry lifetime of its eviction.
Beyond that window it cannot be restored by anyone.

None of these contracts have a restore entry point, and they cannot add one
that works. Restoration is a ledger operation: it is performed in a
transaction before the contract is called, not from inside a contract call,
so a `restore_subscription(id)` function would have no way to bring back an
evicted `Sub(id)`. Recovery is done with the Stellar CLI.

### Procedure

1. Confirm the entry is archived. A read of an archived `Sub(id)` fails:

   ```bash
   stellar contract invoke --id $SUBSCRIPTION_CONTRACT --source merchant \
     -- get_subscription --subscription_id $SUB_ID
   ```

   An archived entry is reported as an error from the simulation, which
   names the missing or archived key.

2. Restore the persistent entry with `stellar contract restore`. Pass the
   contract id, the key, and the persistent durability:

   ```bash
   stellar contract restore --id $SUBSCRIPTION_CONTRACT \
     --durability persistent \
     --key '<KEY for Sub(SUB_ID)>'
   ```

   Replace `<KEY for Sub(SUB_ID)>` with the key of the subscription record.
   The key is the `DataKey::Sub(u64)` value, so it is written as an enum
   variant with the subscription id. Check the exact key encoding against
   `stellar contract restore --help` for your CLI version before running
   this, because the argument format for enum keys differs between releases.

3. Repeat the read from step 1. It should now return the subscription.

If the restore is refused, the entry is past the maximum restoration window
and cannot be recovered.

### Prevention

Charging, pausing, resuming and cancelling each extend the instance and the
touched entries, so a subscription that is used regularly does not archive.
To guard a deployment with long billing cycles, call a read-only entry point
such as `get_subscription` at least once a month for each subscription, or
run `charge` on each billing date.


## `BySubscriber` keepalive during `cancel`

`allowance::outstanding` reads `BySubscriber(subscriber)` through
`storage::read_index`, which extends that entry's TTL on every read. So
`cancel` (via `allowance::sync`) already bumps the index before computing the
allowance. On a live network an archived entry must be restored before the
transaction can touch it, so `read_index` does not silently see an empty list
for an archived index; an empty result only occurs for an index that never
existed. The scenario in issue #242 (allowance zeroed for other subscriptions)
was therefore not confirmed by reading the code, and no contract change was
made. `ttl_by_subscriber_keepalive.rs` tests that `cancel` extends the index TTL
and leaves the allowance for the remaining subscriptions intact. The residual
risk is a subscriber whose index has been archived and who does not restore it;
the existing monthly `refresh_allowance` guidance above covers it.
