# TFY AI Agent Harness

This document expands the repo-root `AGENTS.md` for AI-agent-assisted development. `AGENTS.md` is the authoritative instruction surface; this file is the long-form reference.

## Product invariant

TFY saves model-visible tokens at agent I/O boundaries while preserving recoverable local evidence.

Never weaken these invariants:

1. **Plain text by default** — do not forward JSON envelopes into model context unless the host explicitly asks for machine transport outside the model prompt.
2. **No negative savings** — summarize command output only when the model-visible candidate is smaller than redacted public raw output.
3. **Raw evidence stays local and recoverable** — exact stdout/stderr bytes must be stored before public reduction.
4. **Truthful adapter claims** — MCP support is host-routed MCP integration, not private Codex hook interception.
5. **Proof-gated workspace mutation only** — workspace writes require explicit apply surfaces with authority/provenance, base-hash, per-operation proof, and plan-hash gates; display restoration is never apply authority.

## Code map

- `crates/tfy-core/` — token-saving primitives, raw store, redaction, context/code compaction, command feedback.
- `crates/tfy-runtime/` — runtime envelope and gateway contracts.
- `crates/tfy-cli/src/main.rs` — CLI shape and dispatch only.
- `crates/tfy-cli/src/gateways.rs` — Tool/Context/Output gateway execution.
- `crates/tfy-cli/src/adapter.rs` — generic-shell adapter install/run/report surface.
- `crates/tfy-cli/src/agent.rs` — configured AI-agent wrapper surface; does not mutate ordinary human shell startup files.
- `crates/tfy-cli/src/display.rs` — display-only restore/formatter UX surface.
- `crates/tfy-cli/src/workspace.rs` — exact WorkspaceApplyPlan validate/apply gates.
- `crates/tfy-cli/src/mcp.rs` — stdio MCP JSON-RPC server and Codex MCP setup snippet.
- `crates/tfy-cli/src/util.rs` — small CLI utility helpers.
- `docs/contributing/` — contributor, git, and release workflow harness.

## Command routing for agents

Use targeted checks while editing, then run the full gate before claiming done:

```bash
./scripts/verify.sh
```

Useful focused checks:

```bash
cargo test -p tfy-cli --test tool_gateway
cargo test -p tfy-cli --test adapter_gateway
cargo test -p tfy-cli --test mcp_server
cargo test -p tfy-cli --test agent_workspace
cargo run -q -p tfy-cli -- tool-gateway -- sh -c 'printf ok'
cargo run -q -p tfy-cli -- adapter capabilities
cargo run -q -p tfy-cli -- agent capabilities
cargo run -q -p tfy-cli -- mcp capabilities
```

## Review checklist

Before finalizing a change, verify:

- Tiny command output still passes through without JSON/ref overhead.
- Secret-like values and credential URLs are redacted before model-visible output.
- Non-zero child command exits do not kill MCP server sessions.
- MCP stdout remains JSON-RPC only.
- State/report resources are scoped to the requested session.
- Docs do not claim private hooks or provider prompt mutation unless implemented and tested.
- `tfy agent run` marks agent-runtime origin while direct human terminal commands remain untouched.
- `restore-display` output is readable/display-only and not used as apply authority.
- Workspace apply validates every operation before any write and requires the validated plan hash.
