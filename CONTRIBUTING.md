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
