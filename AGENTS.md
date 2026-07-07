# AGENTS.md — TFY Repository Instructions

This file is the repo-root instruction surface for Codex and other AI coding agents. It applies to the whole repository.

## Product identity

TFY is a Rust-first token-saving middleware for AI-agent I/O boundaries. It should reduce model-visible tokens while preserving local, recoverable evidence.

## Non-negotiable invariants

1. **Plain text by default** — model-facing command output must not default to JSON envelopes.
2. **No negative savings** — only summarize when the model-visible result is smaller than redacted public raw output.
3. **Raw evidence first** — exact stdout/stderr bytes must be stored locally before reduction, redaction, suppression, or summarization.
4. **Truthful adapter claims** — MCP support means host-routed MCP tool/resource integration. Do not claim private Codex hook interception, provider prompt mutation, or universal shell interception unless implemented and tested.
5. **Preview-only output validation** — do not claim workspace apply authority until explicit authority/provenance gates and tests exist.

## Product routing rules

- Bare project-local `tfy start --agent` is the simple product path: it must prepare the generic TFY agent wrapper plus Codex and Claude Code official project hook routes by default; public product wording should name supported hosts only.
- Do not regress bare `tfy start --agent` back to wrapper-only behavior unless the product direction is explicitly changed with tests and docs.
- Do not implement that default by aliasing it to `--host all`; this would silently expand scope beyond the Codex/Claude supported named-host path.
- Setup/configuration remains `configured_unverified` and `active=false` until real route-bound raw/ledger/no-negative/positive-savings evidence exists. Never treat host config creation as launch support.

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

## Git Flow branch policy

TFY uses Git Flow. See `docs/contributing/GIT_POLICY.md` for the complete policy.

Permanent branches:

- `main` — production/release history only. No direct feature PRs.
- `develop` — integration branch for the next release. Normal PRs target this branch.

Working branches:

- `feature/<slug>` — product feature from `develop` to `develop`.
- `bugfix/<slug>` — non-emergency bug fix from `develop` to `develop`.
- `hotfix/<slug>` — emergency production fix from `main` to `main`, then back-merge/cherry-pick to `develop`.
- `release/<version>` — stabilization branch from `develop` to `main`, then back-merge/tag to `develop`.
- `docs/<slug>` — documentation-only change from `develop` to `develop`.
- `refactor/<slug>` — internal restructuring from `develop` to `develop`.
- `test/<slug>` — test-only work from `develop` to `develop`.
- `chore/<slug>` — CI/repo/dependency maintenance from `develop` to `develop`.

`codex/<slug>` is legacy/scratch-only. Do not open new long-lived review PRs from `codex/*`; convert real AI-authored work to a proper Git Flow branch before review.


## Automatic work-unit Git/GitHub closeout

For every coherent work unit, agents must perform Git/GitHub closeout automatically unless the user explicitly says not to commit, not to push, or not to touch GitHub. Do not wait for a separate “git/GitHub 작업” instruction after implementation is complete.

Closeout steps:

1. Confirm the current branch follows Git Flow. If it does not, create or switch to the correct branch class before committing.
2. Run the required verification gate for the change size.
3. Stage only files that belong to the completed work unit.
4. Commit with the required Conventional+Lore format.
5. Push the branch.
6. Create or update the matching draft PR against the correct Git Flow target branch.
7. Update the PR body with scope, validation, supported/unsupported claims, and risk notes.
8. Wait for GitHub checks when available and report their status.

Do not auto-merge PRs, change branch protection, change repository settings, delete remote branches, or force-push shared history unless explicitly requested or already required for an approved history-cleanup task.

When a user explicitly requests agent-driven merge, the agent may merge after rechecking CI, merge state, branch target, and independent review evidence. GitHub does not allow an author to approve their own pull request; record any failed self-approval attempt honestly and rely on separately collected independent review evidence rather than claiming a GitHub approval that cannot exist.

If a work unit is intentionally local-only, record that in the final response with the reason and the exact unpushed state.

## Commit policy

Every non-trivial commit must use a **Conventional intent line + Lore trailers** format:

```text
<type>(<scope>): <why this change exists>

<optional concise body explaining constraints and approach>

Constraint: <external constraint that shaped the decision>
Rejected: <alternative considered> | <reason>
Confidence: <low|medium|high>
Scope-risk: <narrow|moderate|broad>
Directive: <future-facing warning>
Tested: <verification run>
Not-tested: <known gaps>
```

Allowed subject types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`, `security`.

Commit rules:

- The subject states why the change exists, not a file list.
- `Tested:` and `Not-tested:` are always required.
- Include `Rejected:` for any meaningful alternative that future agents should not retry.
- Include `Directive:` when future agents could accidentally break an invariant.
- Avoid vague subjects: `update files`, `fix stuff`, `WIP`, `agent changes`, `misc`.

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
