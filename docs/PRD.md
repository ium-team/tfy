# PRD: TFY Standalone Product Definition

## Goal

Build TFY as an independent token-efficient AI work interface.

TFY should reduce tokens across:

1. code context
2. AI code output
3. command/tool feedback
4. raw/full context expansion
5. human-readable restoration

The target is not a throwaway MVP. The first public shape should be planned as a release-grade product surface with measurable savings, correctness safeguards, raw fallback, and agent-tool compatibility from the beginning.

## Product Constraints

- **Independent identity:** TFY may study RTK, especially for command-output filtering, but the TFY product boundary is wider than RTK and must not be described as an RTK fork.
- **Broad language support:** TFY should aim for as many programming languages as practical by using parser/indexer abstractions and language adapters rather than a single-language-only design.
- **Agent-tool compatibility:** TFY should work regardless of which AI coding agent/tool the user runs. The interface should expose compact context, expansion requests, compact patches, command summaries, and raw refs in a tool-neutral way.
- **No AI guessing for restoration:** symbol maps, name restoration, and formatting are deterministic TFY responsibilities.
- **Safety over savings:** if compact context risks hiding needed code or important command output, TFY expands context/raw output.

## In Scope

- semantic name index
- compact code representation
- deterministic scope-aware 1:1 symbol maps
- on-demand code expansion
- compact AI output / patch format
- readable code restoration
- risk-aware command output compression
- raw command output references and expansion
- token savings evaluation
- AI performance degradation evaluation

## Out of Scope

- Becoming a direct RTK fork/clone as the whole product identity.
- Using AI to guess symbol maps or restoration.
- Hiding important errors just to save tokens.
- Assuming compact context is always enough.

## Product Modes

### Code Context Mode

AI receives semantic names first, then selected compact code and maps.

### Tool Feedback Mode

AI receives compressed command feedback first, then raw output on demand.

### Safe Fallback Mode

TFY provides full code context or raw output when ambiguity/risk is high.

## RALPLAN-DR Summary

### Principles

1. Reduce tokens without hiding decision-critical evidence.
2. Keep semantic meaning recoverable through deterministic, scope-aware maps.
3. Prefer names-first and summaries-first context before body/raw expansion.
4. Stay tool-neutral and language-adapter-friendly.
5. Preserve TFY's broader identity instead of collapsing into RTK-like command filtering.

### Decision Drivers

1. **Net token savings:** savings must remain positive after map/context/raw expansions.
2. **Correctness retention:** AI task success must not degrade beyond an explicit measured threshold.
3. **Recoverability:** every compact artifact must have a deterministic path back to readable code or raw evidence.

### Viable Options

#### Option A: TFY as a full AI work-context protocol

- Pros: covers code input, code output, tool feedback, restoration, and evaluation in one product identity.
- Cons: larger architecture and more surfaces to validate.

#### Option B: TFY as command-output compression first

- Pros: easier to ground from RTK and easier to ship quickly.
- Cons: contradicts the user's broader idea and risks becoming an RTK clone.

#### Option C: TFY as code-minifier/restorer only

- Pros: focuses on the novel compact-code idea.
- Cons: misses command/tool token waste and agent workflow integration.

#### Option D: Names-first selective context with lightly compacted readable bodies

- Pros: less AI comprehension loss than aggressive `f1/a/b` symbol shortening; easier debugging and early validation.
- Cons: lower token savings than the user's more aggressive compact-code proposal.

Chosen direction: **Option A**, while using Option B/RTK as a reference lane for the tool feedback subsystem and Option C as the code representation subsystem.

Within Option A, TFY should support multiple compactness levels:

1. **Light compact:** preserve readable names where useful, remove only obvious formatting/noise.
2. **Symbol compact:** use short symbols such as `f1`, `a`, `b` with deterministic maps.
3. **Fallback/full:** restore readable names or provide full context when comprehension risk rises.

## Acceptance Criteria

- TFY can represent code in compact form.
- TFY can provide original semantic names via index/map.
- TFY can restore compact output to readable code.
- TFY can compress command output by risk tier.
- TFY can expose raw command output by reference.
- TFY can measure token savings and correctness impact.
- TFY clearly remains independent from RTK while using RTK as a reference for command-output compression.
- TFY documents release-grade expectations rather than an MVP-only shortcut.
- TFY documents broad language and agent-tool compatibility as first-class constraints.

## Risk Mitigation

### Token Savings vs AI Performance Degradation

- Measure net token savings after symbol maps, requested expansions, and raw output requests.
- Compare AI task success against a full-context baseline for the same tasks.
- Treat savings as invalid if compact mode causes unacceptable correctness degradation, even when token counts improve.
- Allow policy tuning by compactness level: light compact -> symbol compact -> fallback/full.

### Missing Needed Code

Fallback triggers:

- AI asks for symbols/scopes not present in current compact body.
- Patch touches unresolved identifiers or external call sites.
- Static analysis/test failure points outside selected scopes.
- The selected function depends on non-local state, side effects, inheritance, macros, generated code, or dynamic dispatch.

Mitigation:

- expand related neighborhood first
- then expand file/module
- then provide full context when ambiguity remains

### Missed Command Error

Fallback/raw expansion triggers:

- non-zero exit code
- stderr content
- stack traces, test failures, compiler diagnostics, file/line references
- unknown command family or unknown output format
- summary confidence below policy threshold

Mitigation:

- preserve actionable error evidence in the compact view
- keep `raw_ref` for every command output
- allow full raw or range raw expansion

### Deterministic Restoration

Validation steps:

- resolve compact symbols only through the stored scope-aware map
- reject unmapped or ambiguous compact identifiers
- restore original names before formatting
- parse restored code when parser support exists
- compare no-op roundtrips and semantic diffs before accepting patch output

## ADR

### Decision

Define TFY as a standalone AI work-context protocol and representation system.

### Drivers

- Reduce token usage beyond command outputs.
- Preserve AI understanding through semantic indexes and maps.
- Preserve correctness through expansion/fallback.
- Use RTK only as a reference for command-output compression.

### Consequences

- TFY needs architecture for code indexing, compact representation, maps, restoration, tool feedback, and evaluation.
- TFY can borrow lessons from RTK but should not inherit RTK's narrower identity.
- TFY must evaluate both compression ratio and AI work quality; token reduction alone is insufficient.

### Alternatives Considered

- **RTK-style command-output filter only:** rejected because it does not include compact code, semantic maps, on-demand source expansion, or readable restoration.
- **Pure code minifier:** rejected because command/tool outputs are also a major token source.
- **AI-generated summaries/maps:** rejected because restoration must be deterministic and auditable.

### Follow-ups

- Define language adapter interface.
- Define agent-neutral context/expansion protocol.
- Define raw output storage/reference protocol.
- Build benchmark harness for token savings and correctness degradation.

## Available Agent Types / Follow-up Staffing Guidance

- `architect`: refine protocol boundaries, language-adapter abstractions, and recovery paths.
- `dependency-expert`: evaluate parser/indexer libraries for multi-language support.
- `executor`: implement bounded prototypes after this plan exits ralplan.
- `test-engineer`: build savings/correctness benchmark harness.
- `verifier`: confirm docs, tests, and benchmark evidence before release.

Recommended execution path after planning: `$ultragoal` for durable sequential productization, optionally with `$team` for parallel lanes (`language adapters`, `tool feedback`, `benchmark harness`, `docs`). `$ralph` is only a fallback for a single-owner completion loop if explicitly selected.
