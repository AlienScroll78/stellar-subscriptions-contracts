# Contributing

Thanks for helping improve this project. This document covers code style, the
pull request process, and the workflow for extending the contracts.

## Code style

- Run `cargo fmt` and `cargo clippy --all-targets -- -D warnings` before pushing.
- Keep functions small and documented; public items need doc comments.
- Prefer checked arithmetic (`checked_add`, `checked_sub`, ...) over raw operators.
- Do not introduce new dependencies without discussion in an issue first.

## Pull request process

1. Open an issue describing the change (or pick an existing one).
2. Branch from `main` and keep commits focused.
3. Make sure `cargo test` passes for every affected contract.
4. Open the PR, link the issue, and describe the behaviour change.

## Adding a new entry point

Adding a contract entry point touches several layers. Work through this
checklist in order so nothing is missed.

1. **Add a `DataKey` variant.** In `src/types.rs`, add a variant to the
   `DataKey` enum for any new persistent storage the entry point needs. Keep
   the naming consistent with existing variants.
2. **Write storage functions.** In `src/storage.rs`, add the read/write helpers
   for the new key(s). Use the existing `bump_instance`/`bump_persistent`
   helpers so TTLs stay correct, and return `Result<_, Error>` where a value may
   be absent.
3. **Write the entry point.** In `src/lib.rs`, add the `pub fn` inside the
   `#[contractimpl]` block. Validate inputs, require auth where needed, update
   storage, and return `Result<_, Error>`.
4. **Add an event.** In `src/events.rs`, define a `#[contracttype]` event struct
   and publish it from the entry point after state changes succeed.
5. **Add error codes.** In `src/errors.rs`, add any new `Error` variants. Keep
   the numeric discriminants stable — never renumber existing variants.
6. **Write unit tests.** Add tests under `src/test.rs` covering the happy path,
   each new error branch, and auth failures. Every new `DataKey` variant must be
   handled in the `covers_all_variants` match so the compiler flags any variant
   that is added without a corresponding test.
7. **Update docs.** Document the entry point in the contract's module docs and
   in any user-facing docs (README, integration guides).

### Golden XDR tests

Any change to the contract's public interface (new entry point, changed
argument or return types, new event) requires a golden XDR test. These tests
snapshot the serialized invocation and assert it matches the committed
fixture, catching accidental ABI breaks. Regenerate the fixture only when the
interface change is intentional, and call it out in the PR description.

### `covers_all_variants`

The `covers_all_variants` match in the test module is exhaustive over
`DataKey`. When you add a variant, the build fails until you add a matching arm
and a test for it. This is intentional: it keeps storage coverage honest.
Thanks for helping. This repository holds money-moving contracts, so the bar
for changes to the subscription contract is high. Please read
[The invariants](#the-invariants) before opening a PR.

## Local setup

1. **Rust.** Install [rustup](https://rustup.rs). The pinned toolchain,
   the `wasm32v1-none` target, `rustfmt` and `clippy` are installed
   automatically from `rust-toolchain.toml` the first time you run `cargo`.
2. **Stellar CLI.** Needed to build release wasm and to deploy. Follow
   <https://developers.stellar.org/docs/tools/cli/install-cli>, then check:

   ```bash
   stellar --version
   ```

3. **Clone and test.**

   ```bash
   git clone https://github.com/stellar-sub/stellar-subscriptions-contracts
   cd stellar-subscriptions-contracts
   make test
   ```

## Build, test, deploy

| Task | Command |
|---|---|
| Run all tests | `make test` (`cargo test --workspace`) |
| Run one contract's tests | `cargo test -p subscription` (or `plan`, `registry`) |
| Run only the invariant tests | `cargo test -p subscription invariant_` |
| Run mutation testing on subscription contract code | `cargo mutants --file contracts/subscription/src/lib.rs` |
| Lint | `make clippy` |
| Format | `make fmt` |
| Build wasm (CLI) | `make build` |
| Build wasm (cargo only, as CI does) | `make wasm` |
| Markdown links and spelling | `make lint-docs` |
| Core CI checks | `make check` |

Install Node.js with npm and run `pipx install codespell==2.4.1` before
running `make lint-docs`. Add project vocabulary to
`.github/docs-words.txt` one word per line.

| Fuzz deeper than the default | `PROPTEST_CASES=500 cargo test -p subscription fuzz` |
| Prove the invariants on the live deployment | `make verify` (after `DEMO=1 make deploy`) |
| Deploy to Testnet | `make deploy` |

`make deploy` runs `scripts/deploy.sh`, which creates and funds a
`subs-deployer` identity if needed, deploys the plan, registry and
subscription contracts, initializes them and links them. Set
`NETWORK` / `IDENTITY` to override, and `DEMO=1` to also run a live
plan → subscribe → charge that proves a same-interval second charge is
rejected on-chain. Contract ids are written to `.deployments/<network>.env`;
copy them into `DEPLOYMENTS.md`.

Linking is one-shot: each contract accepts its links once and only before
its first plan or subscription. To re-link, deploy fresh instances.

## The invariants

A subscription is a promise from the subscriber: "you may take *this much*,
*this often*, *until I say stop*." Each part of that sentence is an invariant.

### CAP — never more than authorized

The total charged to a subscription can never exceed the `total_cap` the
subscriber agreed to. A charge that would go over is refused outright, not
partially filled.

**Why it matters:** the cap is the subscriber's worst-case loss. If a merchant
is compromised, buggy, or malicious, the cap is the most they can take. If CAP
can be broken, a subscriber has effectively handed over their whole balance.

### INTERVAL — never more often than authorized

At most one charge per billing interval. A second charge before
`next_charge_ledger` is refused even when it comes from the correct merchant,
and a merchant who charges late cannot then charge several times in a row to
catch up.

**Why it matters:** without it, a merchant could drain the entire cap in one
ledger. CAP bounds *how much*; INTERVAL bounds *how fast*, giving the
subscriber time to notice and cancel.

### REVOCATION — never after the subscriber says stop

Once the subscriber cancels, no charge succeeds again, ever. Cancelling is
final, only the subscriber can do it, and nothing — not the merchant, the
admin, the token or the registry — can block or undo it.

**Why it matters:** consent that cannot be withdrawn is not consent. This is
the subscriber's escape hatch when anything else goes wrong.

All three are enforced in `charge()` **before** any funds move, and each has
its own test file: `test/invariant_cap.rs`, `test/invariant_interval.rs`,
`test/invariant_revocation.rs`. `test/fuzz.rs` checks all three after every
step of random action sequences.

## Adding a feature without weakening the invariants

Work through this checklist for any change to `contracts/subscription`, and
say in the PR description how each point is satisfied.

1. **Write the invariant-facing tests first.** If your feature touches
   charging, status or amounts, add tests showing CAP, INTERVAL and REVOCATION
   still hold with it — ideally failing before your implementation exists.
   Add the feature's actions to `test/fuzz.rs` so they get interleaved with
   everything else.
2. **Never add a path that moves funds outside `charge()`.** `charge()` is the
   single place the three checks run. A second transfer path is a second
   place they must all be re-implemented correctly.
3. **Never add a way out of a final state.** Nothing may move a subscription
   out of `Cancelled` or `Exhausted`.
4. **Never let anything block `cancel`.** Follow-up work in `cancel` (allowance,
   registry, anything new) must be best-effort and report its outcome in the
   event rather than failing the call.
5. **Keep the auth split.** `charge` requires the merchant's signature;
   `cancel`, `pause`, `resume` and anything else that changes the
   subscriber's consent require the subscriber's. Admin functions must not
   touch subscriptions. Add a `mock_auths` test proving the wrong party's
   signature is rejected.
6. **Checked arithmetic, compared before transfer.** Any arithmetic on
   `total_charged`, caps or ledgers uses `checked_*` and returns
   `Error::Overflow`. Compare against `total_cap` before any transfer.
7. **No panics.** Return an `Error` variant; never `unwrap`, `expect`,
   `panic!` or `todo!` in contract code. Error codes are append-only.
8. **Emit an event** for any new state change.
9. **Keep signed nested calls stable across ledgers.** Wallets sign the
   authorization tree they saw when simulating, and the transaction lands a
   few ledgers later. Any argument of a nested call the subscriber signs
   (such as the token `approve`) must not be derived from the exact current
   ledger, or the call will pass simulation and fail on-chain. Add a case to
   `test/simulation_drift.rs`.

### Worked example: `refresh_allowance`

Token allowances expire, so long subscriptions need a way to renew theirs.
`refresh_allowance(subscriber, token)` was added like this:

- **Auth:** it changes what the subscriber has approved, so it requires the
  **subscriber's** signature (point 5).
- **No new fund path:** it only calls `approve`; funds still move only in
  `charge()` (point 2).
- **Cannot loosen CAP:** the approved amount is recomputed from contract state
  as the sum of remaining caps of the subscriber's non-final subscriptions.
  Even an allowance larger than that could not be spent past a subscription's
  cap, because `charge()` checks the cap itself (point 6).
- **Tests first:** a test zeroes the allowance, shows `charge` failing with
  `TransferFailed` and leaving state untouched, then shows `refresh_allowance`
  restoring charging on the normal schedule — and that a cancelled
  subscription contributes nothing to the refreshed amount (point 1).

## Proptest regression files

When `test/fuzz.rs` finds a counterexample it writes a regression file to
`contracts/subscription/src/test/regressions/`. That file is committed so
the failing case is re-run on every subsequent `cargo test`, preventing
silent regressions.

**Policy:**

- The `regressions/` directory is tracked in git. Do not add it to
  `.gitignore`.
- If `cargo test -p subscription fuzz` produces a new file under
  `regressions/`, commit it together with the fix in the same PR.
- If the counterexample reveals a genuine invariant violation, treat it
  as a security issue and follow [SECURITY.md](SECURITY.md).
- If the counterexample is a false positive (environment issue, test
  harness bug), open an issue, delete the file and explain why in the
  commit message.

To run the fuzz test and surface any failures:

```bash
cargo test -p subscription fuzz
# or, for a deeper search:
PROPTEST_CASES=500 cargo test -p subscription fuzz
```

## Code style

- `cargo fmt` and `cargo clippy -- -D warnings` must pass (CI enforces both).
- Match the surrounding code: module layout (`types`, `errors`, `storage`,
  `lib`), doc comments on public functions explaining *why*, and test names
  that read as sentences.
- Commits: [Conventional Commits](https://www.conventionalcommits.org)
  (`feat(subscription): ...`, `test(plan): ...`, `docs: ...`). One logical
  change per commit.

## Pull requests

- Keep PRs focused. Invariant-affecting changes should not be mixed with
  refactors.
- CI must be green: tests, clippy, fmt and the wasm build.
- Changes to `contracts/subscription` need a review that explicitly checks
  the checklist above.

## Sister repositories

- Web app: https://github.com/stellar-sub/stellar-subscriptions-web
- API + Docs: https://github.com/stellar-sub/stellar-subscriptions-api-docs

## Review of error codes and status discriminants

`contracts/*/src/errors.rs` (the `#[repr(u32)]` error codes) and
`contracts/*/src/types.rs` (such as the `SubStatus` discriminants) are part of
the public ABI. Clients, indexers and `docs/errors.json` depend on the exact
numeric values, so renumbering or reusing a value is a breaking change.
`.github/CODEOWNERS` therefore requests review from both
`@stellar-sub/maintainers` and `@stellar-sub/security` for these files. CODEOWNERS
only requests reviewers: to require two approvals, enable "Require review from
Code Owners" and at least 2 required approvals in the branch protection rules.
