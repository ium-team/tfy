# Test Spec: TFY Standalone Concept

## Code Compression Tests

- Build semantic index from source functions/scopes.
- Generate compact code with short symbols.
- Emit deterministic local symbol map.
- Restore no-op compact roundtrip to readable source.
- Reject ambiguous compact patches.
- Expand selected function body on request.
- Fall back to full context when ambiguity is high.

## Symbol Map Tests

- Scope-aware maps avoid collisions.
- Shadowed variables restore correctly.
- Public/unsafe names are preserved or marked non-renamable.
- Map output is deterministic across unchanged code.

## Tool Feedback Tests

- Failed command preserves error evidence.
- Successful command compresses aggressively.
- Raw output is stored and requestable.
- Raw range expansion works around a file/line/error.
- Unknown output uses conservative compression.

## Evaluation Tests

Measure:

- raw token count
- compact token count
- net savings after expansions
- AI task success/failure
- missed-needed-code failures
- missed-command-error failures
- fallback frequency
- restoration success/failure

## Compatibility Tests

- Run the same compact-context scenario through at least two agent/tool integration shapes, such as direct CLI protocol and editor/agent prompt wrapper.
- Verify command-output raw refs can be expanded regardless of agent/tool frontend.
- Verify compact patch restoration remains deterministic when the agent emits only compact symbols.
- Verify language adapters expose a common scope index contract across different languages.

## Release-Grade Gates

- Token savings are reported as net savings after maps, expansions, and raw requests.
- Correctness degradation is measured against a baseline full-context workflow.
- Any missed-needed-code or missed-command-error failure is classified and used to tune fallback policy.
- Full raw command output and full code context remain recoverable for audit/debug.
- Documentation clearly distinguishes TFY from RTK while preserving RTK-derived lessons for tool feedback.

## Pass/Fail Criteria

These thresholds are initial release gates and can be tightened after benchmark data exists.

### Token Savings

- **Pass:** median net token savings is positive after maps, expansions, and raw requests.
- **Pass:** at least one evaluated workflow class reaches meaningful savings without correctness regression.
- **Fail:** apparent savings disappear after required expansions/raw requests.

### Correctness Degradation

- **Pass:** compact workflows complete the same benchmark tasks as the full-context baseline within the configured tolerance.
- **Fail:** compact mode causes repeated task failures that full-context baseline does not show.
- **Fail:** any correctness regression is left unclassified.

### Restoration

- **Pass:** no-op compact roundtrip restores byte-equivalent or formatter-equivalent readable code.
- **Pass:** restored patches parse successfully for languages with parser support.
- **Fail:** any unmapped, collision-prone, or ambiguous symbol is accepted instead of rejected.
- **Fail:** restored output changes public/unsafe/non-renamable identifiers without explicit policy.

### Missing Needed Code

- **Pass:** every missed-needed-code case is classified with a fallback-policy update or explicit known limitation.
- **Fail:** any unclassified missed-needed-code failure remains before release.

### Missed Command Error

- **Pass:** non-zero exits, stderr, stack traces, compiler diagnostics, and test failures preserve actionable evidence in compact output.
- **Pass:** raw output is available by `raw_ref` for every summarized command/tool result.
- **Fail:** any unclassified missed-command-error failure remains before release.

## RTK Reference Tests

When comparing with RTK-like behavior:

- verify TFY can match or learn from command-output compression patterns
- verify TFY remains broader than command-output filtering
- compare raw-output recovery policies
