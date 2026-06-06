# TFY Concept

## Definition

TFY is a **whole-workflow token-saving service for AI coding**. It controls what an AI agent sees, how compactly it sees it, how it asks for more, how compact output is restored, and how raw/full evidence remains recoverable.

TFY's final product identity is broader than any single method:

```text
semantic code + project docs + commands + CI + Git/GitHub + task state + patches + provider context
-> representation ladder
-> method registry
-> agent-neutral compact protocol
-> deterministic restoration / raw fallback / evaluation gates
```

## What TFY is not

- not only a code minifier
- not only command-output compression
- not an RTK fork
- not a provider-specific prompt-cache wrapper
- not a lossy summarizer that hides evidence
- not a phased MVP narrative

## Final product principles

1. **All token surfaces are in scope.** Code, docs, logs, tool calls, patches, task state, and repeated provider prompts can all be optimized.
2. **Meaning is recoverable.** Compact symbols, refs, redactions, and summaries must have deterministic recovery or raw fallback where correctness/audit matters.
3. **Safety beats compression.** TFY expands when uncertainty, risk, or diagnostics rise.
4. **Agent-neutral core.** Any AI tool can use the core protocol; provider-specific features are optional adapters.
5. **Future methods are expected.** New techniques enter through the method registry and evaluation gates.

## Main flow

```text
1. Index project artifacts and task state.
2. Represent each artifact through the cheapest safe ladder level.
3. Give the agent semantic orientation before selected compact detail.
4. Preserve refs to raw/full source and command evidence.
5. Let the agent emit compact patches or edit scripts.
6. Restore, validate, and format human/project-ready output.
7. Evaluate net savings, correctness, fallback, and overhead.
```

## Canonical architecture spine

See `TOKEN_SAVING_ARCHITECTURE.md` for:

- representation ladder
- method registry schema
- core vs optional adapter boundary
- method families
- safety and performance model

Focused docs should specialize this spine, not redefine TFY.
