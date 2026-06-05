# TFY Agent-Neutral Protocol

TFY exposes an agent-neutral protocol so any AI agent, editor, shell wrapper, or orchestration layer can use the same token-saving service.

## Current implemented CLI surface

- `tfy index <path>` — semantic scope names first.
- `tfy expand <path> <scope> [--compactness light|symbol]` — selected compact body, map, metrics.
- `tfy full <path> <scope>` — full source fallback.
- `tfy decide-context` — selected/related/full fallback decision from payload and diagnostics.
- `tfy restore` — deterministic readable restoration from compact payload.
- `tfy run -- <command...>` — risk-aware compact command feedback.
- `tfy raw <raw_ref>` — full or ranged raw command output.
- `tfy eval-code <path> <scope>` — raw vs compact token estimate and quality gate placeholder.
- `tfy languages` — language adapters.

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

- Scope IDs, not display names, are the durable selection contract.
- Command raw refs are validated, append-only, opaque evidence handles; they are not a stable content-addressing contract.
- Python layout/string literals are preserved when needed for correctness.
- Non-Python compaction preserves strings/template literals.
- Restoration rejects unmapped compact symbols.
- Provider adapter usage must report hit/miss/usage evidence when available.
