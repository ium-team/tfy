# AGENTS.md — TFY Repository Instructions

This file is the repo-root instruction surface for Codex and other AI coding agents. It applies to the whole repository.

## Product identity

TFY is a Rust-first token-saving middleware for AI-agent I/O boundaries. It should reduce model-visible tokens while preserving local, recoverable evidence.

Do not describe TFY as an RTK clone. RTK is a useful reference for repo discipline and command-output compression, but TFY has a broader gateway/MCP/runtime architecture.

## Non-negotiable invariants

1. **Plain text by default** — model-facing command output must not default to JSON envelopes.
2. **No negative savings** — only summarize when the model-visible result is smaller than redacted public raw output.
3. **Raw evidence first** — exact stdout/stderr bytes must be stored locally before reduction, redaction, suppression, or summarization.
4. **Truthful adapter claims** — MCP support means host-routed MCP tool/resource integration. Do not claim private Codex hook interception, provider prompt mutation, or universal shell interception unless implemented and tested.
5. **Preview-only output validation** — do not claim workspace apply authority until explicit authority/provenance gates and tests exist.

## Code ownership map

- `crates/tfy-core/` — reusable token-saving primitives, raw store, redaction, context/code compression, command feedback.
- `crates/tfy-runtime/` — runtime envelope, gateway, negotiation, and state contracts.
- `crates/tfy-cli/src/main.rs` — CLI declaration and dispatch only.
- `crates/tfy-cli/src/gateways.rs` — Tool/Context/Output gateway execution.
- `crates/tfy-cli/src/adapter.rs` — generic-shell adapter install/run/report behavior.
- `crates/tfy-cli/src/mcp.rs` — stdio MCP JSON-RPC server and Codex MCP setup snippet.
- `crates/tfy-cli/src/util.rs` — small CLI helper functions only.
- `docs/` — durable architecture, protocol, harness, and contributor documentation.
- `.github/` — GitHub issue, PR, and CI scaffolding.

## Branch policy

Use explicit branch prefixes:

- `feat/<slug>` — product feature.
- `fix/<slug>` — bug fix.
- `docs/<slug>` — documentation-only change.
- `refactor/<slug>` — structural change with no intended behavior change.
- `test/<slug>` — test-only change.
- `chore/<slug>` — maintenance, CI, release, dependency, or repo hygiene.
- `codex/<slug>` — AI-agent-authored branch when no human branch exists yet.

Rules:

- Never mix unrelated product work and repo-harness work in the same new branch unless the user explicitly asks for a combined PR.
- Do not rename or force-push shared branches unless explicitly requested.
- If continuing an existing PR branch, keep commits scoped and explain the added story in the PR body.

## Commit policy

Every commit must follow the Lore protocol used in this repo:

```text
<intent line: why the change was made, not what changed>

<optional concise body explaining constraints and approach>

Constraint: <external constraint that shaped the decision>
Rejected: <alternative considered> | <reason>
Confidence: <low|medium|high>
Scope-risk: <narrow|moderate|broad>
Directive: <future-facing warning>
Tested: <verification run>
Not-tested: <known gaps>
```

Commit rules:

- The first line states the reason, not a file list.
- Include `Rejected:` when a tempting alternative should not be retried.
- Include `Directive:` when future agents could accidentally break an invariant.
- Include `Tested:` with real commands.
- Include `Not-tested:` for live host, provider, or GitHub setting gaps.

## Required verification before claiming completion

Run the full local gate unless the change is docs-only and clearly does not affect code:

```bash
./scripts/verify.sh
```

Equivalent full gate:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

Useful focused checks:

```bash
cargo test -p tfy-cli --test tool_gateway
cargo test -p tfy-cli --test adapter_gateway
cargo test -p tfy-cli --test mcp_server
cargo run -q -p tfy-cli -- tool-gateway -- sh -c 'printf ok'
cargo run -q -p tfy-cli -- adapter capabilities
cargo run -q -p tfy-cli -- mcp capabilities
```

## Documentation placement

- Repo-wide agent instructions belong in this root `AGENTS.md`.
- Long-form explanation belongs in `docs/`.
- GitHub collaboration templates belong in `.github/`.
- Do not hide core contributor/agent rules only under `.codex/`.

## Safe edit rules

- Prefer preserving existing CLI contracts unless a test/spec says otherwise.
- Add regression tests before changing model-visible output behavior.
- Keep `main.rs` dispatch-focused.
- Keep transport-specific behavior in dedicated modules.
- Keep docs aligned with actual support claims.
