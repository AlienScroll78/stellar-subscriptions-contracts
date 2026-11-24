# Support

Thanks for using this project! Here's how to get help.

## Questions and Bugs

Please use [GitHub Issues](https://github.com/stellar-sub/stellar-subscriptions-contracts/issues) to:

- Ask questions about usage or configuration.
- Report bugs or unexpected behavior.
- Request features or improvements.

Before opening a new issue, please search existing issues to avoid duplicates. When reporting a bug, include as much detail as possible (steps to reproduce, expected vs. actual behavior, and relevant logs or versions).

## FAQ

### My merchant says charges are failing — what do I do?

**Symptoms:** The merchant's charge attempts fail with a `TransferFailed` error, even though the subscription is still active and the subscriber has funds.

**Cause:** Subscriptions rely on a pre-approved allowance that the subscriber grants to the contract. That allowance expires after a period of time. Once it has expired, the contract can no longer pull funds, so every charge attempt fails with `TransferFailed`.

**Fix:** The subscriber needs to call `refresh_allowance` to grant a fresh allowance. After that, the merchant's charges will succeed again.

**Nothing is lost:** A failed charge does not change any state — no funds move, no subscription state is modified, and no charge is recorded. Once the allowance is refreshed, the merchant can simply retry the charge.

## Security Vulnerabilities

**Do not open a public issue for security problems.**

If you discover a security vulnerability, please follow the responsible disclosure process described in [SECURITY.md](SECURITY.md). This helps protect users while a fix is prepared.

## Contributing

For information on how to contribute to this project, please read [CONTRIBUTING.md](CONTRIBUTING.md).