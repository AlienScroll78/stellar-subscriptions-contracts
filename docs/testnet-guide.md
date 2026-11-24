# Testnet guide

## Keepalive

Persistent entries on testnet are archived if nobody interacts with the
deployment for about 30 days. The workflow
[`.github/workflows/testnet-keepalive.yml`](../.github/workflows/testnet-keepalive.yml)
runs every Monday at 06:00 UTC (and on demand via "Run workflow") and executes
`scripts/verify-testnet.sh`, which exercises the live contracts.

If the script fails, the workflow opens a GitHub issue with the `gh` CLI
linking to the failed run. Restore archived entries or redeploy with
`DEMO=1 make deploy`, record the new ids in `DEPLOYMENTS.md`, then close the
issue.

The script reads `.deployments/testnet.env`, so the deployment ids must be
available to the runner (commit them or provide them in a setup step) for the
check to pass.
