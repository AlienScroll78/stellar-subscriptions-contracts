# Runbooks

Operational procedures for recovering from common failure modes in production.

## Registry misconfiguration

### Symptom

Every state-changing call on the subscription contract fails with `RegistryUpdateFailed`:

- `subscribe`
- `charge`
- `pause`
- `resume`

Read-only calls and `cancel` continue to work.

### What happens

The subscription contract stores the registry address in its contract state. That link is set once at deployment and cannot be changed afterwards. If the address is wrong (typo, wrong network, un-deployed contract, or a registry that does not implement the expected interface), every call that needs to consult or update the registry reverts with `RegistryUpdateFailed`.

Because the link is immutable:

- All state-changing calls that touch the registry fail: `subscribe`, `charge`, `pause`, and `resume`.
- `cancel` still works. It does not depend on the registry, so subscribers can always exit.
- There is no on-chain admin function to repoint the contract at a different registry.

### Recovery

The only recovery path is to redeploy and migrate:

1. **Deploy a fresh subscription contract** with the correct registry address. Verify the address against the intended network and confirm the registry contract is deployed and implements the expected interface before deploying.
2. **Smoke test the new instance** on the target network: run `subscribe`, then `charge`, `pause`, and `resume` against a test subscriber to confirm none of them revert with `RegistryUpdateFailed`.
3. **Announce the migration** to subscribers, including the new contract address and the cut-off for the old instance.
4. **Migrate subscribers** to the new instance. Subscribers must re-subscribe (and re-approve allowance) on the new contract; existing subscriptions on the misconfigured instance cannot be transferred on-chain.
5. **Wind down the old instance.** Subscribers can still call `cancel` on it to release any remaining allowance, so keep it reachable until the migration window closes.

### Prevention

- Double-check the registry address (and network) before deploying.
- Run the smoke test in step 2 as part of every deployment checklist.
