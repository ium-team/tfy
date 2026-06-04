# Test Spec: TFY Definition and Future Prototype

## Purpose

Validate the TFY concept as a token-saving AI code interface that uses:

- names-first semantic discovery
- compact AI-facing code bodies
- deterministic 1:1 symbol maps
- on-demand expansion
- full-context fallback

This test spec is for future implementation planning; no source implementation exists yet.

## Test Categories

### 1. Semantic Index Tests

- Given a source project, TFY identifies functions/scopes and emits original semantic names.
- The initial AI context contains original long names, not only compact IDs.
- Index entries include stable IDs and source locations.
- Optional metadata such as signatures/call relationships is emitted when available.

### 2. Compact Body Tests

- Given a selected function, TFY emits a compact representation with minimal whitespace/newlines.
- Local variables/functions/types are converted to short symbols where safe.
- Public or unsafe names can be preserved or marked non-renamable.
- Compact output remains parseable by TFY.

### 3. Mapping Tests

- Every compact symbol in a provided body has exactly one original-name mapping in that scope.
- Mappings are scope-aware and collision-safe.
- Mapping is deterministic across repeated runs for unchanged code.
- TFY can emit only the relevant map for selected scopes.

### 4. On-Demand Expansion Tests

- AI/tool can request a function by original semantic name.
- TFY returns only that selected compact body plus needed local mapping.
- TFY can expand to caller/callee/type/test neighborhoods.
- TFY can fall back to full context when requested or when ambiguity is high.

### 5. Restoration Tests

- No-op compact roundtrip restores readable source.
- Compact patch can be mapped back to original names.
- Formatter restores human-readable indentation/newlines.
- Ambiguous or invalid compact patches are rejected instead of silently corrupting source.

### 6. Token Measurement Tests

Measure token usage for:

1. original full source
2. names-only semantic index
3. selected compact body without map
4. selected compact body with local map
5. expanded neighborhood
6. full context fallback

Report net token savings across the whole interaction, not only first prompt.

### 7. AI Performance Evaluation

Create benchmark tasks for:

- simple local edit where names-only selection is enough
- edit needing one selected function body
- bug requiring related callee expansion
- bug requiring full context fallback
- case where compact identifiers reduce comprehension
- case where mapping restores comprehension

For each task, measure:

- task success/failure
- number of expansions requested
- final token total
- whether missing-needed-code caused failure
- whether fallback recovered correctness

## Acceptance Gates

A future implementation should not be considered successful unless:

- semantic index generation works
- compact body generation works
- symbol maps are deterministic and exact
- no-op restoration works
- on-demand expansion works
- full-context fallback works
- net token savings are measured
- AI performance degradation is measured, not assumed

## Verification Strategy

- Unit tests for symbol mapping and compact generation.
- Golden-file tests for source -> compact -> restored source.
- Integration tests for names-first request flow.
- Evaluation harness for token counts and task accuracy.
- Regression tests for missing-needed-code scenarios.

## Known Hard Cases

- macro-heavy code
- reflection/string-key based access
- public API names
- generated code
- overloaded functions/methods
- closures and nested scopes
- cross-file imports/re-exports
- dynamic dispatch or trait/interface resolution

---

# Revision 1: Critic-Gated Test Detail

## Map Semantics Test Fixtures

Create fixtures with:

- parameters and locals with the same original name in nested scopes
- two functions that both use compact `a`, proving scope-relative maps are safe
- public exports that must not be renamed
- string-key/reflection-like references that must be preserved or marked unsafe
- deterministic repeated runs over unchanged code

Expected results:

- no accidental symbol collision
- stable map IDs for unchanged scopes
- public/unsafe names preserved or annotated
- shadowed names restored correctly

## Expansion Trigger Tests

### Selected Body

Given a semantic index and a request for `calculateUserDiscount`, TFY returns only that body plus local map.

### Related Neighborhood

Given `calculateUserDiscount` calls `getMembershipLevel`, and the edit concerns membership behavior, TFY expands to include the callee.

### Full File/Module

Given module-level state or imports affect the selected function, TFY expands to file/module context.

### Full Context Fallback

Given ambiguity, explicit AI request, failed validation, or broad refactor task, TFY falls back to full relevant context and records the trigger.

## Evaluation Baseline Tests

Every benchmark compares:

1. full readable context
2. names-first + readable body
3. names-first + light compact body
4. names-first + aggressive compact body + map
5. aggressive compact + all expansions/fallbacks

Each benchmark report must include:

- tokenizer name or fallback metric
- first prompt token count
- net token count
- expansion count
- fallback count
- task success/failure
- restoration success/failure
- ambiguous patch rejection count

## Updated Acceptance Gate Tests

- No-op roundtrip passes.
- Ambiguous compact patch is rejected.
- Fallback frequency is reported.
- Aggressive compact mode is compared against readable/light selective baseline.
- Scope-aware maps handle shadowing and repeated compact symbols.
- Public/unsafe symbols are not renamed unless adapter proves safety.
