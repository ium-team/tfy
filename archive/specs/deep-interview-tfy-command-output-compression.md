# Spec Addendum: TFY Command Output Compression

## Metadata
- Source: `$deep-interview`
- Profile: quick
- Context type: greenfield/spec-only
- Final ambiguity: ~15.8%
- Related plan: `.omx/plans/prd-tfy-definition.md`

## Core Addition
TFY should also reduce token waste from **AI command/tool output**, not only source-code context.

Examples:
- `git status`, `git diff`, `git log`
- test output
- build/typecheck output
- `grep`, `find`, `ls`, `cat`, and other shell outputs
- generic agent tool results

This extends TFY into a broader **token budget manager for code context + tool feedback**.

## User-Preferred Policy
Use **risk-aware output compression**:

```text
important/error output -> preserve evidence, compress only formatting/whitespace
success/no-action output -> aggressive compression/summarization
all output -> raw original retained for fallback/audit
```

## Compression Tiers

### Tier 0: Raw Retention
TFY always stores the full raw command output internally with metadata:

- command
- exit code
- timestamp
- working directory
- stdout/stderr split if available
- raw output hash/id

The AI may receive a compressed view, but raw output remains requestable.

### Tier 1: Important/Error-Preserving Compression
Used for:

- non-zero exit codes
- errors
- warnings
- stack traces
- compiler/typecheck diagnostics
- failing tests
- security/destructive command output
- anything marked important by detector or user/agent

Policy:
- preserve original error lines/snippets
- preserve file paths, line numbers, symbols, exit codes
- preserve enough surrounding context to act
- only remove redundant whitespace, repeated boilerplate, or duplicated blocks
- never paraphrase away the actionable evidence

### Tier 2: Normal Success Compression
Used for:

- successful commands
- expected/no-action output
- repeated progress logs
- passing test summaries
- normal git cleanliness/status

Policy:
- summarize aggressively
- collapse repeated lines
- report counts/status instead of full text
- keep raw output ID for fallback

### Tier 3: No-Signal Elision
Used when output adds no new decision value.

Examples:
- “all tests passed” repeated after known pass
- empty output from expected command
- long successful install/build progress with no warnings

Policy:
- emit minimal status only
- include raw output reference

## Example Output Shape

```json
{
  "command": "npm test",
  "exit_code": 1,
  "compression_tier": "important-error-preserving",
  "summary": "2 tests failed in auth validation",
  "preserved_evidence": [
    "src/auth.test.ts:42 expected 401, received 200",
    "src/auth.ts:88 validateToken returned true for expired token"
  ],
  "omitted": {
    "passing_tests": 128,
    "repeated_lines": 340
  },
  "raw_output_ref": "cmdout:20260604:abc123"
}
```

## Relationship to Existing TFY Code Compression
This is conceptually parallel to names-first code context:

- code context: show names/structure first, expand bodies on demand
- command output: show compressed signal first, expand raw output on demand

Both follow the same principle:

> Give the AI the smallest representation that preserves decision quality, and keep a safe path to expand when uncertainty or risk is high.

## Non-goals
- Do not hide error evidence just to save tokens.
- Do not rely on AI to invent summaries when deterministic extraction can preserve evidence.
- Do not discard raw outputs.
- Do not treat success and failure logs with the same compression policy.

## Acceptance Criteria
- Raw command output is retained and requestable.
- Non-zero/error outputs preserve actionable evidence.
- Successful/no-action outputs can be aggressively compressed.
- Compression report includes what was omitted and why.
- AI/tool can request raw output by reference.
- Evaluation measures token savings and whether compression caused missed errors.

## Recommended PRD Update
Add a TFY subsystem:

**Command Output Compressor / Tool Feedback Budgeter**

Responsibilities:
1. classify command output by risk/signal tier
2. compress output according to tier
3. preserve raw output for fallback
4. expose raw refs and expansion requests
5. measure token savings and missed-signal risk
