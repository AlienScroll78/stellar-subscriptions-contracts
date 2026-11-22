# Contributing

Thanks for your interest in contributing! This document covers the basics.

## Getting started

1. Fork the repository and create a branch from `main`.
2. Make your change with tests where appropriate.
3. Open a pull request describing the change and linking the related issue.

## Pull requests

- Keep PRs focused on a single issue.
- Include a clear description of the change and how it was verified.
- Ensure CI passes before requesting review.

## WASM size budget

Contract WASM sizes are limited by `.github/wasm-size-budget.json`. If your
change increases a contract's size, follow the process in
[`docs/wasm-size-budget.md`](docs/wasm-size-budget.md): update the budget in the
same PR and include a justification comment. Budget increases above the warn
threshold require maintainer approval.
