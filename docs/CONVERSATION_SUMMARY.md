# TFY Conversation Summary

## 1. Original Idea

The user proposed reducing AI token usage by having AI operate on compact code:

- no indentation
- minimal newlines
- short variable/function names like `f1`, `a`, `b`
- deterministic mapping back to original semantic names
- human-readable output restored after AI work

## 2. Semantic Name Mapping

Concern: short names may reduce AI understanding.

Resolution:

- AI first sees original semantic names.
- Compact symbols have 1:1 maps to original names.
- Maps are provided only when needed and scoped to selected code.

## 3. Names-First Context

Instead of sending all code:

1. show original function/scope names
2. AI requests relevant scopes
3. TFY sends compact bodies for requested scopes
4. if ambiguous, expand related context or full context

## 4. Missing Needed Code Problem

The main risk is hiding code the AI actually needs.

Resolution:

```text
names first -> selected body -> related neighborhood -> full context fallback
```

## 5. Restoration Clarification

Restoration means:

```text
AI compact output -> original names -> readable formatting -> project-ready code
```

This is deterministic and handled by TFY, not guessed by AI.

## 6. Command Output Compression

The user added that command outputs like `git status`, test logs, build logs, and shell outputs also waste tokens.

Resolution:

- compress all command/tool output by risk tier
- preserve errors/important evidence
- compress success/no-action output aggressively
- always keep raw output available by reference

## 7. RTK Relationship

RTK should be referenced heavily for command-output compression, but TFY should stay independent.

RTK reference areas:

- command filters
- raw output recovery
- token tracking
- hooks/proxying

TFY independent identity:

- code context protocol
- compact AI code
- semantic index
- symbol maps
- restoration
- tool feedback compression
- full AI work-context management

## 8. Product Constraints Added Later

The user clarified that TFY should not be treated as a small MVP-only experiment. The first planned product shape should include the full product direction:

- as many programming languages as practical
- compatibility with many AI agent tools
- measurable token savings
- measurable accuracy/correctness impact
- fallback paths when compact context is not enough
- raw/full outputs available after command-output summarization

## 9. Folder/Documentation Organization Request

The user asked to create a new standalone `tfy` folder, separate from the already-cloned RTK reference, move/copy earlier documents into it, and document the conversation so far. The safe interpretation used here is to copy prior OMX artifacts into `tfy/archive/` for traceability while leaving `.omx/tmp/rtk` and previous workflow artifacts intact.
