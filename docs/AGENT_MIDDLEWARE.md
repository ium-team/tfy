# TFY Agent Middleware

## Purpose

TFY is intended to sit inside supported AI-agent runtimes as an **I/O middleware**, not merely as a command a human types manually. The Rust CLI remains the debug/protocol surface, and the supported product path is explicit host routing through TFY wrappers, adapters, or MCP tools. Editor auto-integration and provider/API gateway proxying are outside TFY scope, and private/hidden Codex prompt hooks are not claimed.

Current status: the Rust core, CLI primitives, project/global lifecycle intent commands (`tfy start`, `tfy stop`, `tfy status --agent/--human`, `tfy fuckyou`, `tfy global start|stop|fuckyou`) plus `tfy use always|stop|cancel|fuckyou` global aliases, bare-command arrow-key TUI selection, lifecycle target aliases (`agent`/`ai`, `human`, `both`), effective status guidance including `lifecycle_summary`, lifecycle `route_state`/`active` fields, `tfy-runtime` wire contract, Tool/Shell Gateway surfaces, Context Gateway CLI, Output Gateway preview/validate CLI plus proof-gated local single-file selected-scope apply, State Gateway ledger/projection CLI, `tfy adapter` generic-shell command-boundary adapter, configured `tfy agent` AI-runtime wrapper, display-only restore formatter, exact multi-file WorkspaceApplyPlan validate/apply, and `tfy mcp` stdio tool/resource server are implemented. MCP now includes a host-routed agent-native Code I/O workflow: bounded ID-first scope listing, compact selected-scope context with `ApplyProof`, preview-only validation, display-only restoration, WorkspaceApplyPlan validation, and proof-gated apply. Product setup can now generate named-host MCP routing snippets for Codex, Claude Code, Cursor, OpenCode, and Hermes, and can safely apply/uninstall Codex project `.codex/config.toml`, Claude Code project `.mcp.json`, and Cursor project `.cursor/mcp.json` routing with backup/idempotency/provenance; `tfy start --agent` defaults to the Codex writer, prepares raw/ledger directories, and records only `configured_unverified`/`active=false` until lifecycle desire is on and real route evidence exists; `--no-apply` keeps lifecycle intent only. OpenClaw remains planned-discovery until an official/current route is proven. Private/hidden Codex hooks are not claimed, and editor/provider automatic hooks are outside TFY scope. Conservative proof-gated unique-anchor fuzzy workspace mutation is implemented; broader semantic conflict resolution remains fail-closed.

## Boundary model

The gateway model is the external runtime integration model. It does not replace TFY's internal architecture spine:

- Internal spine: representation ladder, token-saving method registry, agent-neutral Rust core.
- External boundaries: Tool Gateway, Context Gateway, Output Gateway, State Gateway.
- Runtime connectors: shell/agent wrapper, MCP host routing, and other explicit AI-runtime adapters. Provider/API gateways and editor auto-integration are out of scope; private hidden Codex hooks are not claimed.

## Gateway-to-registry crosswalk

| Gateway boundary | Runtime question | Current TFY primitives | Method families | Owner | Status | Fallback |
|---|---|---|---|---|---|---|
| Tool Gateway | What happens when an agent runs an ordinary command/tool? | `tfy tool-gateway`, `tfy agent run`, `tfy run`, `tfy raw`, `RawStore`, `ToolPolicy` | tool feedback compression, raw refs, output fingerprinting, error clustering, Git/GitHub harness, redaction | Rust core + shell/tool wrapper adapter | Core primitive and CLI entrypoint implemented | `raw_ref`, ranged raw expansion, exit/risk/evidence preservation |
| Context Gateway | What context reaches the model before reasoning? | `tfy context-gateway`, `tfy index`, `expand`, `full`, `decide-context`; MCP `tfy_scope_list`/`tfy_context_get` | semantic skeletons, compact code/maps, retrieval budget planner, adaptive compactness, dependency-neighborhood slicing, refs/deltas | Rust core + runtime envelope + MCP host-routed workflow | Runtime-facing CLI and MCP selected-scope context implemented; private Codex/editor hooks not claimed | related/full fallback on low confidence, diagnostics, unresolved symbols |
| Output Gateway | What happens after the model emits compact code or patches? | `tfy output-gateway`, `tfy restore`, `tfy restore-display`, `tfy workspace validate/apply`; MCP `tfy_output_validate`/`tfy_output_apply`/`tfy_restore_display`/`tfy_workspace_validate`/`tfy_workspace_apply`; proof-gated explicit apply API | deterministic restoration, patch/edit-script output, compact schemas, validation gates | Rust core + runtime envelope + MCP host-routed workflow; narrow local apply implemented | Preview/validate CLI/MCP, content-addressed single-file selected-scope apply, display/file readable restoration, exact multi-file WorkspaceApplyPlan apply, and conservative unique-anchor fuzzy apply with required base and preview proof hashes implemented behind authority/provenance gates | reject unmapped/stale symbols; request full/context fallback before apply |
| State Gateway | What persists across turns without raw-history bloat? | `tfy state-append`, `tfy state-project`, raw refs as evidence handles | task-state compaction, output fingerprinting, local memoization, refs/deltas | Rust runtime schema + event ledger | Append/project CLI implemented; external session adapters planned | preserve decisions/evidence/raw refs; non-authoritative projection triggers fallback |

## Tool Gateway: implemented first

The Tool Gateway is the first middleware wedge because it can wrap ordinary commands without provider APIs.

Agent intent:

```sh
cargo test
```

Gateway execution:

```sh
tfy tool-gateway -- cargo test
```

The AI agent receives plain model-visible text selected by a net-savings gate. Exact stdout/stderr bytes are stored locally first. If the compact summary is strictly smaller than the redacted public raw output, the model receives the summary plus recovery hint/ref. If a same-session command repeats with the same exit code and output hash, TFY elides the repeat only after storing a fresh raw ref and only when the repeat notice is smaller than raw output. If the raw output is already smaller, the model receives the redacted raw output with no JSON/envelope overhead. The implemented P0 command-family path adds fixture-driven summaries and `command_family` analytics for Git status/diff/log, `gh pr checks`, Cargo build/test/check/clippy/fmt-check, TypeScript no-emit checks, and common test runners while leaving unsupported commands on the generic safe path.

`tfy agent report --session <id>` summarizes local `.tfy/agent/custom-guidance.jsonl` records for agent commands that used generic/weak summaries without adding that guidance to model-visible command output.

`tfy run -- <command>` remains a compatibility/debug spelling. `tfy tool-gateway -- <command>` is the runtime-facing name that communicates the intended integration boundary.

### Tool Gateway contracts

- Accept ordinary argv and run the same command.
- Preserve command exit code.
- Store exact stdout/stderr bytes locally behind `raw_ref`.
- Redact public credential-bearing URLs and secret-like assignments before pass-through while preserving original raw output locally.
- Return summaries only when they are strictly smaller than redacted public raw output.
- Pass through redacted raw output for tiny/equal/larger-summary cases to avoid negative savings.
- Suppress binary-ish or unsafe output with a short recoverable placeholder plus `raw_ref`; never emit empty unrecoverable model text for suppressed content.
- Classify success/unknown/critical conservatively.
- Preserve Git/GitHub evidence for dirty state, conflicts, failed checks, review blockers, auth/rate-limit/security output.
- Keep raw/ranged expansion available through `tfy raw`.

## Context Gateway: implemented CLI, external hooks planned

The Context Gateway mediates model input. It should choose the cheapest safe representation:

```text
index/skeleton -> selected compact scope -> related neighborhood -> full fallback
```

It uses existing primitives (`index`, `expand`, `full`, `decide-context`) through `tfy context-gateway` and returns a runtime envelope. Through MCP, `tfy_scope_list` provides bounded snapshot-stable scope ids and `tfy_context_get` returns compact selected-scope code plus symbol map, `base_compact_code`, `context_ref`, and `ApplyProof`. This is host-routed MCP tool use, not private model-context interception for Codex/editor/provider runtimes.

## Output Gateway: preview/validate plus proof-gated apply

The Output Gateway handles structured outputs in preview/validate mode and can apply a narrow content-addressed single-file selected-scope replacement. TFY also now exposes `tfy restore-display` for display-only human-readable restoration and `tfy workspace validate/apply` for exact multi-file WorkspaceApplyPlan operations. The same boundaries are available through MCP as preview-only `tfy_output_validate`, proof-gated `tfy_output_apply`, `tfy_restore_display`, `tfy_workspace_validate`, and `tfy_workspace_apply`:

- compact code payloads produced with TFY symbol maps
- compact patches/edit scripts with explicit scope IDs and map refs
- restore/validate/apply-ready outputs where failure can become a fallback request

It must not rewrite arbitrary natural-language model responses. It may mutate the workspace only through `output-gateway --apply` when exactly one `ApplyProof` source validates the captured path, selected byte range, selected-source hash, compactness/language, restored symbols, and parser confidence; parent event ids alone are not authority.

## State Gateway: schema-first, implemented as event-fed CLI

The State Gateway is a shared ledger substrate fed by other gateways, not a separate product spine.

Minimum ledger fields:

```yaml
goal: current task objective
constraints: relevant constraints
decisions: durable decisions and rationale
openQuestions: unresolved correctness-affecting questions
changedFiles: paths, scopes, and patch refs touched so far
toolEvidence: command summaries, risk, raw_ref, around hints
contextRefs: scope refs, full fallback refs, summaries, invalidation hashes
blockers: blockers and required authority/input
verification: checks run, results, timestamps, raw refs
nextActions: bounded next steps and stop condition
```

The current CLI seeds the ledger from Tool Gateway JSON/JSONL events and can project compact state. Context and Output events use the same envelope schema and should be appended by adapters when they participate in a runtime loop.

## Adapter rule

Adapters connect concrete runtimes to the Rust core. They may automate invocation, layout, caching, or transport, but they cannot weaken TFY correctness semantics:

- no hiding critical evidence
- no dropping raw/full fallback
- no provider-specific behavior required for correctness
- no claim of token savings without evaluation gates

## Runtime SDK and automatic-interception foundation

The first full-agent-runtime implementation adds a shared runtime wire contract plus local gateway surfaces. This is the foundation required before TFY can truthfully claim automatic interception for a concrete runtime.

Implemented runtime foundation:

- `tfy-runtime` crate with `RuntimeEnvelope<T>`, `GatewayRequest`, `GatewayResponse`, `GatewayEvent`, `AdapterCapabilities`, `RuntimePolicy`, `ProvenanceRefs`, `FallbackReason`, and `ValidationStatus`.
- Mandatory envelope metadata: `protocol_version`, `adapter_kind`, `adapter_version`, `supported_gateways`, `authority_mode`, `session_id`, `request_id`, `trace_id`, `policy`, `provenance`, `origin`, and optional `turn_id` / `parent_event_id`. Origin records `kind`, `host`, `invocation`, `intercepted`, and `user_shell_mutated`.
- Capability negotiation through `tfy runtime-capabilities` and `tfy runtime-negotiate`.
- Structured Tool Gateway modes: `tfy tool-gateway --json`, `tfy tool-gateway --jsonl`, and ledger event emission. These are debug/adapter/internal surfaces and must not be forwarded into model prompts by default.
- Structured Tool Gateway payloads include `model_text` and `rendering_kind`; the `summary` field mirrors `model_text` for compatibility and is not guaranteed to be a synthesized summary.
- Shell surfaces: `tfy shell <command>` is raw passthrough convenience with no TFY summary/artifacts; `tfy shell -- <command>` is the shell adapter wrapper for runtimes that can configure command execution through a TFY wrapper while the agent still expresses ordinary shell intent. Human UX has a distinct `tfy human shell` managed-session surface with human-managed-session provenance, not agent/generic-shell provenance.
- Context Gateway CLI: `tfy context-gateway <path> <scope>` returning compact/full context decisions in a runtime envelope.
- Output Gateway CLI: `tfy output-gateway --payload <json>` restores/validates structured compact code; `tfy restore-display --payload <json>` produces display-only readable code; `tfy output-gateway --apply` applies only proof-gated single-file selected-scope replacements; `tfy workspace validate/apply` handles exact multi-file modify/add/delete/rename/move plans behind per-op proof, base-hash, and plan-hash gates.
- State Gateway CLI: `tfy state-append` and `tfy state-project` over an append-only JSONL event ledger.

Support boundaries:

- Supported AI routing is explicit host routing through `tfy agent`, `tfy adapter`, or MCP (`tfy mcp serve`).
- Private/hidden Codex prompt hooks are not claimed.
- Editor auto-integration and provider/API request proxying are out of scope.
- Exact multi-file modify/add/delete/rename/move apply and conservative unique-anchor fuzzy apply with required base and preview proof hashes exist behind explicit authority, provenance, per-operation proof, base-hash, preview-hash, and plan-hash gates.

### Runtime gateway lifecycle

```text
adapter capabilities -> negotiation -> RuntimeEnvelope<GatewayRequest>
  -> gateway execution -> RuntimeEnvelope<GatewayResponse>
  -> GatewayEvent append -> State Gateway projection
  -> fallback/raw/full expansion when confidence or provenance is insufficient
```

State projections are never authoritative unless they cite source event ids and validation status. Missing lineage produces non-authoritative projection metadata and must trigger fallback rather than silently replacing evidence.

## Adapter v1: generic-shell command-boundary interception

Implemented adapter v1 packages the existing Tool Gateway behavior into a host-configurable command wrapper:

```sh
tfy adapter capabilities
tfy adapter install --target generic-shell --dry-run
tfy adapter run --session <id> -- <ordinary command>
tfy adapter report --session <id>
```

An AI agent still intends to run the ordinary command. The host runtime or wrapper invokes `tfy adapter run` instead of executing the command directly. TFY then stores raw output, selects the model-visible text through the no-negative-savings gate, appends an internal ledger event with `command_family`, preserves the original exit code, and exposes session plus family-level savings through `adapter report`.

Support claim boundary:

- `generic-shell` command-boundary interception: implemented and tested.
- Codex/OMX command wrapping: only supported where the host is explicitly configured to call the generic-shell adapter.
- MCP stdio tool/resource integration: implemented and tested through `tfy mcp serve`, including `tfy_scope_list`, enriched `tfy_context_get`, preview-only `tfy_output_validate`, and proof-gated `tfy_output_apply`.
- Product UX lifecycle: implemented through `tfy init`, `tfy doctor`, `tfy smoke`, and `tfy gain`. These commands make setup, diagnostics, local MCP smoke, and savings reporting easier while keeping the same host-routing boundary.
- Configured AI-agent wrapper: implemented through `tfy agent run`; lifecycle `tfy start --agent` now auto-configures the implemented safe Codex project MCP route unless `--no-apply` is passed; Claude Code and Cursor writers are explicit host/all routes, but it does not prove host invocation or launch support; effective `active=true` is derived only from verified host/route evidence and savings, not config presence.
- Human lifecycle: `tfy start --human` records managed-session intent and, on supported Linux bash v1 interactive project-only runs, immediately enters a TFY-managed project-scoped shell session with a generated `.tfy/human/bin` PATH shim for ordinary external commands. It still keeps `ordinary_terminal_interception=false` because ordinary terminals outside that managed session are not globally intercepted. Supported sessions report `session_wrapper_available=true`, `managed_session_available=true`, `managed_session_scope=project_scoped_tfy_managed_session`, and wrapped-command evidence in the TFY ledger; unsupported non-Linux platforms report `managed_session_available=false` with `support_status=managed_session_unsupported_platform`. `tfy setup --human` is the short dry-run for the explicit one-time bash rc hook, and `tfy setup --human --apply` installs that hook for trusted TFY-marked repos by delegating to the same safety path as `tfy human auto-activate install`; npm install prints only opt-in guidance and never mutates rcfiles. `tfy human shell --no-auto-intercept` keeps only the managed shell environment without PATH-shim routing, and `tfy human install --dry-run|--output <path>` generates the TFY-owned sourceable script. Shell-local functions, aliases, builtins, direct paths, explicit `tfy-human-bypass`, TFY gateway, and outside-scope commands run raw without summary claims; automatic interactive/TUI/stateful subcommand classification is not claimed in v1. Wrapped commands expose TFY ledger evidence and preserve wrapper exit status.
- Private Codex hook, editor auto-integration, and provider/API prompt gateway: not claimed; editor/provider are outside TFY scope.

## Adapter v2: MCP/Codex setup foundation

`tfy mcp serve` lets an MCP-aware agent host call TFY at the tool/resource boundary:

```bash
tfy mcp serve --session <id> --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw
```

The host still decides to route through MCP. TFY does not secretly intercept every Codex command. The supported Codex path is:

```bash
tfy mcp install --target codex --dry-run
```

which prints a concrete `codex mcp add tfy -- tfy mcp serve ...` command and an equivalent TOML snippet. This keeps setup reversible and prevents false claims about private runtime hooks.

The product-facing Codex path is:

```bash
tfy init --codex --dry-run
tfy init --codex --project --apply
tfy doctor --codex
tfy smoke --mcp
tfy smoke --codex
tfy setup --ai --codex --dry-run
tfy status --json
tfy explain
tfy gain # reports no-data until adapter/tfy_tool_run command events exist
```

`tfy init --codex --project --apply` writes only a TFY-owned marker block in `AGENTS.md`. `tfy setup --ai --host <host> --dry-run` and `tfy mcp install --target <host> --dry-run` generate host-specific MCP setup snippets for Codex, Claude Code, Cursor, OpenCode, and Hermes; `tfy setup --ai --host codex --apply --project`, `tfy setup --ai --host claude-code --apply --project`, and `tfy setup --ai --host cursor --apply --project` are safe project writers and preserve unrelated host config with backup, provenance, idempotency, and TFY-only uninstall. Other JSONC/YAML/TOML config writing remains dry-run/manual until a reversible writer exists. Unsupported hosts must not be silently marked configured by `tfy start --agent`. `tfy smoke --codex` and `tfy smoke --host <host>` remain checklist/evidence-collection surfaces and do not claim that the host invoked TFY; `tfy smoke --mcp` is the automated local proof that TFY's MCP Code I/O path works. `tfy smoke --all` adds local generic-shell and agent-wrapper smoke evidence. `tfy launch-report` exposes the v1 host matrix, uses the canonical statuses `config_snippet_available`, `not_configured`, `applied_unverified`, `verified_local_mcp`, `verified_host_invocation`, `launch_supported`, `unsupported`, and `planned_discovery`; lifecycle status additionally uses `configured_unverified` for safe config writes that have no host invocation evidence, plus claim tiers, and requires route-specific artifact-backed host setup, real-invocation, config path, ledger/raw evidence, no-negative/positive savings, and overhead evidence before named or required routes become `launch_supported`. Setup success is not savings success. `tfy gain` reports command-output savings only after adapter or MCP `tfy_tool_run` command events exist.
