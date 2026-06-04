# Context Snapshot: tfy-token-compression

- Task statement: Evaluate and concretize a proposed companion/extension to `rtk` named `tfy` / "token fuck you" for token-reducing code representation.
- Desired outcome: Clarify whether the idea is useful, what problem it solves, what the first version should do, and what should stay out of scope before planning or implementation.
- Stated solution: Let AI operate on a compact code form: one-line/minified layout, short meaningless identifiers, and selective context disclosure; restore human-friendly names/formatting when output reaches humans without using AI in the transform layer.
- Probable intent hypothesis: Reduce LLM token cost/context usage while preserving enough semantic mapping to develop/read code effectively, possibly by building reversible compiler-like transforms and context slicing.
- Known facts/evidence: Current workspace has no source files, README, docs, or existing code artifacts beyond `.omx/` state/log files. This is currently greenfield in this directory.
- Constraints stated by user: Human-facing output should be restored to readable variable/function names and formatting; transform between compact and readable forms should not depend on AI.
- Unknowns/open questions: Primary user, target language(s), whether this is a CLI/library/protocol, MVP scope, lossless vs lossy transforms, mapping persistence, integration with editors/LLMs, evaluation metrics, privacy/security boundaries, and relationship to `rtk`.
- Decision-boundary unknowns: What OMX may decide automatically vs what requires user approval; acceptable tradeoffs between token savings, debuggability, determinism, and correctness.
- Likely codebase touchpoints: None discovered yet; greenfield repo.
- Relevant repo docs/rules/context inspected: Current directory listing; no README/docs/CONTEXT found. User-provided AGENTS.md instructions govern workflow.
- Terminology/doc conflicts found: `tfy` name is intentionally provocative; no repo terminology exists yet. Need decide public/product naming vs internal codename.
- Prompt-safe initial-context summary status: not_needed.
