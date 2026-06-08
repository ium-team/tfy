# TFY Agent-Neutral Protocol

TFY exposes an agent-neutral protocol so any AI agent, editor, shell wrapper, or orchestration layer can use the same token-saving service.

## Current implemented CLI surface

- `tfy index <path>` — semantic scope names first.
- `tfy expand <path> <scope> [--compactness light|symbol]` — selected compact body, map, metrics.
- `tfy full <path> <scope>` — full source fallback.
- `tfy decide-context` — selected/related/full fallback decision from payload and diagnostics.
- `tfy restore` — deterministic readable restoration from compact payload.
- `tfy restore-display` — display-only readable formatting/restoration for human UX; not apply authority.
- `tfy tool-gateway -- <command...>` — runtime-facing Tool Gateway entrypoint for command interception; default output is plain model-visible text, not JSON.
- `tfy run -- <command...>` — compatibility/debug spelling for risk-aware compact command feedback.
- `tfy raw <raw_ref>` — full or ranged raw command output.
- `tfy eval-code <path> <scope>` — raw vs compact token estimate and quality gate placeholder.
- `tfy languages` — language adapters.
- `tfy adapter capabilities` — implemented adapter support matrix.
- `tfy adapter install --target generic-shell --dry-run` — reversible setup instructions for command-wrapper interception.
- `tfy adapter run --session <id> -- <command...>` — supported generic-shell command-boundary adapter.
- `tfy adapter report --session <id>` — session savings/evidence report from adapter ledger.
- `tfy agent capabilities` — reports the configured AI-agent-wrapper-only automatic interception boundary and origin contract.
- `tfy agent install --dry-run` — prints a reversible wrapper script without mutating shell startup files.
- `tfy agent run --session <id> -- <command...>` — AI-runtime command wrapper that marks origin as agent runtime while leaving ordinary human terminals untouched.
- `tfy mcp capabilities` — supported MCP tools/resources and support-boundary report.
- `tfy mcp serve --session <id>` — MCP stdio JSON-RPC server; stdout is JSON-RPC only. Exposes Tool/Context/Output/State tools, including host-routed Code I/O tools for bounded scope listing, compact context, preview validation, restore-display, WorkspaceApplyPlan validation, and proof-gated apply.
- `tfy workspace validate --payload plan.json` / `tfy workspace apply --payload plan.json --plan-hash <hash>` — explicit multi-file WorkspaceApplyPlan validation and exact apply for modify/add/delete/rename/move operations, gated by origin/provenance, per-op proof, base hashes, and plan hash. Fuzzy mutation remains fail-closed/preview-only.
- `tfy mcp install --target codex --dry-run` — concrete Codex MCP command/TOML setup snippet without mutating config.

## Final protocol capabilities

The final architecture extends the protocol around registry methods:

- `registry` — list available token-saving methods and their gates.
- `ref put/get` — content-addressed artifact references for stable/repeated artifacts; command raw refs remain opaque append-only evidence handles.
- `delta` — changed-artifact views since a known snapshot.
- `skeleton` — semantic structure without full bodies.
- `budget` — choose cheapest sufficient representation for a token budget.
- `ledger` — compact task/conversation state.
- `adapter` — optional provider/model layout and usage accounting.

Names may change during implementation, but the concepts are part of the first public architecture.

## Agent contract

1. Start with registry/index/skeleton before requesting full context.
2. Use selected compact views only with TFY-provided maps/refs.
3. Expand related/full/raw context when diagnostics, ambiguity, or risk appears.
4. Emit patch/edit-script outputs when possible; use restoration before human/project output.
5. Preserve raw refs for command, CI, Git/GitHub, security, and review evidence.
6. Treat provider adapters as optional optimizations; the neutral protocol remains sufficient.
7. Do not claim token savings without evaluation-gate evidence.

## Data contract

Machine payloads may use compact schema dictionaries for token efficiency. Every compact schema must have:

- schema version
- human-readable debug form
- stable field mapping
- compatibility/fallback behavior

## Hardened contracts

- Scope IDs, not display names, are the snapshot-stable exact selection contract for the current indexed snapshot.
- Command raw refs are validated, append-only, opaque evidence handles; they are not a stable content-addressing contract.
- Python layout/string literals are preserved when needed for correctness.
- Non-Python compaction preserves strings/template literals.
- Restoration rejects unmapped compact symbols.
- Provider adapter usage must report hit/miss/usage evidence when available.


## Model-visible Tool Gateway protocol

The default Tool Gateway contract is text-first:

```sh
tfy tool-gateway -- <ordinary command>
tfy shell -- <ordinary command>
```

Execution flow:

1. Run the original argv.
2. Store exact stdout/stderr bytes locally behind an opaque `raw_ref`.
3. Build a redacted public raw candidate.
4. Build a redacted compact summary candidate.
5. Return the summary only when it is strictly smaller than the public raw candidate.
6. Otherwise return the redacted raw text unchanged.

This means small outputs such as `ok` stay `ok`; TFY does not add JSON, envelope keys, or `raw_ref` overhead when doing so would increase model-visible tokens. Summarized, truncated, suppressed, or unsafe output includes a recovery hint/ref. Redaction happens before any pass-through reaches the model.

## Middleware protocol boundary

The CLI is both a manual debug surface and the contract used by adapters. Production usage should prefer gateway names where available:

- Tool Gateway: `tfy tool-gateway -- <ordinary command>` wraps command execution and returns the smallest safe model-visible text chosen by the net-savings gate.
- Context Gateway: `tfy context-gateway` runtime envelope over `index`, `expand`, `full`, and `decide-context`; automatic external hooks remain adapter work.
- Output Gateway: `tfy output-gateway` structured preview/validate API over `restore`; `--apply` supports local single-file selected-scope replacement only when a content-addressed `ApplyProof` validates the exact path, byte range, source hash, compactness, language, and parser confidence. Parent event ids alone are not authoritative; apply payloads must also carry the captured source context_ref and base compact code so TFY can bind the proposed edit to the proven context.
- State Gateway: `tfy state-append` / `tfy state-project` ledger/ref API fed by gateway events.

Do not overclaim automatic interception: today the Rust CLI/core primitives, `tfy-runtime`, local Tool/Shell/Context/Output-preview-and-proof-gated-apply/State gateway surfaces, generic-shell adapter, configured AI-agent wrapper, and MCP stdio tool/resource server exist. MCP supports host-routed Code I/O with snapshot-stable scope ids, compact context, preview validation, restore-display, WorkspaceApplyPlan validation, proof-gated single-file selected-scope apply, and proof-gated exact multi-file workspace apply. Codex private hooks, editor hooks, provider gateway, fuzzy mutation, and universal shell interception remain separate adapters/features until they pass e2e gates.

## Runtime envelope protocol

Structured runtime interception uses the `tfy-runtime` crate. Runtime envelopes are **debug/adapter/internal only** unless an integration explicitly needs machine-readable transport outside the model prompt. Adapters must not forward envelopes, ledger JSON, `--json`, or `--jsonl` output into model context by default.

For Tool Gateway envelopes, `payload.model_text` is the selected model-visible text and `payload.rendering_kind` describes how it was produced (`pass_through`, `summary`, or `suppressed`). The legacy-compatible `payload.summary` mirrors `model_text` for now and must not be interpreted as always being a synthesized summary.

Every runtime-facing request, response, and event is wrapped in `RuntimeEnvelope<T>` with:

- `protocol_version`
- `adapter_kind` and `adapter_version`
- `supported_gateways`
- `authority_mode`
- `session_id`, `request_id`, `trace_id`, optional `turn_id`, optional `parent_event_id`
- `provenance`
- `origin` (`kind`, `host`, `invocation`, `intercepted`, `user_shell_mutated`)
- `policy`
- gateway-specific `payload`

Current runtime-facing CLI surfaces:

```sh
tfy runtime-capabilities
tfy runtime-negotiate --gateway tool --output-mode text
tfy tool-gateway -- <command...>        # model-visible text default
tfy shell -- <command...>               # model-visible text default
tfy tool-gateway --json -- <command...> # debug/adapter/internal only
tfy tool-gateway --jsonl -- <command...># debug/adapter/internal only
tfy shell --json -- <command...>        # debug/adapter/internal only
tfy adapter capabilities
tfy adapter install --target generic-shell --dry-run
tfy adapter run --session <id> -- <command...>
tfy adapter report --session <id>
tfy agent capabilities
tfy agent install --dry-run
tfy agent run --session <id> -- <command...>
tfy init --codex --dry-run
tfy init --codex --project --apply
tfy init --show
tfy init --uninstall --codex --project --apply
tfy doctor --codex
tfy smoke --mcp
tfy smoke --codex
tfy gain # no-data until command-output savings events exist
tfy mcp capabilities
tfy mcp serve --session <id> --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw
tfy mcp install --target codex --dry-run
tfy context-gateway <path> <scope> [--diagnostics ...]
tfy output-gateway --payload payload.json
tfy restore-display --payload payload.json
tfy workspace validate --payload workspace-plan.json
tfy workspace apply --payload workspace-plan.json --plan-hash <hash>
tfy state-append --payload event.json
tfy state-project
```

`tfy shell` is the local shell-adapter wrapper. `tfy agent run` is the AI-runtime wrapper with explicit origin/provenance and `user_shell_mutated=false`. Neither command magically modifies a third-party runtime by itself; a runtime must configure its command execution path to call the wrapper. `tfy mcp serve` is the supported MCP stdio integration point for MCP-aware hosts; it still requires host MCP routing and is not a private Codex hook or provider prompt gateway.

## Product lifecycle protocol

`tfy init`, `tfy doctor`, `tfy smoke`, and `tfy gain` are product-facing wrappers around the lower-level protocol surfaces:

- `tfy init --codex` defaults to project-scoped dry-run. `--apply` writes only a TFY-owned marker block in `AGENTS.md`; global mode targets `~/.codex/AGENTS.md`. P0 prints the Codex MCP command and does not directly mutate `~/.codex/config.toml`.
- `tfy init --show` reports marker-block state. `tfy init --uninstall --codex --project --apply` removes only the TFY marker block and preserves all non-TFY content.
- `tfy doctor --codex` checks the local binary, starts an MCP child for `initialize`/`tools/list`, verifies required tools, checks `.tfy/mcp` and `.tfy/raw` writability, and reports Codex setup as pass/warn/fail without claiming private hooks.
- `tfy smoke --mcp` runs a deterministic local MCP Code I/O scenario: scope list, exact-id compact context, preview validate without mutation, proof-gated apply, and ledger evidence.
- `tfy smoke --codex` is checklist/report-only in P0; it may guide manual host validation but must not claim Codex invoked TFY.
- `tfy gain` reads `.tfy/mcp/ledger.jsonl`, `.tfy/adapter/ledger.jsonl`, and explicit `--ledger` paths, then reports real raw/model-visible byte savings from `ToolCommandCompleted` command-output events produced by `tfy_tool_run`, Tool Gateway, or the adapter. Code I/O smoke events are verification evidence, not savings data. With no command data it exits successfully with `No TFY savings data found yet`.

## Adapter v1 protocol

`tfy adapter` is the first implemented automatic-interception surface. It is automatic only after a host agent/runtime configures its command execution path to call the adapter wrapper.

Supported today:

- `generic-shell`: supported command-boundary interception through `tfy adapter run` or a dry-run/generated shell shim.

Not yet claimed as supported automatic interception:

- Codex-specific hooks
- MCP proxy/server interception
- editor integrations
- provider prompt/cache payload mutation

Adapter v1 keeps the same model-visible contract as Tool Gateway: stdout is plain selected text by default, JSON is explicit debug/internal only, raw bytes are stored locally first, and session reports measure baseline raw vs model-visible output.

Adapter reports use `raw_bytes` and `model_bytes` as the public size contract. Legacy runtime ledger fields such as `raw_chars` / `model_chars` are compatibility-only and are not emitted by `tfy adapter report`.

## MCP stdio contract

`tfy mcp serve` implements a line-delimited JSON-RPC stdio server for MCP hosts. It supports `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/templates/list`, and `resources/read`. `initialize` declares both tool and resource capabilities. Concrete resources are returned by `resources/list`; parameterized URI templates are returned by `resources/templates/list`.

MCP tools expose existing TFY gateway behavior rather than a second compression engine. `tfy_tool_run` stores raw output locally, returns compact model-facing output only when it is smaller/safe, preserves command exit metadata, appends an MCP ledger event when possible, and never exits the MCP server on child command failure. `tfy_scope_list` returns bounded snapshot-stable scope ids, `tfy_context_get` returns compact selected-scope context with symbol map, `base_compact_code`, `context_ref`, and `ApplyProof`, `tfy_output_validate` restores in preview-only mode, and `tfy_output_apply` mutates only through the existing proof-gated single-file selected-scope apply path. `tfy_restore_display` provides display-only readable output, while `tfy_workspace_validate` and `tfy_workspace_apply` expose exact multi-file WorkspaceApplyPlan validation/apply with plan-hash and per-operation proof gates. Parent event ids or ledger state alone are not apply authority. Raw/report/state recovery is available through `tfy://raw/{raw_ref}`, `tfy://report/{session}`, and `tfy://state/{session}`.

MCP session resources are session-scoped. `tfy://state/{session}` and `tfy_state_project` filter the ledger by requested session before projecting state, so one session cannot receive another session's tool evidence through state reads. JSON-RPC notifications such as `notifications/initialized` are treated as one-way messages and do not produce stdout responses.
