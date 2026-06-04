# PRD: TFY — Token-Saving AI Code Interface

## 1. Product Definition

**TFY** is a deterministic code-context compression interface for AI coding workflows.

Its purpose is to reduce token usage by separating:

1. **Human-readable code** — formatted, meaningful names, normal source files.
2. **AI-facing compact code** — minimal whitespace/newlines and short symbols such as `f1`, `a`, `b`.
3. **Semantic name maps** — deterministic 1:1 mappings from compact symbols back to original meaningful names.
4. **Names-first context discovery** — the AI initially sees semantic function/scope names, then requests only the code regions it needs.

TFY is not merely a minifier. It is a **semantic index + compact representation + on-demand context expansion + restoration pipeline**.

## 2. User Intent

The user wants AI coding agents to operate with fewer tokens while retaining enough semantic understanding to make correct edits.

The key idea is:

- AI does **not** need pretty formatting or long identifiers in the code body all the time.
- But AI may need the semantic meaning behind `f1`, `a`, `b`.
- Therefore TFY gives the AI compact code plus optional, deterministic mappings to original meaningful names.
- Instead of sending all code up front, TFY first exposes a list/index of meaningful function or scope names.
- The AI chooses which scope it wants to inspect; TFY then sends only that compact code plus the relevant mapping.
- If the task is ambiguous or broad, TFY may send larger neighborhoods or full context as a safety fallback.

## 3. Core Workflow

### 3.1 Index Phase

TFY parses/indexes a project and divides it into addressable scopes, such as:

- files
- modules
- classes/types
- functions/methods
- blocks or local scopes when useful

For each scope, TFY tracks:

- original semantic name
- compact symbol ID
- path/location
- signature/type information when available
- relationships: callers, callees, imports, references, tests if available
- local identifier map for compact bodies

### 3.2 Initial AI Context

Instead of sending full source code, TFY first sends a **semantic index** using original long names, not compact names.

Example:

```text
Available functions:
- calculateUserDiscount
- validatePaymentMethod
- createOrderSummary
- syncInventoryAfterOrder
- sendOrderConfirmationEmail
```

The goal is that the AI can infer likely relevance from meaningful names before reading bodies.

### 3.3 On-Demand Scope Request

The AI requests scopes by semantic name or stable ID.

Example:

```text
Show calculateUserDiscount and validatePaymentMethod.
```

TFY then provides compact bodies, not pretty source.

Example:

```js
f1(a,b){c=f2(a);return f3(c,b)}
f4(a){return a&&a.p&&f5(a.p)}
```

### 3.4 One-Time Semantic Mapping

For each provided compact region, TFY sends the relevant 1:1 mapping once, or sends it on demand.

Example:

```json
{
  "f1": "calculateUserDiscount",
  "a": "userProfile",
  "b": "orderItems",
  "c": "membershipLevel",
  "f2": "getMembershipLevel",
  "f3": "applyDiscount",
  "f4": "validatePaymentMethod",
  "f5": "isSupportedPaymentProvider"
}
```

This mapping is deterministic and produced by TFY, not inferred by AI.

### 3.5 AI Output

The AI outputs compact code/patches using the same compact symbols and minimal formatting.

TFY applies the patch to compact representation, maps it back to human-readable code, restores names, formats the result, and validates the transformation.

### 3.6 Expansion and Fallback

If the AI is unsure, or if the requested task likely depends on hidden context, TFY expands progressively:

```text
semantic names
-> signatures / structure
-> selected compact bodies + local map
-> related callers/callees/types/tests
-> full relevant file/module
-> full context fallback
```

Full context fallback is not a failure. It is a correctness safety mechanism.

## 4. In Scope

- Compact AI-facing code representation:
  - minimal indentation
  - minimal newlines
  - short function/variable/type symbols
- 1:1 symbol mapping:
  - compact symbol -> original semantic name
  - original semantic name -> compact symbol
  - scope-aware to avoid collisions
- Names-first semantic index:
  - original long function/scope names shown before bodies
  - body disclosure only when requested or required
- On-demand context expansion:
  - selected scopes
  - related scopes
  - full context fallback
- Deterministic restoration:
  - compact patch -> readable source
  - original names restored
  - formatting restored via deterministic formatter where possible
- Evaluation:
  - token savings
  - AI task accuracy/performance degradation
  - expansion frequency
  - fallback frequency
  - restoration correctness

## 5. Out of Scope / Non-goals

- Do not use AI to create or restore the symbol mapping.
- Do not assume names-only context is always enough for edits.
- Do not treat aggressive compression as always better than correctness.
- Do not hide full context when ambiguity or correctness risk is high.
- Do not bind TFY to one AI vendor or agent tool.
- Do not claim multi-language safety without language-specific parsing/validation.

## 6. Design Principles

1. **Correctness fallback beats compression purity**
   - If compact context is insufficient, TFY must expand.

2. **Meaning should be available, not always inline**
   - Original names are semantic hints. They can be shown via index/map instead of embedded everywhere in bodies.

3. **AI-facing representation can be ugly; human-facing output must be readable**
   - Compact code is an internal transport format.

4. **Mappings must be deterministic**
   - AI may consume maps but should not invent them.

5. **Measure net savings, not first-prompt savings**
   - Count expansions, retries, and fallback cost.

## 7. Key Risks

### 7.1 Missing Needed Code

The AI may choose the wrong function from names alone or miss a hidden dependency.

Mitigation:
- expose meaningful names first
- include signatures/relationships where cheap
- let AI request more context
- detect ambiguity/risk and expand
- allow full context fallback

### 7.2 Semantic Loss from Short Names

Compact symbols remove human-readable hints.

Mitigation:
- provide local mapping once per compact region
- keep semantic index visible
- optionally preserve names for public APIs or high-risk code

### 7.3 Mapping Token Cost

Mappings can erase token savings if sent too often.

Mitigation:
- send maps only for provided scopes
- cache/reuse maps within session
- support “map-on-demand”
- measure net token cost

### 7.4 Patch Restoration Failure

Compact AI output may not map cleanly back to source.

Mitigation:
- stable IDs and scope-aware maps
- parse/validate compact patches
- run formatter and diff validation
- reject ambiguous patches

## 8. Acceptance Criteria

A future TFY prototype or product should demonstrate:

1. It can build a semantic index of functions/scopes with original names.
2. It can generate compact bodies with short symbols and minimal formatting.
3. It can emit a scope-local 1:1 map for all compact symbols used in a provided body.
4. It can restore a no-op compact roundtrip to readable code.
5. It can provide names-first context and then selected function bodies on demand.
6. It can expand to related context or full context when requested/ambiguous.
7. It can report token savings at each level.
8. It can report AI performance degradation or task success/failure in evaluation scenarios.

## 9. RALPLAN-DR Summary

### Principles

- Preserve correctness through expansion/fallback.
- Keep semantic meaning accessible via names/maps, not always inline.
- Treat compact code as transport, not human product.
- Keep transforms deterministic and scope-aware.
- Measure net token savings after all expansions.

### Decision Drivers

1. Reduce token usage without unacceptable AI accuracy loss.
2. Prevent missing-needed-code failures.
3. Keep TFY agent-tool agnostic and reversible.

### Viable Options

#### Option A: Pure compact-code minifier

Pros:
- easiest to build
- immediate token reduction

Cons:
- high semantic loss
- does not solve missing-needed-code
- weaker product differentiation

Rejected as insufficient alone.

#### Option B: Names-first selective disclosure with compact bodies

Pros:
- matches user intent
- strongest token-saving potential
- preserves semantic discovery through original names
- supports expansion/fallback

Cons:
- needs indexing and request protocol
- correctness depends on expansion policy

Recommended.

#### Option C: Always include full maps and broad context

Pros:
- safer for AI comprehension
- fewer missing-context failures

Cons:
- token savings may disappear
- less aligned with TFY purpose

Rejected as default; acceptable as fallback/safe mode.

## 10. ADR

### Decision

Define TFY as a **names-first selective context system with compact AI-facing code bodies and deterministic 1:1 semantic symbol maps**.

### Drivers

- Token reduction must not blindly sacrifice AI comprehension.
- Original function/variable names are useful semantic hints.
- Sending all code up front is wasteful.
- The AI needs a safe path to request more code.

### Alternatives Considered

- Simple minifier only: rejected because it ignores semantic/context risks.
- Full context with shorter names only: rejected because savings may be too small.
- AI-generated summaries: rejected for the transform/restoration layer because TFY should be deterministic.

### Consequences

- TFY must implement an index/request/expansion protocol, not just a formatter.
- Evaluation must include net token savings and accuracy/fallback behavior.
- Language adapters and scope-aware maps become core architecture.

### Follow-ups

- Build detailed architecture for indexer, compact generator, map store, expansion policy, restoration, and evaluator.
- Decide first implementation language path.
- Define benchmark tasks for missing-needed-code and semantic-loss failures.

---

# Revision 1: Critic-Gated Clarifications

## 11. Serious Alternative: Names-First Selective Context with Light/Readable Bodies

The strongest alternative to aggressive TFY compact bodies is:

> Use the same names-first semantic index and on-demand disclosure protocol, but provide readable or lightly compacted bodies instead of fully short-symbol compact bodies by default.

### Option D: Names-first selective context with readable/lightly compacted bodies

Pros:
- Preserves much of the value from not sending the whole project.
- Keeps original semantic names inside selected bodies, improving AI comprehension.
- Reduces restoration/mapping complexity.
- Provides a strong baseline to test whether aggressive compact bodies are actually worth it.

Cons:
- Lower token savings inside selected bodies.
- Does not fully realize the original TFY idea of AI-facing compact code.
- May underperform on very verbose code where identifier shortening gives large savings.

### Mode Selection Rule

TFY should not force one body representation globally. It should support modes:

1. **Readable selective mode**
   - names-first index
   - selected readable bodies
   - safest comprehension baseline

2. **Light compact mode**
   - remove nonessential formatting
   - preserve original semantic identifiers
   - middle ground for token reduction with lower semantic loss

3. **Aggressive compact mode**
   - minimal formatting
   - short local symbols
   - requires local 1:1 mapping
   - best when measured net savings justify comprehension/restoration risk

4. **Full context safe mode**
   - original readable context
   - used for ambiguity, high-risk edits, failed validation, or explicit request

The recommended TFY architecture remains compatible with all modes. The product decision is not “always aggressive compaction”; it is “choose the smallest context representation that remains safe enough for the task.”

## 12. Map Semantics / Collision Rules

TFY mappings are deterministic but **scope-relative**, not globally naive.

### 12.1 Namespaces

Each compact symbol belongs to a namespace:

- `fn` for functions/methods
- `var` for local variables/parameters
- `type` for classes/types/interfaces/structs/enums
- `mod` for modules/files
- `field` for fields/properties when safely transformable

Compact symbols may be reused across independent scopes only if the scope ID disambiguates them.

Example:

```json
{
  "scope":"src/order.ts::calculateUserDiscount",
  "symbols":{
    "fn:f1":"calculateUserDiscount",
    "var:a":"userProfile",
    "var:b":"orderItems"
  }
}
```

### 12.2 Stable IDs

Every scope receives a stable identity based on path + qualified original name + structural position/hash.

- Renaming a local variable should not change the function scope ID.
- Moving a function may change path-qualified ID unless TFY can prove continuity.
- Compact symbol assignment should be deterministic for unchanged scope content.

### 12.3 Shadowing

If original code has shadowed names, TFY must preserve distinct bindings.

- Same original spelling in nested scopes maps to distinct scoped entries.
- Compact symbols may restart per nested scope if the map encodes scope boundaries.
- Restoration must never merge shadowed bindings accidentally.

### 12.4 Public and Unsafe Symbol Preservation

TFY must not rename symbols when renaming can affect external behavior unless a language adapter proves safety.

Preserve or mark non-renamable:

- public API exports
- reflection/string-key referenced names
- serialized field names
- macro-generated or macro-consumed identifiers
- dynamic dispatch hooks
- framework lifecycle method names
- generated code

### 12.5 Deterministic Ordering

Symbol assignment order should be stable:

1. scope owner/function symbol
2. parameters in source order
3. local declarations in source order
4. referenced helper functions in deterministic reference order
5. types/imports in deterministic source/reference order

### 12.6 Map Reuse

A map may be sent once per AI session/scope and then referenced by stable map ID.

Example:

```json
{
  "map_id":"map:src/order.ts::calculateUserDiscount:v3",
  "symbols":{ "var:a":"userProfile" }
}
```

Subsequent compact bodies may reference `map_id` instead of resending identical mappings.

## 13. Expansion Trigger Policy

TFY expands context through explicit and automatic triggers.

### 13.1 Selected Body Trigger

Return a selected compact/readable body when:

- AI explicitly requests a function/scope by semantic name or ID.
- Task directly names a function/scope.
- Static retrieval scores a scope as highly relevant.

### 13.2 Related Neighborhood Trigger

Expand to callers/callees/types/tests when:

- selected scope calls another project-local function used in the changed logic
- selected scope depends on a type whose fields/invariants affect logic
- edit touches error handling, state mutation, permissions, validation, money, auth, or persistence
- tests exist for selected scope
- AI requests “related code,” “dependencies,” or “where this is used”

### 13.3 Full File/Module Trigger

Expand to the full file/module when:

- multiple nearby scopes are requested
- imports/re-exports/module-level state affect behavior
- local scope boundaries are ambiguous
- language adapter cannot safely isolate a scope
- compact patch fails parse/restore validation inside the selected scope

### 13.4 Full Context Fallback Trigger

Fallback to broader/full context when:

- AI says it is unsure or needs full context
- static analysis cannot identify dependencies confidently
- tests/static checks fail after a compact-context patch
- the task is broad architectural refactor rather than local edit
- missing-needed-code is detected in benchmark/evaluation loop
- safety mode is explicitly selected

### 13.5 Trigger Ownership

Expansion may be triggered by:

1. AI request
2. TFY static detector
3. validation/test failure
4. user/tool policy mode

TFY must record which trigger caused each expansion for evaluation.

## 14. Evaluation Baseline

### 14.1 Tokenizer Baselines

At minimum, TFY reports token counts using:

- a project-configured tokenizer if available
- one default tokenizer selected for the target AI workflow
- raw byte/character count as a tokenizer-independent fallback metric

Exact tokenizer choice may be deferred to implementation, but every evaluation report must name the tokenizer used.

### 14.2 Comparison Modes

TFY must compare at least:

1. full readable source/context
2. names-first + readable selected bodies
3. names-first + lightly compacted selected bodies
4. names-first + aggressive compact bodies + local maps
5. aggressive compact with expansion/fallback costs included

### 14.3 Benchmark Task Categories

Evaluation should include tasks where:

- function name alone is enough to select scope
- selected body is enough to complete edit
- related callee/caller is needed
- hidden type invariant is needed
- full file/module is needed
- aggressive short symbols harm comprehension
- local map recovers comprehension
- fallback recovers a missing-needed-code failure

### 14.4 Minimum Reported Metrics

Every evaluation run reports:

- initial token count
- final net token count after expansions
- expansion count and expansion levels used
- fallback frequency
- task success/failure
- restoration success/failure
- ambiguous patch rejection count
- comparison against readable/light selective baseline

### 14.5 Threshold Policy

Fixed success thresholds are deferred until empirical data exists. However, reports must expose enough metrics to decide whether aggressive compact mode is better than readable/light selective mode for a task class.

## 15. Updated Acceptance Gates

A future TFY implementation must:

1. Pass no-op roundtrip for selected supported language fixtures.
2. Reject ambiguous compact patches rather than silently corrupting source.
3. Report fallback frequency and expansion triggers.
4. Compare aggressive compact mode against less-aggressive names-first selective-context baseline.
5. Preserve or mark non-renamable public/unsafe symbols.
6. Demonstrate scope-aware collision-safe maps, including shadowing cases.
7. Report net token savings after all expansions, not just initial prompt savings.

---

# Revision 2: Command Output Compression Addendum

TFY should also support a **Command Output Compressor / Tool Feedback Budgeter** for AI-executed command outputs such as git, tests, build/typecheck logs, and shell results.

## Policy

```text
important/error output -> preserve evidence, compress only formatting/whitespace
success/no-action output -> aggressive compression/summarization
all output -> raw original retained for fallback/audit
```

## Requirements

- Store full raw command output with a stable raw output reference.
- Preserve actionable error evidence: file, line, symbol, exit code, stack frame, failing assertion, diagnostic message.
- Aggressively compress successful/no-action/no-signal output.
- Report omitted lines/counts/categories.
- Allow AI/tool to request raw output or selected raw ranges by reference.
- Evaluate token savings and missed-signal risk.
