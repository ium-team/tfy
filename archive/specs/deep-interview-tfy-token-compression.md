# Spec: TFY Token Compression Feasibility + Product Direction

## Metadata
- Source: `$deep-interview`
- Profile: standard
- Context type: greenfield
- Final ambiguity: ~31.2% (threshold 20%; ended early by user request)
- Context snapshot: `.omx/context/tfy-token-compression-20260604T004238Z.md`
- Transcript: `.omx/interviews/tfy-token-compression-20260604T004238Z.md`

## Intent
Build and evaluate **TFY (token fuck you)** as a token-reducing code representation layer for AI coding workflows, related to/for users of `rtk` (rust token killer). The core idea is that AI does not need human-friendly code formatting or names while working; it can operate on a compact representation, while humans still receive readable code.

## Desired Outcome
A practical product/tooling direction that can:
1. Convert human-readable code into a compact AI-facing representation.
2. Use short/meaningless identifiers and minimized layout to reduce token usage.
3. Preserve a deterministic mapping so readable names can be restored when needed.
4. Avoid AI in the conversion/restoration layer.
5. Provide selective context: give the AI function names, mappings, or only relevant code chunks instead of whole files where possible.
6. Work across as many languages as practical.
7. Remain agent-tool agnostic so users can apply it with different AI coding tools.
8. Measure token savings, AI task accuracy, and failure modes after implementation.

## In Scope
- Feasibility evaluation of the original TFY approach.
- Architecture for deterministic transform layers:
  - readable code -> compact representation
  - compact representation -> human-readable output using stored mapping/formatting metadata
- Identifier mapping strategy: full names mapped to compact symbols (e.g., `a`, `b`, `f1`, or numeric IDs).
- Formatting minimization: one-line/no indentation representation where safe.
- Selective context packaging: function/file summaries, symbol maps, demand-driven expansion.
- Multi-language direction, preferably via parser abstractions such as AST/tree-sitter-style language adapters.
- Agent-tool agnostic CLI/library workflow.
- Post-build evaluation of token savings and AI correctness.

## Out of Scope / Non-goals
Partially unresolved. Current user preference rejects narrow MVP and wants broad launch-product scope. Practical implied non-goals for a first planning pass:
- Do not rely on AI to perform code compaction/restoration.
- Do not assume perfect semantic understanding by the transform layer unless language adapters support it.
- Do not treat “minify everything” as sufficient; AI task accuracy must be measured.

## Decision Boundaries
OMX/planning may decide without more confirmation:
- A feasibility-first plan is acceptable.
- Implementation can begin with an exploratory architecture if it preserves the full-product direction.
- Token savings and AI task accuracy should be measured empirically after implementation.

Needs user confirmation if changed:
- Reducing product scope to a classic MVP.
- Limiting launch to only one language permanently.
- Making TFY depend on one AI agent/vendor.
- Using AI inside the transform/restoration layer.

## Constraints
- Multi-language support should be a design goal.
- Agent-tool agnostic usage is required.
- Token reduction and AI work accuracy are both first-class goals.
- Human-facing output must be readable.
- Mapping/restoration must be deterministic, not AI-generated.

## Feasibility Evaluation

### What is realistically possible
The idea is **partly feasible and promising**, but it should be treated as a compiler/indexing/context-protocol problem, not only as code minification.

Feasible pieces:
- **Whitespace/format compression**: straightforward for many languages; token savings are real but limited because modern tokenizers often compress whitespace reasonably and code identifiers dominate many files.
- **Identifier shortening**: feasible with AST/scope-aware parsers. Bigger savings are possible, especially in verbose codebases. Must avoid changing public APIs, reflection-sensitive names, string-based references, macros, generated code, and serialization keys.
- **Name mapping**: feasible via sidecar map files. The AI can receive compact code plus a symbol map on demand.
- **Selective context**: highly feasible and likely the strongest part. Instead of giving full code, provide symbol tables, function signatures, dependency graph, and expand only relevant bodies.
- **Human restoration**: formatting can be restored with existing formatters; names can be restored if transformations are lossless and scope-aware.

### Main risks
- **AI comprehension may degrade** if names become meaningless. Human-readable names carry semantic hints; removing them saves tokens but also removes information the model uses.
- **Net token savings can disappear** if the mapping table must be included every time. TFY should use demand-driven maps, not dump all mappings.
- **Correctness across many languages is hard**. Identifier renaming is not universally safe without language-specific knowledge.
- **One-line code may hurt model performance**. Reduced formatting saves tokens but can harm structural readability for LLMs.
- **Agent-tool agnostic integration** means TFY should likely output prompt/context bundles, patches, and reversible files rather than depend on a specific agent API.

### Stronger product framing
TFY should not be “make code ugly for AI.” A stronger framing is:

> TFY is a reversible code-context compression and symbol-indexing layer for AI coding agents. It trades human-readable surface form for compact, deterministic, selectively expandable context while preserving a path back to readable code.

## Recommended Architecture Direction
1. **Project indexer**
   - Parse files.
   - Build symbol table, scopes, imports, call/reference graph where possible.

2. **Language adapters**
   - Start with a shared adapter interface.
   - Use parser-backed transforms where available.
   - Allow fallback modes: whitespace-only, identifier-only, context-only.

3. **Compact representation generator**
   - Minify formatting.
   - Rename local/private symbols where safe.
   - Preserve or annotate unsafe/public symbols.

4. **Sidecar mapping store**
   - Maps compact IDs ↔ original names.
   - Tracks scope, file, symbol kind, and restoration rules.

5. **Selective context packer**
   - Produces agent-agnostic context bundles.
   - Supports “give only signatures/map first, expand bodies on demand.”

6. **Restoration pipeline**
   - Apply compact edits back to original code.
   - Restore names.
   - Run formatter.
   - Validate diff.

7. **Evaluator**
   - Measure token reduction.
   - Run task-based AI accuracy tests.
   - Track failure categories.

## Testable Acceptance Criteria (initial)
- Token count report exists for original vs TFY compact context.
- TFY can generate a compact bundle and sidecar map without AI.
- TFY can restore human-readable code after a no-op roundtrip.
- TFY supports at least one strong parser-backed language path and a design for adding more.
- TFY can produce agent-agnostic prompt/context files usable outside a specific AI tool.
- Evaluation harness reports savings, restoration failures, and AI task success/failure.

## Pressure-pass Findings
The main pressure point is the conflict between token reduction and AI accuracy. The user chose not to define fixed thresholds now; instead, TFY should build an implementation/evaluator and measure actual savings and accuracy after implementation.

## Residual Risks
- Exact launch scope is broad and unresolved.
- Non-goals are not fully explicit because the user rejected narrowing.
- Success thresholds are empirical rather than predefined.
- Multi-language support may require staged rollout despite the desired launch-product framing.

## Recommended Handoff
Use `$ralplan` next for architecture/test-shape review before implementation, or `$ultragoal` if converting this directly into durable implementation goals.
