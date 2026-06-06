# Contributing to TFY

TFY is a Rust-first agent middleware project. Contributions should keep the runtime small, explicit, and honest about integration boundaries.

## Branches

Use descriptive branches:

- `codex/<short-topic>` for AI-agent authored work.
- `feat/<short-topic>` for human-authored features.
- `fix/<short-topic>` for bugs.
- `docs/<short-topic>` for documentation-only work.
- `chore/<short-topic>` for maintenance.

Keep unrelated work out of the same branch.

## Commit messages

Use the repository Lore protocol: the first line explains **why**, not just what changed, and trailers record constraints, rejected alternatives, confidence, risk, and tests when useful.

Minimal example:

```text
Keep model prompts free of transport JSON

Constraint: TFY must avoid negative token savings on tiny command output.
Rejected: Always returning JSON envelopes | increases model-visible tokens and confuses users.
Confidence: high
Scope-risk: narrow
Tested: cargo test -p tfy-cli --test tool_gateway
```

## Pull requests

Every PR should describe:

- what changed
- why it changed
- supported vs unsupported runtime claims
- validation commands
- user/developer impact

Prefer draft PRs while an adapter boundary or output contract is still being reviewed.

## Verification gate

Run before requesting review:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

If any command cannot run, document the reason and the next-best evidence.

## Architecture expectations

- `main.rs` should stay dispatch-focused.
- Transport-specific code belongs in a transport module such as `mcp.rs`.
- Adapter-specific install/report behavior belongs in `adapter.rs`.
- Reusable token-saving behavior belongs in `tfy-core`.
- Runtime envelope types belong in `tfy-runtime`.

## Claim discipline

Do not claim that TFY automatically intercepts a runtime unless the runtime adapter exists and has e2e tests. Current supported boundaries are CLI/generic-shell routing and MCP host routing.
