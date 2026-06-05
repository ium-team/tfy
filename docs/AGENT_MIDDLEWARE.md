# TFY Agent Middleware

## Purpose

TFY is intended to sit inside an AI-agent runtime as an **I/O middleware**, not merely as a command a human types manually. The Rust CLI remains the debug/protocol surface, but the product path is that shells, MCP tools, editor integrations, Codex-style runtimes, or provider adapters route high-token boundaries through TFY automatically.

Current status: the Rust core and CLI primitives are implemented. The first runtime-facing interception boundary is exposed as the Tool Gateway CLI entrypoint (`tfy tool-gateway -- <command>`), while automatic shell/MCP/Codex/provider hooks and deeper context/output/state runtime adapters remain planned.

## Boundary model

The gateway model is the external runtime integration model. It does not replace TFY's internal architecture spine:

- Internal spine: representation ladder, token-saving method registry, agent-neutral Rust core.
- External boundaries: Tool Gateway, Context Gateway, Output Gateway, State Gateway.
- Runtime connectors: shell proxy, MCP proxy, Codex hook, editor integration, or provider adapter.

## Gateway-to-registry crosswalk

| Gateway boundary | Runtime question | Current TFY primitives | Method families | Owner | Status | Fallback |
|---|---|---|---|---|---|---|
| Tool Gateway | What happens when an agent runs an ordinary command/tool? | `tfy tool-gateway`, `tfy run`, `tfy raw`, `RawStore`, `ToolPolicy` | tool feedback compression, raw refs, output fingerprinting, error clustering, Git/GitHub harness, redaction | Rust core + shell/tool wrapper adapter | Core primitive and CLI entrypoint implemented | `raw_ref`, ranged raw expansion, exit/risk/evidence preservation |
| Context Gateway | What context reaches the model before reasoning? | `tfy index`, `expand`, `full`, `decide-context` | semantic skeletons, compact code/maps, retrieval budget planner, adaptive compactness, dependency-neighborhood slicing, refs/deltas | Rust core + agent context adapter | Core primitives implemented; runtime adapter planned | related/full fallback on low confidence, diagnostics, unresolved symbols |
| Output Gateway | What happens after the model emits compact code or patches? | `tfy restore`; future patch/apply API | deterministic restoration, patch/edit-script output, compact schemas, validation gates | Rust core + apply/runtime adapter | Restore primitive implemented; structured patch gateway planned | reject unmapped/stale symbols; request full/context fallback before apply |
| State Gateway | What persists across turns without raw-history bloat? | future ledger/ref commands; raw refs as evidence handles | task-state compaction, output fingerprinting, local memoization, refs/deltas | Rust schema + runtime/session adapter | Schema-first planned; event-fed from other gateways | preserve decisions/evidence/raw refs; request raw transcript/context on low confidence |

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

## Context Gateway: planned runtime API

The Context Gateway mediates model input. It should choose the cheapest safe representation:

```text
index/skeleton -> selected compact scope -> related neighborhood -> full fallback
```

It should use existing primitives (`index`, `expand`, `full`, `decide-context`) and expose a runtime-facing context request API later. It must not claim automatic model-context interception until a concrete adapter implements it.

## Output Gateway: structured only first

The first Output Gateway release should handle only structured, apply-ready outputs:

- compact code payloads produced with TFY symbol maps
- compact patches/edit scripts with explicit scope IDs and map refs
- restore/validate/apply-ready outputs where failure can become a fallback request

It must not rewrite arbitrary natural-language model responses until an adapter proves that behavior safe and testable.

## State Gateway: schema-first and event-fed

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

Initial implementation can seed the ledger from Tool Gateway events; Context and Output events should join the same schema when those gateways are implemented.

## Adapter rule

Adapters connect concrete runtimes to the Rust core. They may automate invocation, layout, caching, or transport, but they cannot weaken TFY correctness semantics:

- no hiding critical evidence
- no dropping raw/full fallback
- no provider-specific behavior required for correctness
- no claim of token savings without evaluation gates
