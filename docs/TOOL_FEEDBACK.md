# TFY Tool Feedback Layer

## Purpose

TFY should also reduce token waste from commands and tool outputs used by AI agents.

Examples:

- `git status`, `git diff`, `git log`
- test logs
- build/typecheck logs
- shell outputs like `find`, `grep`, `ls`, `cat`
- generic tool calls

## Core Policy

```text
error/important output -> preserve evidence, compress only whitespace/repetition/noise
success/no-action output -> compress aggressively
all output -> keep raw original available by reference
```

## Output Shape

Example failure summary:

```json
{
  "command": "npm test",
  "status": "failed",
  "summary": "2 tests failed, 128 passed",
  "important": [
    "auth.test.ts:42 expected 401, got 200",
    "token.test.ts:19 expired token accepted"
  ],
  "omitted": {
    "passing_tests": 128,
    "progress_lines": 340
  },
  "raw_ref": "cmdout_001"
}
```

Example success summary:

```json
{
  "command": "npm test",
  "status": "ok",
  "summary": "All 130 tests passed",
  "omitted": {
    "full_log_lines": 900
  },
  "raw_ref": "cmdout_002"
}
```

## Raw Output Expansion

AI or user can request:

```text
show raw cmdout_001
show raw cmdout_001 around auth.test.ts:42
```

TFY then provides the full output or selected raw ranges.

## Risk Tiers

| Tier | Policy |
|---|---|
| Critical/error | preserve raw evidence and line/file/error details |
| Warning/important | preserve warning lines and nearby context |
| Normal success | summarize strongly |
| No signal | emit minimal status only |
| Unknown | conservative summary + raw ref |

## Relationship to Code Compression

Code and command output use the same philosophy:

```text
Code:
  names first -> selected body -> full context

Command output:
  summary first -> selected raw range -> full raw output
```
