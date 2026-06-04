# Spec Addendum: TFY Token Reduction Problem — Missing Needed Code

## Metadata
- Source: `$deep-interview`
- Profile: standard
- Context type: greenfield
- Final ambiguity: ~12.5%
- Context snapshot: `.omx/context/tfy-problem-discussion-20260604T005155Z.md`
- Transcript: `.omx/interviews/tfy-problem-discussion-20260604T005155Z.md`

## Core Problem
The central concern is not whitespace minification or identifier shortening. The important risk is:

> TFY saves tokens by not showing the full code, but the AI may fail because the hidden code contained necessary information.

This is the `missing-needed-code` problem.

## Distinction: Selective Context vs Structural Summary
They overlap but are not identical.

- **Selective context**: decides *what* to include or hide.
  - Example: show `checkout()` body, show only names of related functions, hide unrelated modules.
- **Structural summary**: decides *how* to represent code that is not fully shown.
  - Example: show function signatures, names, call graph, type names, side-effect tags, or summaries instead of bodies.

For TFY, the user is most worried about the combined area: hiding code or showing only structure may omit what the AI needs.

## User-Preferred Mitigation
Use a **names-first selective disclosure policy**:

1. Initially provide function/scope names and lightweight structural information.
2. Let the AI infer from names which functions or regions it wants to inspect.
3. If the AI/tool is unsure, ambiguous, or the task looks risky, expand context.
4. If still ambiguous, fetch the whole relevant/full context instead of forcing compression.

## Practical Product Implication
TFY should not be a one-shot compressor that always hides code. It should be an **interactive context budget manager**:

- Start small.
- Expose names/signatures/structure.
- Expand on demand.
- Fall back to full context when uncertainty is high.

## Token Reduction Methods and Performance Risk

### 1. Whitespace / one-line minification
- Token savings: low to moderate.
- Accuracy risk: medium; code structure may become harder for AI to parse.
- Best use: secondary optimization, not the main strategy.

### 2. Identifier shortening
- Token savings: moderate to high in verbose code.
- Accuracy risk: high if semantic names are removed too early.
- Best use: only with reversible mapping and probably not for all identifiers.

### 3. Selective context
- Token savings: potentially highest.
- Accuracy risk: missing needed code.
- Best use: main TFY strategy, but only with safe expansion policy.

### 4. Structural summary
- Token savings: high when replacing bodies.
- Accuracy risk: summaries omit conditions, side effects, error handling, invariants.
- Best use: first-pass orientation, not final edit context when correctness matters.

## Recommended TFY Policy
A good default policy is:

```text
names first -> signatures/structure -> selected bodies -> expanded neighborhood -> full context fallback
```

Where “expanded neighborhood” means related callers, callees, types, imports, trait/interface implementations, tests, and error paths.

## Success Criteria for This Problem
TFY should be evaluated by measuring:

1. Token savings at each disclosure level.
2. How often AI asks for more context.
3. How often omitted code caused an incorrect answer/edit.
4. Whether fallback-to-full-context recovers correctness.
5. Net token savings after all expansions, not only first prompt savings.

## Decision Boundaries
OMX/planning may assume:
- Selective context and structural summary are the first problem area to design around.
- Missing needed code is the main failure mode to mitigate.
- Full context fallback is allowed when uncertainty is high.

Do not assume without confirmation:
- That TFY should always aggressively compress.
- That names-only context is enough for edits.
- That full context fallback is a failure; it is an intended safety mechanism.

## Recommended Next Handoff
Use `$ralplan` to turn this into an architecture and test plan, especially an evaluation matrix for names-first context disclosure and full-context fallback.
