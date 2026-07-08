# TFY Agent Middleware

## Purpose

## Boundary model

The gateway model is the external runtime integration model. It does not replace TFY's internal architecture spine:

## Gateway-to-registry crosswalk

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

## Output Gateway: preview/validate plus proof-gated apply

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

## Agent route foundation

Bare `tfy start --agent` prepares the product-facing command routes by writing `.tfy/agent/tfy-agent-wrapper` and the safe project-local Codex plus Claude Code official PreToolUse Bash hook configs. If the user later uses Codex in that project, the Codex hook route is ready; if they later use Claude Code, the Claude Code hook route is ready. Human and agent routes share the same raw-first/no-negative/custom command-summary pipeline; the difference is only where the command boundary is entered.

Setup remains `configured_unverified` until a real host invocation creates route-bound evidence. `tfy start --agent --host codex` and `tfy start --agent --host claude-code` remain explicit/narrow setup paths.

The product-facing setup path is:
