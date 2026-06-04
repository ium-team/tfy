# Context Snapshot: TFY Definition

- Task statement: Define the TFY concept from all discussion so far as planning documentation.
- Desired outcome: Durable PRD/test-spec/consensus planning artifacts for a future implementation handoff.
- Known facts/evidence: Workspace is greenfield; prior deep-interview artifacts exist under `.omx/specs/` and `.omx/interviews/`.
- Core clarified concept: TFY is an AI-facing compact code interface where AI receives and emits compact code without indentation/newlines and with short symbols (`f1`, `a`, `b`), while deterministic 1:1 maps connect compact symbols to original semantic names. AI first sees a semantic index of original long names for functions/scopes, requests needed scopes, receives only those compact bodies plus the relevant mapping once, and can fall back to broader/full context when ambiguous.
- Constraints: no AI in transform/restoration layer; agent-tool agnostic; multi-language direction; token savings must be measured against AI performance degradation.
- Unknowns/open questions: exact first language/runtime implementation, tokenizer benchmark targets, UI/CLI protocol details, exact expansion heuristics.
- Likely touchpoints: future source code only; current task writes planning docs only.
