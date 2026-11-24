# Sequence diagrams: subscribe, charge, cancel, batch_charge, merchant_end, subscribe_until

Who signs what, and which contracts the subscription contract calls, for
the core flows. Each diagram's notes mark the signature each step
needs: the subscriber's, the merchant's, or none (a contract calling
another contract needs no human signature — only the one signature on the
top-level call, which must already authorize any nested calls it makes).

See [docs/subscription.md](subscription.md) for the full rules; this page
only shows the call shape.

## `subscribe`

```mermaid
sequenceDiagram
    actor Subscriber
    participant Sub as Subscription contract
    participant Plan as Plan contract
    participant Token
    participant Reg as Registry contract

    Subscriber->>Sub: subscribe(subscriber, merchant, token, amount,<br/>interval, cap, plan_id)
    note right of Subscriber: signature: subscriber

    Sub->>Sub: validate params (SameParty, InvalidAmount,<br/>InvalidInterval, InvalidCap)

    opt plan_id != 0
        Sub->>Plan: get_plan(plan_id)
        note right of Sub: signature: none (view call)
        Plan-->>Sub: Plan { merchant, token, amount, interval, active }
        Sub->>Sub: check plan matches and is active<br/>(PlanNotFound / PlanInactive / PlanMismatch)
    end

    Sub->>Sub: write new Subscription (status = Active)

    Sub->>Token: approve(subscriber, spender = Sub, amount, live_until)
    note right of Sub: signature: subscriber<br/>(nested call; must be part of what<br/>the subscriber's wallet signed)
    Token-->>Sub: ok / ApprovalFailed

    opt registry linked
        Sub->>Reg: register_subscription(id, subscriber, merchant)
        note right of Sub: signature: none<br/>(subscription contract authorizes itself)
        Reg-->>Sub: ok / RegistryUpdateFailed
    end

    Sub-->>Subscriber: subscription_id
    Sub->>Sub: emit Subscribed
```

A failure at any `Sub->>Sub` validation, the plan check, the approval, or
(when linked) the registry call aborts the whole transaction — nothing is
written and no event is emitted.

## `charge`

```mermaid
sequenceDiagram
    actor Merchant
    participant Sub as Subscription contract
    participant Token
    participant Reg as Registry contract

    Merchant->>Sub: charge(merchant, subscription_id)
    note right of Merchant: signature: merchant

    Sub->>Sub: load subscription; caller is its merchant? (NotMerchant)
    Sub->>Sub: REVOCATION: status == Active?<br/>(else SubscriptionPaused / Cancelled / Exhausted)
    Sub->>Sub: expires_at check (SubscriptionExpired)
    Sub->>Sub: INTERVAL: now >= next_charge_ledger?<br/>(else IntervalNotElapsed)
    Sub->>Sub: CAP: total_charged + amount <= total_cap?<br/>(else Overflow / CapExceeded)
    Sub->>Sub: write total_charged, next_charge_ledger,<br/>status = Exhausted if cap reached exactly

    Sub->>Token: transfer_from(spender = Sub, subscriber, merchant, amount)
    note right of Sub: signature: none<br/>(subscription contract spends its own allowance)
    Token-->>Sub: ok / TransferFailed

    alt transfer failed
        Sub->>Sub: return TransferFailed — transaction rolls back,<br/>including the write above
    else transfer succeeded
        opt registry linked
            Sub->>Reg: update_subscription(id, total_charged, status)
            note right of Sub: signature: none
            Reg-->>Sub: ok / RegistryUpdateFailed (also rolls back the charge)
        end
        Sub->>Sub: emit Charged
    end
```

A rejected call (any check above `Sub->>Token`) never reaches the token, so
no funds move and nothing is written.

## `cancel`

```mermaid
sequenceDiagram
    actor Subscriber
    participant Sub as Subscription contract
    participant Token
    participant Reg as Registry contract

    Subscriber->>Sub: cancel(subscriber, subscription_id)
    note right of Subscriber: signature: subscriber

    Sub->>Sub: load subscription; caller owns it? (NotSubscriber)

    alt status is Cancelled
        Sub->>Sub: return SubscriptionCancelled — no state change
    else status is Exhausted
        Sub->>Sub: return SubscriptionExhausted — no state change
    else status is Active or Paused
        Sub->>Sub: write status = Cancelled
        note right of Sub: this write is what makes REVOCATION final —<br/>nothing below can block or undo it

        Sub->>Token: approve(subscriber, spender = Sub,<br/>new outstanding amount, live_until)
        note right of Sub: signature: subscriber<br/>(nested call; best-effort —<br/>failure does not block cancel)
        Token-->>Sub: ok / approval failed
        Sub->>Sub: allowance_updated = (approval ok?)

        opt registry linked
            Sub->>Reg: update_subscription(id, total_charged, Cancelled)
            note right of Sub: signature: none; best-effort —<br/>failure does not block cancel
            Reg-->>Sub: ok / registry call failed
        end
        Sub->>Sub: registry_updated = (registry call ok?)

        Sub->>Sub: emit Cancelled { total_charged, unused_cap,<br/>allowance_updated, registry_updated }
    end
```

Cancelling is the one call where a failed follow-up (the allowance sync or
the registry update) never fails the call itself — only the cancellation's
own validity (ownership, and not already `Cancelled` or `Exhausted`) can stop
it. Both outcomes are reported as booleans on the `Cancelled` event instead.

## `batch_charge`

```mermaid
sequenceDiagram
    actor Merchant
    participant Sub as Subscription contract
    participant Token
    participant Reg as Registry contract

    Merchant->>Sub: batch_charge(merchant, subscription_ids)
    note right of Merchant: signature: merchant

    Sub->>Sub: validate batch (EmptyBatch / BatchTooLarge)

    loop each subscription_id
        Sub->>Sub: load subscription; caller is its merchant? (NotMerchant)
        Sub->>Sub: REVOCATION: status == Active?<br/>(else SubscriptionPaused / Cancelled / Exhausted)
        Sub->>Sub: expires_at check (SubscriptionExpired)
        Sub->>Sub: INTERVAL: now >= next_charge_ledger?<br/>(else IntervalNotElapsed)
        Sub->>Sub: CAP: total_charged + amount <= total_cap?<br/>(else Overflow / CapExceeded)
        Sub->>Sub: write total_charged, next_charge_ledger,<br/>status = Exhausted if cap reached exactly

        Sub->>Token: transfer_from(spender = Sub, subscriber, merchant, amount)
        note right of Sub: signature: none<br/>(subscription contract spends its own allowance)
        Token-->>Sub: ok / TransferFailed

        alt transfer failed
            Sub->>Sub: return TransferFailed — whole batch rolls back,<br/>including every write above
        else transfer succeeded
            opt registry linked
                Sub->>Reg: update_subscription(id, total_charged, status)
                note right of Sub: signature: none
                Reg-->>Sub: ok / RegistryUpdateFailed (also rolls back the batch)
            end
            Sub->>Sub: emit Charged
        end
    end

    Sub-->>Merchant: charged_count
```

`batch_charge` runs the same per-subscription checks as `charge` in a single
transaction: if any subscription fails, the entire batch rolls back — no
funds move and no event is emitted for any of them.

## `merchant_end`

```mermaid
sequenceDiagram
    actor Merchant
    participant Sub as Subscription contract
    participant Reg as Registry contract

    Merchant->>Sub: merchant_end(merchant)
    note right of Merchant: signature: merchant

    Sub->>Sub: load merchant record; caller is the merchant? (NotMerchant)
    Sub->>Sub: write merchant status = Ended
    note right of Sub: this write is what makes REVOCATION final —<br/>nothing below can block or undo it

    opt registry linked
        Sub->>Reg: update_merchant(merchant, Ended)
        note right of Sub: signature: none; best-effort —<br/>failure does not block merchant_end
        Reg-->>Sub: ok / registry call failed
    end
    Sub->>Sub: registry_updated = (registry call ok?)

    Sub->>Sub: emit MerchantEnded { merchant, registry_updated }
```

Ending a merchant is final for REVOCATION: once `merchant_end` succeeds, no
new subscriptions can be created for that merchant and existing ones can no
longer be charged. Like `cancel`, a failed registry follow-up never fails the
call itself — the outcome is reported as a boolean on the `MerchantEnded`
event instead.

## `subscribe_until`

```mermaid
sequenceDiagram
    actor Subscriber
    participant Sub as Subscription contract
    participant Plan as Plan contract
    participant Token
    participant Reg as Registry contract

    Subscriber->>Sub: subscribe_until(subscriber, merchant, token, amount,<br/>interval, cap, plan_id, expires_at)
    note right of Subscriber: signature: subscriber

    Sub->>Sub: validate params (SameParty, InvalidAmount,<br/>InvalidInterval, InvalidCap, InvalidExpiry)

    opt plan_id != 0
        Sub->>Plan: get_plan(plan_id)
        note right of Sub: signature: none (view call)
        Plan-->>Sub: Plan { merchant, token, amount, interval, active }
        Sub->>Sub: check plan matches and is active<br/>(PlanNotFound / PlanInactive / PlanMismatch)
    end

    Sub->>Sub: write new Subscription (status = Active,<br/>expires_at = expires_at)

    Sub->>Token: approve(subscriber, spender = Sub, amount, live_until)
    note right of Sub: signature: subscriber<br/>(nested call; must be part of what<br/>the subscriber's wallet signed)
    Token-->>Sub: ok / ApprovalFailed

    opt registry linked
        Sub->>Reg: register_subscription(id, subscriber, merchant)
        note right of Sub: signature: none<br/>(subscription contract authorizes itself)
        Reg-->>Sub: ok / RegistryUpdateFailed
    end

    Sub-->>Subscriber: subscription_id
    Sub->>Sub: emit Subscribed
```

`subscribe_until` is `subscribe` with an explicit `expires_at`. The expiry is
stored on the subscription and enforced later in `charge`:

```mermaid
sequenceDiagram
    actor Merchant
    participant Sub as Subscription contract

    Merchant->>Sub: charge(merchant, subscription_id)
    note right of Merchant: signature: merchant

    Sub->>Sub: load subscription; caller is its merchant? (NotMerchant)
    Sub->>Sub: REVOCATION: status == Active?<br/>(else SubscriptionPaused / Cancelled / Exhausted)
    Sub->>Sub: EXPIRY: now < expires_at?<br/>(else SubscriptionExpired)
    note right of Sub: this is the check that makes<br/>subscribe_until's expiry binding —<br/>a charge at or after expires_at is rejected
    Sub->>Sub: INTERVAL / CAP checks, then transfer_from ...
```

Once `now >= expires_at`, every `charge` (and every entry in a
`batch_charge`) for that subscription is rejected with `SubscriptionExpired`
before any funds move.
