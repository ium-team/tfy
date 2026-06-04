# Test Spec: RTK Risk-Aware Command Output Budgeter

## Goal

Verify an RTK-compatible all-command-output budgeter that compresses success/no-action output aggressively while preserving actionable failure evidence and raw-output fallback.

## 1. Unit Tests: Risk Classification

Fixtures:

- non-zero exit with `error:` line -> `CriticalEvidence`
- non-zero exit with no obvious error line -> `CriticalEvidence` or `Unknown` with raw ref
- zero exit with `warning:` lines -> `WarningEvidence`
- zero exit with all tests passed -> `NormalSuccess`
- empty output with zero exit -> `NoSignal`
- large unclassified output -> `Unknown`

Assertions:

- exit code influences tier
- error/warning/panic/fail patterns trigger evidence tiers
- success/no-signal output never classified as critical without evidence

## 2. Unit Tests: Evidence Preservation

For failure outputs, filtered output must retain:

- exit code
- error/warning line
- file path
- line/column numbers
- failing assertion text
- stack trace head or relevant frame

It may remove:

- repeated progress lines
- passing tests
- duplicated log blocks
- excess whitespace

## 3. Unit Tests: Success Compression

For successful outputs:

- passing tests collapse to summary
- successful build progress collapses to summary
- clean `git status` collapses to minimal status
- repeated progress lines are omitted with counts

## 4. Integration Tests: Runner Behavior

Using a fake command or shell fixture:

- `run_filtered` still tracks raw vs filtered savings.
- exit code is preserved.
- tee hint appears for failure/high-risk large output.
- stdout-only behavior remains compatible.
- `skip_filter_on_failure` still bypasses budgeter when explicitly requested.

## 5. Integration Tests: TOML Filter Compatibility

- Existing TOML filter output remains unchanged under `CommandSpecificOnly` policy.
- Under risk-aware policy, non-zero output with errors is not swallowed by `match_output` success messages.
- `match_output.unless` remains a safety mechanism.
- `filter_stderr` output can be classified and preserved.

## 6. Regression Tests: Do Not Hide Errors

Cases:

- success-looking output followed by one error
- long install log with warning near the middle
- compiler output with many warnings and one error
- test output with mostly passing tests and one failure
- command returns non-zero but stderr is short

Expected:

- actionable evidence preserved
- omitted counts reported
- raw ref present for large/high-risk output

## 7. Evaluation Metrics

Every benchmark reports:

- raw bytes/tokens
- filtered bytes/tokens
- savings percentage
- risk tier
- omitted lines/bytes
- raw ref present/absent
- whether a planted error was preserved
- false positive/negative classification count

## 8. Rollout Tests

Policy modes:

1. `CommandSpecificOnly` — baseline/current behavior.
2. `RiskAwareFallback` — budgeter applies only to generic or unsafe/verbose outputs.
3. `AlwaysRiskAware` — budgeter applies after every command-specific filter.

Acceptance gate:

- `CommandSpecificOnly` baseline must not regress.
- `RiskAwareFallback` must preserve all planted errors.
- `AlwaysRiskAware` must be measured before defaulting on.

---

# Revision 1: Runner-Mode and Streaming Tests

## 9. Runner Mode Matrix Tests

### `run_filtered`

- Raw output contains one error and many success lines.
- Command-specific filter returns a compact string.
- Budget classifier still sees raw error and preserves/fails safe.

### `run_filtered_with_exit`

- Non-zero exit code with short stderr is classified as CriticalEvidence.
- Exit-aware filter cannot downgrade critical tier to normal success.

### TOML fallback

- `match_output` success rule with `unless` does not fire when raw contains error.
- If a project/user TOML filter accidentally returns success-like output for raw error output, risk-aware policy prevents evidence hiding.

### `run_streamed`

V1 test:

- Documented as not post-processed by budgeter.
- Existing streaming filter behavior remains unchanged.
- Raw tee/tracking still works as current architecture permits.

Future adapter test:

- A `BudgetedStreamFilter` fixture suppresses progress lines before emission.
- It emits error lines immediately.
- It reports omitted counts at flush/on_exit.

### `run_passthrough`

- Budgeter does not apply.
- Tracking remains passthrough.

## 10. Classifier Precedence Tests

- Raw stderr with `error:` + filtered `[ok]` still classifies as CriticalEvidence.
- Exit code 1 + empty output classifies as CriticalEvidence or Unknown-safe, not NormalSuccess.
- Raw warning + success summary classifies as WarningEvidence.
- Filtered output is never allowed to downgrade raw evidence tier.

## 11. Raw Retention Policy Tests

- Critical large output forces tee/raw hint even if success retention is disabled.
- Critical small output may avoid tee when all evidence is displayed.
- Normal success does not tee by default.
- Unknown large output gets raw hint under `unknown_raw = large`.
- Tee rotation/size limits still apply.

## 12. Command Module Ownership Tests

- Existing cargo/git/pytest-style command filter golden outputs do not change under `CommandSpecificOnly`.
- Under `RiskAwareFallback`, high-quality command-specific failure output is not replaced by generic summary.
- Budgeter only adds raw hint/omission metadata when needed.
