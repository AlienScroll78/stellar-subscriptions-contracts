.PHONY: build wasm wasm-size test clippy fmt fmt-check lint-docs check clean deploy verify testnet-deploy testnet-integration

NETWORK ?= testnet
IDENTITY ?= subs-deployer
DEMO ?= 1

# Build all contracts to wasm with the Stellar CLI.
build:
	stellar contract build

# Build all contracts to wasm with cargo only (what CI runs).
wasm:
	cargo build --workspace --release --target wasm32v1-none

# Run every contract's test suite, including the invariant tests.
test:
	cargo test --workspace

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

# Check every contract's wasm against the size budgets in
# .github/wasm-size-budget.json. Run `make wasm` first.
# To update budgets after an intentional size change:
#   make wasm && scripts/check-wasm-size.sh --update
wasm-size:
	bash scripts/check-wasm-size.sh

# Check Markdown links and spelling (requires npm and codespell 2.4.1).
lint-docs:
	find docs -type f -name '*.md' -exec npm exec --yes --package=markdown-link-check@3.15.0 -- markdown-link-check --quiet {} +
	find docs -type f -name '*.md' -exec codespell --ignore-words .github/docs-words.txt {} +

# Core CI checks, in CI's order.
check: test clippy fmt-check wasm wasm-size lint-docs

clean:
	cargo clean

# Deploy, initialize and wire all three contracts.
deploy:
	NETWORK=$(NETWORK) IDENTITY=$(IDENTITY) ./scripts/deploy.sh

# Deploy, initialize, wire all three contracts on Testnet with sensible defaults
# (seeds the demo plan, identities and subscription so verification can run).
testnet-deploy:
	NETWORK=$(NETWORK) IDENTITY=$(IDENTITY) DEMO=$(DEMO) ./scripts/deploy.sh

# Prove CAP, INTERVAL and REVOCATION against the live deployment
# (run `DEMO=1 make deploy` first).
verify:
	NETWORK=$(NETWORK) ./scripts/verify-testnet.sh

# Load the shared Testnet environment (examples/env.sh) and run live invariant
# verification (scripts/verify-testnet.sh) against the deployed contracts.
testnet-integration:
	NETWORK=$(NETWORK) bash -c 'source ./examples/env.sh && ./scripts/verify-testnet.sh'
