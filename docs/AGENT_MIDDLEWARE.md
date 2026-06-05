# TFY Agent Middleware

## Purpose

TFY is intended to sit inside an AI-agent runtime as an **I/O middleware**, not merely as a command a human types manually. The Rust CLI remains the debug/protocol surface, but the product path is that shells, MCP tools, editor integrations, Codex-style runtimes, or provider adapters route high-token boundaries through TFY automatically.

Current status: the Rust core, CLI primitives, `tfy-runtime` wire contract, Tool/Shell Gateway surfaces, Context Gateway CLI, Output Gateway preview/validate CLI, and State Gateway ledger/projection CLI are implemented. Codex/MCP/editor/provider automatic hooks and Output Gateway workspace apply remain adapter-specific follow-up work.

## Boundary model

The gateway model is the external runtime integration model. It does not replace TFY's internal architecture spine:

- Internal spine: representation ladder, token-saving method registry, agent-neutral Rust core.
- External boundaries: Tool Gateway, Context Gateway, Output Gateway, State Gateway.
- Runtime connectors: shell proxy, MCP proxy, Codex hook, editor integration, or provider adapter.

## Gateway-to-registry crosswalk

| Gateway boundary | Runtime question | Current TFY primitives | Method families | Owner | Status | Fallback |
|---|---|---|---|---|---|---|
| Tool Gateway | What happens when an agent runs an ordinary command/tool? | `tfy tool-gateway`, `tfy run`, `tfy raw`, `RawStore`, `ToolPolicy` | tool feedback compression, raw refs, output fingerprinting, error clustering, Git/GitHub harness, redaction | Rust core + shell/tool wrapper adapter | Core primitive and CLI entrypoint implemented | `raw_ref`, ranged raw expansion, exit/risk/evidence preservation |
| Context Gateway | What context reaches the model before reasoning? | `tfy context-gateway`, `tfy index`, `expand`, `full`, `decide-context` | semantic skeletons, compact code/maps, retrieval budget planner, adaptive compactness, dependency-neighborhood slicing, refs/deltas | Rust core + runtime envelope; external runtime adapters later | Runtime-facing CLI implemented; Codex/MCP/editor hooks planned | related/full fallback on low confidence, diagnostics, unresolved symbols |
| Output Gateway | What happens after the model emits compact code or patches? | `tfy output-gateway`, `tfy restore`; future explicit apply API | deterministic restoration, patch/edit-script output, compact schemas, validation gates | Rust core + runtime envelope; apply adapter later | Preview/validate CLI implemented; workspace apply planned behind authority/provenance gates | reject unmapped/stale symbols; request full/context fallback before apply |
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

The AI agent receives a compact summary and `raw_ref`; the exact stdout/stderr remains local behind the raw ref.

`tfy run -- <command>` remains a compatibility/debug spelling. `tfy tool-gateway -- <command>` is the runtime-facing name that communicates the intended integration boundary.

### Tool Gateway contracts

- Accept ordinary argv and run the same command.
- Preserve command exit code.
- Store exact stdout/stderr locally behind `raw_ref`.
- Redact public credential-bearing URLs while preserving original raw output locally.
- Classify success/unknown/critical conservatively.
- Preserve Git/GitHub evidence for dirty state, conflicts, failed checks, review blockers, auth/rate-limit/security output.
- Keep raw/ranged expansion available through `tfy raw`.

## Context Gateway: implemented CLI, external hooks planned

The Context Gateway mediates model input. It should choose the cheapest safe representation:

```text
index/skeleton -> selected compact scope -> related neighborhood -> full fallback
```

It uses existing primitives (`index`, `expand`, `full`, `decide-context`) through `tfy context-gateway` and returns a runtime envelope. It must not claim automatic model-context interception for Codex/MCP/editor/provider runtimes until concrete adapters implement those hooks.

## Output Gateway: preview/validate first

The first Output Gateway release handles structured, apply-ready outputs in preview/validate mode:

- compact code payloads produced with TFY symbol maps
- compact patches/edit scripts with explicit scope IDs and map refs
- restore/validate/apply-ready outputs where failure can become a fallback request

It must not rewrite arbitrary natural-language model responses, and it must not mutate the workspace until an apply adapter proves authority, provenance, preview, and validation gates.

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
- Mandatory envelope metadata: `protocol_version`, `adapter_kind`, `adapter_version`, `supported_gateways`, `authority_mode`, `session_id`, `request_id`, `trace_id`, `policy`, `provenance`, and optional `turn_id` / `parent_event_id`.
- Capability negotiation through `tfy runtime-capabilities` and `tfy runtime-negotiate`.
- Structured Tool Gateway modes: `tfy tool-gateway --json`, `tfy tool-gateway --jsonl`, and ledger event emission.
- Shell adapter wrapper: `tfy shell -- <command>` for runtimes that can configure command execution through a TFY wrapper while the agent still expresses ordinary shell intent.
- Context Gateway CLI: `tfy context-gateway <path> <scope>` returning compact/full context decisions in a runtime envelope.
- Output Gateway CLI: `tfy output-gateway --payload <json>` restoring and validating structured compact code as preview-only v1.
- State Gateway CLI: `tfy state-append` and `tfy state-project` over an append-only JSONL event ledger.

Still adapter-required:

- Codex hook integration.
- MCP server/proxy integration.
- Editor integration.
- Provider/cache-specific prompt layout integration.
- Output Gateway workspace mutation beyond preview/validate; apply requires explicit authority, provenance, and validation gates.

### Runtime gateway lifecycle

```text
adapter capabilities -> negotiation -> RuntimeEnvelope<GatewayRequest>
  -> gateway execution -> RuntimeEnvelope<GatewayResponse>
  -> GatewayEvent append -> State Gateway projection
  -> fallback/raw/full expansion when confidence or provenance is insufficient
```

State projections are never authoritative unless they cite source event ids and validation status. Missing lineage produces non-authoritative projection metadata and must trigger fallback rather than silently replacing evidence.
