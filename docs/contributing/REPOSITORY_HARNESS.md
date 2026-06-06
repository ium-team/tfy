# Repository Harness

This document explains the maintainability harness around TFY. It exists so future human and AI contributors can work with less rediscovery.

## Harness layers

1. **Code boundaries** — Rust modules are split by responsibility: core primitives, runtime contract, CLI dispatch, gateways, adapters, MCP transport.
2. **Agent harness** — `AGENTS.md` and `docs/AGENT_HARNESS.md` give AI agents the project map, invariants, and focused checks.
3. **Planning artifacts** — `.omx/plans/` stores PRDs/test specs for larger stories.
4. **GitHub templates** — `.github/` captures repeatable issue/PR expectations.
5. **Verification script** — `scripts/verify.sh` runs the local quality gate.

## Adding a new adapter

1. Document the boundary and unsupported claims.
2. Add capability reporting before install/run behavior.
3. Add tests that prove the host-visible claim.
4. Keep raw recovery and no-negative-savings behavior intact.
5. Update README and protocol docs.

## Adding a new command compressor

1. Add reusable logic to `tfy-core`.
2. Add focused unit tests for noisy, tiny, failing, secret-bearing, and binary-ish outputs.
3. Route CLI behavior through existing gateways.
4. Measure model-visible bytes against raw bytes.

## Release readiness

A release candidate should have:

- clean full verification gate
- updated README/docs
- PR checklist completed
- changelog or release note entry for user-visible behavior
- no untracked generated/raw artifacts
