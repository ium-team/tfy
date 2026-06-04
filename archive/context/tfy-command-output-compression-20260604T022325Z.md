# Context Snapshot: TFY Command Output Compression

- Task statement: Explore whether TFY can also reduce token waste from AI-executed command outputs such as git status, test logs, build output, and shell command results.
- Desired outcome: Clarify if command-output compression should be included in TFY and what problem it should solve.
- Stated solution: Summarize/filter/minify command output before it is sent back into AI context, similar to TFY's code-context compression.
- Probable intent hypothesis: Reduce unnecessary tokens from repetitive or verbose command outputs while preserving the actionable signal an AI agent needs.
- Known facts/evidence: Existing TFY docs define names-first code context compression, compact code, maps, on-demand expansion, and fallback. Command output compression is not yet explicitly defined in the prior PRD/test-spec.
- Constraints: Must not hide important errors; must preserve enough evidence for AI decisions; should be deterministic/tooling-driven rather than AI-generated where possible.
- Unknowns/open questions: Which command categories matter first, how aggressive filtering can be, how to preserve raw output for fallback/audit, and whether this is a TFY core feature or adjacent module.
- Decision-boundary unknowns: Whether command output compression should be part of TFY or a separate companion layer.
- Relevant repo docs/rules/context inspected: Prior TFY planning artifacts in thread; current task is greenfield/spec-only.
- Prompt-safe initial-context summary status: not_needed.
