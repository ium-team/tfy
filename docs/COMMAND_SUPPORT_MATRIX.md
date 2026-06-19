# Command Support Matrix

This document is the human-readable companion to `docs/command-support-matrix.json`.
The JSON file is the claim-gate source of truth.

## Claim tiers

| Tier | Meaning |
|---|---|
| `mapped_not_implemented` | TFY has recorded the RTK-overlap command/filter as a target, but no TFY-owned strategy or fixture evidence exists yet. |
| `implemented_p0_not_parity_claimed` | TFY already implements a P0 command family with raw-first, redaction, no-negative, and fixture evidence, but it is not benchmark-manifest-coupled for public RTK comparison wording. |
| `fixture_verified` | Future tier: implementation has fixtures and route evidence but no benchmark-manifest parity row. |
| `parity_claim_eligible` | Future tier: support matrix evidence and benchmark manifest evidence both pass for that row. |

## Public wording rule

Use “RTK-overlap” or “RTK-informed coverage” unless a row has `parity_claim_eligible=true` and a valid `benchmark_manifest_id` in `docs/command-benchmark-manifest.json`.

## Current status

- Existing TFY P0 implemented families are marked `implemented_p0_not_parity_claimed`.
- A broad first wave of built-in TFY DSL filters is marked `fixture_verified` across system/dev, JS/TS, Python/Ruby/Go/JVM/Dotnet, and conservative cloud/infra families; none are benchmark-manifest-coupled for public comparison wording.
- Remaining RTK Rust modules and built-in filters stay `mapped_not_implemented` until TFY-owned strategies/filters and fixtures land.
- Human auto-wrapping is false by default for unknown, secret-heavy, possible-interactive, cloud/infra, or destructive-adjacent families until each row receives an explicit `human_auto_wrapped=true` decision and route test.
- User TOML command rules are local/custom extensions documented in `docs/COMMAND_RULES.md`; they do not create official support-matrix rows or RTK comparison eligibility by themselves.

Validate the matrix with:

```sh
node scripts/validate-command-support-matrix.js
```
