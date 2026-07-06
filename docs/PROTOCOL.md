# TFY Agent-Neutral Protocol

TFY exposes an agent-neutral protocol so any AI agent, editor, shell wrapper, or orchestration layer can use the same token-saving service.

## Current implemented CLI surface

- `tfy index <path>` — semantic scope names first.
- `tfy expand <path> <scope> [--compactness light|symbol]` — selected compact body, map, metrics.
- `tfy full <path> <scope>` — full source fallback.
- `tfy decide-context` — selected/related/full fallback decision from payload and diagnostics.
- `tfy restore` — deterministic readable restoration from compact payload.
- `tfy restore-display` — display-only readable formatting/restoration for human UX; not apply authority.
- `tfy restore-file` / `tfy restore-patch` — restore compact AI transport into canonical readable code for file writes and user display.
- `tfy tool-gateway -- <command...>` — runtime-facing Tool Gateway entrypoint for command interception; default output is plain model-visible text, not JSON.
- `tfy run -- <command...>` — compatibility/debug spelling for risk-aware compact command feedback.
- `tfy raw <raw_ref>` — full or ranged raw command output.
- `tfy eval-code <path> <scope>` — raw vs compact token estimate and quality gate placeholder.
- `tfy languages` — language adapters.
- `tfy adapter capabilities` — implemented adapter support matrix.
- `tfy adapter install --target generic-shell --dry-run` — reversible setup instructions for command-wrapper interception.
- `tfy adapter run --session <id> -- <command...>` — supported generic-shell command-boundary adapter.
- `tfy adapter report --session <id>` — session savings/evidence report from adapter ledger.
- `tfy start [agent|ai|human|both] [--agent] [--human] [--host <host|all>] [--apply] [--no-apply] [--verify]` / `tfy stop [agent|ai|human|both] [--agent] [--human]` / `tfy fuckyou [agent|ai|human|both] [--agent] [--human] [--yes]` — project lifecycle intent commands backed by `.tfy/lifecycle.json`; `start` also prepares `.tfy/raw` plus `.tfy/state`, `.tfy/adapter`, `.tfy/agent`, and `.tfy/mcp` ledger directories so later raw/log/savings evidence has stable local paths; bare interactive commands open an arrow-key TUI, while piped stdin choices stay script-compatible and missing non-TTY choices fail closed; `stop` preserves raw/shared evidence, while `fuckyou` performs scoped confirmed cleanup and preserves `.tfy/raw` by default. Agent start auto-configuration is safe-writer-only; currently `tfy start --agent` and `tfy start agent` create `.tfy/agent/tfy-agent-wrapper` unless `--no-apply` is passed, while explicit `--host codex`, `--host claude-code`, `--host cursor`, and `--host all` apply their safe project host writers; all record only `configured_unverified`. `active=true` is a derived/effective claim, not a synonym for lifecycle desire or config presence; it requires lifecycle desire plus verified route evidence, raw recovery, no-negative and positive savings, and a supported host/route evidence scope.
- `tfy global start [agent|ai|human|both] [--agent] [--human] [--host <host|all>] [--apply] [--no-apply] [--verify]` / `tfy global stop [agent|ai|human|both] [--agent] [--human]` / `tfy global fuckyou [agent|ai|human|both] [--agent] [--human] [--yes]` — user-global lifecycle intent using `$TFY_HOME`, `$XDG_CONFIG_HOME/tfy`, or `~/.tfy`, separate from project state. `tfy use always|stop|cancel|fuckyou` are convenience aliases for the same user-global lifecycle controls (`always` also accepts the historical typo alias `alwais`). P1 global lifecycle records defaults only; global `--host`/`--apply`/`--verify` options are accepted as host-preference guidance but do not mutate project host config, so each project still needs `tfy start --agent` to create its wrapper, or explicit `--host ...` to write Codex/Claude/Cursor route files.
- `tfy status [--agent] [--human] [--json]` — lifecycle/support status with project/global separation, a top-level `lifecycle_summary` for user-facing desired/configured/active/evidence-required state, explicit effective project-over-global desired/configured/active state, desired_source=none in fresh projects, project_parse_error for malformed local lifecycle state, next-action guidance, and truthful route limitations.
- `tfy agent capabilities` — reports the configured AI-agent-wrapper-only automatic interception boundary and origin contract.
- `tfy agent install --dry-run` — prints a reversible wrapper script without mutating shell startup files.
- `tfy agent run --session <id> -- <command...>` — AI-runtime command wrapper that marks origin as agent runtime while leaving ordinary human terminals untouched. Explicit host setup can additionally write Codex `.codex/config.toml` PreToolUse Bash hooks, Claude Code `.claude/settings.json` PreToolUse Bash hooks, and Cursor `.cursor/mcp.json` MCP routing.
- `tfy hook capabilities` / `tfy hook install --target <host> --dry-run` / `tfy hook run --host codex|claude-code` — official-host-hook routing surface for Codex and Claude Code plus a test shim. Hook routes are thin routers into shared gateways: PreToolUse rewrites Bash input to a TFY command route, does not execute the pending command inside the hook process, fails closed through `TFY_HOOK_DISABLE=1`, and still requires real host-bound route evidence before launch claims.
- `tfy mcp capabilities` — supported MCP tools/resources and support-boundary report.
- `tfy mcp serve --session <id>` — MCP stdio JSON-RPC server; stdout is JSON-RPC only. Exposes Tool/Context/Output/State tools, including host-routed Code I/O tools for bounded scope listing, compact context, preview validation, restore-display, WorkspaceApplyPlan validation, and proof-gated apply.
- `tfy setup --ai --host codex --apply --project` / `--host claude-code --apply --project` / `--host cursor --apply --project` — safe reversible project host writers with backup, idempotency, provenance, and TFY-only uninstall.
- `tfy workspace validate --payload plan.json` / `tfy workspace apply --payload plan.json --plan-hash <hash>` — explicit multi-file WorkspaceApplyPlan validation and exact apply for modify/add/delete/rename/move operations, gated by origin/provenance, per-op proof, base hashes, and plan hash. Conservative `modify_fuzzy` is supported only with `allow_fuzzy_apply`, confidence threshold, unique anchor, base hash, preview hash, and per-op proof gates; ambiguous fuzzy edits fail closed. Multiple same-file mutations are rejected until range-aware application exists; callers must merge them into one full-file candidate.
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
tfy shell -- <ordinary command>   # TFY Shell Gateway wrapper
```

`tfy shell <ordinary command>` without the separator is deliberately outside this Tool Gateway contract: it is raw passthrough convenience for humans, with native stdout/stderr/exit code and no TFY summary, raw store, or ledger artifact.

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

Do not overclaim automatic interception: today the Rust CLI/core primitives, `tfy-runtime`, local Tool/Shell/Context/Output-preview-and-proof-gated-apply/State gateway surfaces, generic-shell adapter, configured AI-agent wrapper, MCP stdio tool/resource server, and hook test shim/planning surface exist. MCP supports host-routed Code I/O with snapshot-stable scope ids, compact context, preview validation, restore-display, WorkspaceApplyPlan validation, proof-gated single-file selected-scope apply, and proof-gated exact multi-file workspace apply. Hook routes are optional official-host UX routers into those same gateways; they do not own summarization, redaction, restore, apply, or claim promotion logic. Private/hidden Codex hooks and universal human-shell interception are not claimed. Editor auto-integration and provider/API gateway proxying are out of scope. Conservative fuzzy mutation is supported only through explicit proof-gated WorkspaceApplyPlan operations.

`tfy launch-report` is the product release reducer. Required v1 routes are `generic_shell` and `tfy_agent_adapter`; both must reach `launch_supported` from route-specific smoke/ledger evidence, raw refs, no-negative-savings proof, and `--host-evidence` JSON with existing setup/invocation artifact paths plus measured overhead (`overhead_ms`/`baseline_ms`) or an explicit `overhead_exception` before the report can pass. `mcp_stdio` is advanced/complementary and local self-smoke evidence is intentionally downgraded to `verified_local_mcp` or `verified_local_hook` until a real host invocation exists. Named hosts such as Codex, Claude Code, Cursor, OpenCode, and Hermes start at `config_snippet_available`/`configurable`; they promote through the evidence ladder `config_written`, `host_launched`, `verified_host_mcp_invocation` or `verified_host_hook`, `route_evidence_recorded`, `savings_verified`, and finally `launch_supported` only when host-bound evidence includes route type, config scope/path, smoke id, setup/invocation artifacts, ledger/raw artifacts, and no-negative plus positive savings for that specific host. OpenClaw remains `planned_discovery`. Provider/API proxying, editor auto-integration, private Codex hooks, and universal human terminal interception remain `not_supported`/planned unless separate adapters and e2e evidence exist.

Savings reporting separates exact byte accounting from token estimates. Byte fields are authoritative (`raw_bytes`, `model_bytes`, `saved_bytes`); token fields are a conservative `ceil(bytes/4)` proxy unless a future adapter records tokenizer-specific counts. Raw evidence is local by default under `.tfy/raw` or configured `--raw-dir` plus gateway ledgers; TFY stores it for audit/recovery and does not upload it.

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
tfy agent report --session <id>       # human-facing report of agent custom-rule candidates
tfy tool-gateway -- <command...>        # model-visible text default
tfy shell <command...>                  # raw passthrough convenience, no TFY savings/artifacts
tfy shell -- <command...>               # model-visible text default through TFY gateway
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
tfy smoke --host codex --live --json
tfy smoke --host claude-code --live --json
tfy setup --ai --codex --dry-run
tfy setup --ai --host codex --apply --project
tfy setup --ai --host claude-code --apply --project
tfy setup --ai --host cursor --apply --project
tfy status --json
tfy explain
tfy gain # no-data until command-output savings events exist
tfy mcp capabilities
tfy mcp serve --session <id> --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw
tfy mcp install --target codex --dry-run
tfy context-gateway <path> <scope> [--diagnostics ...]
tfy output-gateway --payload payload.json
tfy restore-display --payload payload.json
tfy restore-file --payload payload.json --output restored.js
tfy restore-patch --payload payload.json
tfy workspace validate --payload workspace-plan.json
tfy workspace refactor-plan --payload workspace-plan.json --chunk-size 5
tfy workspace apply --payload workspace-plan.json --plan-hash <hash>
tfy state-append --payload event.json
tfy state-project
```

`tfy shell <command>` is raw passthrough convenience; `tfy shell -- <command>` is the local shell-adapter wrapper. `tfy agent run` is the AI-runtime wrapper with explicit origin/provenance and `user_shell_mutated=false`. Neither command magically modifies a third-party runtime by itself; a runtime must configure its command execution path to call the wrapper. `tfy mcp serve` is the supported MCP stdio integration point for MCP-aware hosts; it still requires host MCP routing and is not a private Codex hook or provider prompt gateway.

## Product lifecycle protocol

`tfy init`, `tfy doctor`, `tfy smoke`, and `tfy gain` are product-facing wrappers around the lower-level protocol surfaces:

- Bare interactive `tfy start` / `tfy stop` / `tfy fuckyou` use an arrow-key TUI; non-interactive stdin choices remain available for automation. `tfy start` / `tfy stop` / `tfy fuckyou` manage lifecycle and supported-route configuration, not proof of global interception. Agent lifecycle records supported route intent (`agent_wrapper`, `generic_shell_adapter`, `official_host_hook_when_configured`, `mcp_stdio_complementary`), creates the project agent wrapper by default, and keeps `active=false`, `private_hook_interception=false`, and `provider_prompt_gateway=false`; named-host launch support still requires launch-report evidence. Human lifecycle records managed-session intent, `route_state=intent_recorded`, `active=false`, and `ordinary_terminal_interception=false`; supported platform shells (Linux bash, macOS zsh, Windows PowerShell) can run `tfy start --human` to create/refresh `.tfy/human/auto-activate.json` plus deterministic shell-specific activation content and then enter a TFY-managed current-directory-scoped session where safely resolved ordinary external commands route through TFY by default and may store combined raw command output before summary selection. Non-interactive plain `tfy start --human` remains lifecycle intent-only; `tfy start --human --auto-activate` is the explicit automation path for marker creation without entering a shell. `tfy setup --human` dry-runs the one-time user rc/profile hook; `tfy setup --human --apply` installs it by delegating to the same safety path as `tfy human auto-activate install --shell <bash|zsh|powershell> --rcfile <path> --apply`, which remains supported. npm install may print this guidance but must not edit startup files. New supported shell sessions that read that hook check only `$PWD/.tfy/human/auto-activate.json`, validate that current-directory marker/script through a pinned absolute TFY executable, export `TFY_HUMAN_ROOT`/`TFY_HUMAN_TFY_BIN`, and source only deterministic TFY-generated content. Shell-local/direct-path/explicit-bypass/TFY-gateway/outside-scope commands run raw without summary/no-negative-savings claims; newly created PATH executables may run raw until the next prompt-time/proxy refresh; automatic interactive/TUI/stateful subcommand classification is not claimed in v1. Unsupported platforms report `managed_session_available=false` with `support_status=managed_session_unsupported_platform`. Ordinary terminals outside managed sessions or trusted marked current directories with the explicit rc/profile hook are not globally intercepted; nested child-shell command boundaries and shell `$?` parity are not claimed unless separately proven.

## Adapter v1 protocol

`tfy adapter` is the first implemented automatic-interception surface. It is automatic only after a host agent/runtime configures its command execution path to call the adapter wrapper.

`tfy hook` is the official-host-hook router for Codex and Claude Code plus a test-shim equivalence surface. For PreToolUse, it rewrites Bash input to a TFY command route and does not execute the pending command inside the hook process. It fails closed with `TFY_HOOK_DISABLE=1`; setup remains `configured_unverified` until host-bound e2e route evidence proves the host ran the rewritten TFY command and preserved raw-first/no-negative behavior.

Supported today:

- `generic-shell`: supported command-boundary interception through `tfy adapter run` or a dry-run/generated shell shim.

Not yet claimed as supported automatic interception:

- Codex-specific hooks
- host-specific hook writers without official documentation and e2e evidence
- MCP proxy/server interception
- editor integrations
- provider prompt/cache payload mutation

Adapter v1 keeps the same model-visible contract as Tool Gateway: stdout is plain selected text by default, JSON is explicit debug/internal only, raw bytes are stored locally first, and session reports measure baseline raw vs model-visible output.

Adapter reports use `raw_bytes` and `model_bytes` as the public size contract. Legacy runtime ledger fields such as `raw_chars` / `model_chars` are compatibility-only and are not emitted by `tfy adapter report`.

## MCP stdio contract

`tfy mcp serve` implements a line-delimited JSON-RPC stdio server for MCP hosts. It supports `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/templates/list`, and `resources/read`. `initialize` declares both tool and resource capabilities. Concrete resources are returned by `resources/list`; parameterized URI templates are returned by `resources/templates/list`.

MCP tools expose existing TFY gateway behavior rather than a second compression engine. `tfy_tool_run` stores raw output locally, returns compact model-facing output only when it is smaller/safe, preserves command exit metadata, appends an MCP ledger event when possible, and never exits the MCP server on child command failure. `tfy_scope_list` returns bounded snapshot-stable scope ids, `tfy_context_get` returns compact selected-scope context with symbol map, `base_compact_code`, `context_ref`, and `ApplyProof`, `tfy_output_validate` restores in preview-only mode, and `tfy_output_apply` mutates only through the existing proof-gated single-file selected-scope apply path. `tfy_restore_display` provides display-only readable output, while `tfy_workspace_validate` and `tfy_workspace_apply` expose exact multi-file WorkspaceApplyPlan validation/apply with plan-hash and per-operation proof gates. Parent event ids or ledger state alone are not apply authority. Raw/report/state recovery is available through `tfy://raw/{raw_ref}`, `tfy://report/{session}`, and `tfy://state/{session}`.

MCP session resources are session-scoped. `tfy://state/{session}` and `tfy_state_project` filter the ledger by requested session before projecting state, so one session cannot receive another session's tool evidence through state reads. JSON-RPC notifications such as `notifications/initialized` are treated as one-way messages and do not produce stdout responses.

## Release tier evidence reducer

`tfy launch-report --json` includes four evidence-gated release tiers:

- `developer_preview_ready` requires verified local build/install evidence, first-success route evidence for `generic_shell` and `tfy_agent_adapter`, no-negative plus positive savings, raw lifecycle availability, a benchmark self-manifest, and unsupported claim audit pass; `mcp_stdio` may strengthen evidence but is not a required command-interception route.
- `rc_ready` requires developer preview readiness plus release archive/checksum dry-run, docs/demo/release notes, independent reviews, and PR/CI evidence supplied through `--release-evidence`.
- `ga_ready` remains blocked until at least one named AI host has real invocation evidence with host-bound ledger/raw/no-negative/positive-savings proof and reproducible demo evidence.
- `public_superiority_claim_ready` remains blocked unless a reviewed external benchmark manifest records baseline, corpus, reproducibility, correctness/no-lost-evidence proof, and overhead comparison.

`--release-evidence <json>` accepts packaging/docs/review/CI proof fields, but each boolean must be backed by existing artifact paths (`cargo_install_binary`, `release_binary`, archive/checksum artifacts, docs/release-notes artifacts, review/CI artifacts) before it is trusted. `benchmark_manifest_generated` additionally requires an attached benchmark manifest with `status=pass`, no-negative savings, positive savings, and raw refs for every scenario. This reducer is evidence reporting only; it does not promote provider/API proxying, private Codex hooks, universal terminal interception, editor auto-hooks, or named-host launch support without matching route evidence.

## Raw evidence lifecycle

Raw command output is recoverable by `tfy raw <raw_ref>` and MCP `tfy_raw_get`. First-class lifecycle commands are available for local evidence management:

```sh
tfy raw --list --json
tfy raw <raw_ref> --inspect --json
tfy raw <raw_ref> --export ./raw-export
tfy raw --prune --dry-run --json
tfy raw --prune --apply --json
```

Prune is dry-run unless `--apply` is supplied. These commands manage raw evidence JSON files; gateway ledgers remain explicit local files.
