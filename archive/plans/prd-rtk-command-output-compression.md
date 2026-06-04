# PRD: RTK Risk-Aware Command Output Budgeter

## 1. Product Goal

Add a shared **Risk-Aware Command Output Budgeter** to RTK so all command/tool outputs can be compressed according to signal risk, not only by command-specific filters.

This concretizes the TFY add-on:

```text
error/important output -> preserve evidence, compress only formatting/noise
success/no-action output -> aggressive compression/summarization
all output -> retain raw original for fallback/audit
```

RTK already does command output compression. This plan proposes a common policy layer and APIs so RTK's existing command modules, TOML filters, `summary`, `err`, `test`, hooks, tee, and tracking work together under one risk-aware contract.

## 2. Current RTK Architecture Fit

RTK already has the right foundation:

- `core::runner` centralizes command execution, filtering, tee hints, tracking, and exit-code propagation.
- `core::stream` captures raw stdout/stderr and supports capture/streaming/passthrough modes.
- `core::tee` stores raw output on failures and prints `[full output: path]` hints.
- `core::toml_filter` supports declarative command filters with `match_output.unless`, keep/strip lines, truncation, and stderr filtering.
- `cmds/system/summary` provides a generic heuristic summarizer.
- `cmds/rust/runner` provides generic `err` and `test` wrappers.
- `core::tracking` measures raw vs filtered token savings.

Therefore the feature should be implemented as shared infrastructure layered into the runner/filter pipeline, not as a separate unrelated tool.

## 3. Core Concept

Introduce a normalized **OutputSignal** classification for every command result:

```rust
RiskTier::CriticalEvidence
RiskTier::WarningEvidence
RiskTier::NormalSuccess
RiskTier::NoSignal
RiskTier::Unknown
```

Then apply a tier-specific compression policy.

## 4. Proposed Architecture

### 4.1 New Core Module

Add a new conceptual module:

```text
src/core/output_budget.rs
```

Responsibilities:

1. classify output by risk/signal tier
2. preserve actionable evidence for error/warning output
3. aggressively compress success/no-action/no-signal output
4. produce metadata about omitted content
5. integrate raw-output refs via `core::tee`
6. expose deterministic summaries without relying on AI

### 4.2 Data Model

```rust
pub enum RiskTier {
    CriticalEvidence,
    WarningEvidence,
    NormalSuccess,
    NoSignal,
    Unknown,
}

pub struct OutputBudgetReport {
    pub command_label: String,
    pub exit_code: i32,
    pub risk_tier: RiskTier,
    pub filtered_output: String,
    pub omitted_lines: usize,
    pub omitted_bytes: usize,
    pub preserved_evidence_count: usize,
    pub raw_output_ref: Option<String>,
    pub trigger_reasons: Vec<String>,
}
```

### 4.3 Policy Matrix

| Tier | Typical cases | Compression policy | Raw retention |
|---|---|---|---|
| CriticalEvidence | non-zero exit, compiler errors, failed tests, panics, stack traces, security/destructive output | Preserve original diagnostic snippets, file/line/symbol/exit code; remove only repeated boilerplate/whitespace/progress | Always tee if raw large enough; ideally force tee for critical evidence |
| WarningEvidence | warnings, deprecations, suspicious git state, partial success | Preserve warning lines and nearby context; group repeated warnings | Tee when configured or above threshold |
| NormalSuccess | successful tests/builds, clean git status, successful push/pull, expected install completion | Compact to counts/status summary | Store if `tee.mode=always`, otherwise optional raw ref |
| NoSignal | empty output, repeated progress, already-known success | Emit minimal status | Optional |
| Unknown | unclassified generic shell output | Conservative truncation + raw ref | Prefer tee if output is large |

## 5. Integration Points

### 5.1 `core::runner`

Add optional `RunOptions` policy fields:

```rust
pub output_budget: OutputBudgetPolicy,
pub force_raw_ref_on_success: bool,
```

Possible policies:

```rust
OutputBudgetPolicy::CommandSpecificOnly // current behavior
OutputBudgetPolicy::RiskAwareFallback   // apply when filter output looks unsafe/too verbose
OutputBudgetPolicy::AlwaysRiskAware     // all commands pass through budgeter after filter
```

Default should be conservative: preserve existing command-specific behavior, then apply budgeter only when configured or for generic wrappers until validated.

### 5.2 `core::tee`

Current tee is failure-oriented and path-based. Extend conceptually with:

- `tee_raw_with_reason(raw, command_slug, exit_code, reason)`
- stable raw output ID in addition to path
- optional success tee for large outputs when policy demands auditability

Do not remove current `[full output: path]`; reuse it.

### 5.3 `core::toml_filter`

Add risk-aware fields to TOML filters only if needed after first implementation:

```toml
risk_tier = "normal_success"
preserve_on_exit_nonzero = true
raw_ref = "on_failure"
```

Initial plan can avoid schema churn by applying risk policy after TOML output and using existing `match_output.unless` to prevent swallowing errors.

### 5.4 `cmds/system/summary`

Promote its heuristic type detection into reusable classification helpers where practical:

- TestResults
- BuildOutput
- LogOutput
- ListOutput
- JsonOutput
- Generic

But avoid making `summary` the only implementation; output budgeting must preserve evidence more strictly than freeform summarization.

### 5.5 `cmds/rust/runner` (`rtk err`, `rtk test`)

Use as reference behavior:

- `err`: streaming error/warning pattern preservation
- `test`: failure-only extraction + tee

The new budgeter should generalize these principles across all commands.

## 6. User-Facing Behavior

### Failure Example

Raw:

```text
900 lines of test output
2 failing assertions
many passing tests
```

Filtered:

```text
[FAIL] npm test exit=1
2 failures preserved:
- src/auth.test.ts:42 expected 401, received 200
- src/auth.ts:88 expired token accepted
omitted: 128 passing tests, 340 repeated/progress lines
[full output: ~/.local/share/rtk/tee/...]
```

### Success Example

```text
[ok] npm test: 130 passed
omitted: 900 success/progress lines
raw: cmdout:abc123 (request if needed)
```

### Unknown Generic Output

```text
[summary] command produced 420 lines, 0 errors detected
showing first 20 signal lines; omitted 400
[full output: ...]
```

## 7. RALPLAN-DR Summary

### Principles

1. Preserve actionable evidence before saving tokens.
2. Compress success/no-action output harder than failure output.
3. Always keep a raw-output recovery path for high-risk or large outputs.
4. Reuse RTK runner/tee/tracking infrastructure.
5. Measure missed-signal risk, not only token savings.

### Decision Drivers

1. Avoid hiding important errors from AI agents.
2. Reduce token waste from routine successful command output.
3. Fit RTK's existing command proxy architecture with minimal disruption.

### Viable Options

#### Option A: Add more command-specific filters only

Pros:
- matches current RTK model
- precise per command
- low shared abstraction risk

Cons:
- does not solve generic/all-command-output policy
- duplicates safety logic
- harder to ensure raw retention/fallback consistency

#### Option B: Add shared risk-aware post-filter budgeter

Pros:
- centralizes safety policy
- works with existing Rust and TOML filters
- can cover generic shell output
- preserves command-specific filters as first pass

Cons:
- classification false negatives/positives can hide or over-preserve output
- needs careful default rollout

Recommended.

#### Option C: Replace existing filters with a universal summarizer

Pros:
- simpler conceptual model
- covers all commands uniformly

Cons:
- loses command-specific precision
- high risk of hiding diagnostics
- conflicts with RTK's proven module/TOML architecture

Rejected.

## 8. Acceptance Criteria

- Existing command-specific filters continue to work and preserve exit codes.
- Non-zero/error outputs preserve actionable raw diagnostic evidence.
- Success/no-action outputs can be compressed more aggressively than failures.
- Large/high-risk outputs include a raw-output recovery reference.
- Budget reports include omitted line/byte counts and trigger reasons.
- Tracking records raw vs filtered token savings as before.
- Tests prove failures are not hidden by success/noise compression.
- Generic shell output has conservative fallback behavior.

## 9. ADR

### Decision

Implement command-output compression as a shared, risk-aware post-filter budget layer integrated with RTK runner/tee/tracking, not as a replacement for command-specific filters.

### Why

RTK already has mature command-specific filters and recovery infrastructure. The missing piece is a common policy for all command outputs: failure evidence must be preserved; success noise can be aggressively compressed; raw output should remain recoverable.

### Consequences

- New core classification/compression module is needed.
- Runner options may need extension.
- Tee may need richer raw refs/reasons.
- Evaluation must measure missed-signal risk.

### Follow-ups

- Define exact classifier heuristics.
- Decide default rollout mode: opt-in, generic-only, or all commands.
- Add regression fixtures for failure preservation and success compression.

---

# Revision 1: RTK Architecture Fit Clarifications

## 10. Runner-Mode Insertion Points

RTK has distinct execution paths. The output budgeter must not be treated as a universal post-hoc layer for all paths.

### 10.1 `run_filtered`

Path: capture raw output -> command-specific filter -> print.

V1 behavior:

1. classify using raw stdout/stderr + exit code before final print
2. let command-specific filter remain primary evidence extractor
3. apply budget policy to the filtered string only if it is still verbose or mismatched with risk tier
4. tee raw output according to bounded raw-retention policy
5. track raw vs final output

### 10.2 `run_filtered_with_exit`

Same as `run_filtered`, but classifier and filter both receive exit code. This is the preferred capture path for risk-aware filters because exit code is first-class.

### 10.3 TOML fallback in `main.rs`

V1 behavior:

1. execute and capture stdout/stderr according to TOML `filter_stderr`
2. classify raw captured output + exit code before accepting any short-circuit success message
3. preserve existing `match_output.unless` as first-line safety
4. if raw classification is CriticalEvidence/WarningEvidence, prevent a success-like `match_output` result from hiding evidence
5. append raw hint when high-risk/large output is retained

### 10.4 `run_streamed`

Streaming emits lines live, so a post-filter budgeter cannot retroactively compress emitted output.

V1 decision:

- **Do not apply post-hoc budget compression to already-emitted streaming output.**
- Streaming filters remain command-specific and evidence-preserving.
- The budgeter may only participate through a streaming-aware adapter that gates lines before emission.
- If no streaming-aware adapter exists, `run_streamed` is considered out of scope for V1 budget rewriting, except for final metadata/tee/tracking.

Future streaming-aware adapter concept:

```rust
struct BudgetedStreamFilter<F> {
    inner: F,
    classifier_state: StreamingRiskState,
    emitted_counts: OmissionCounts,
}
```

This adapter would classify lines before emission and may suppress success/progress noise, but must emit critical evidence immediately.

### 10.5 `run_passthrough`

Passthrough inherits TTY directly and does not capture raw output. V1 budgeter does not apply. Tracking remains passthrough-only.

### 10.6 Explicit Scope of V1

V1 applies to:

- capture-based `run_filtered`
- `run_filtered_with_exit`
- TOML fallback/capture filters
- generic `summary`-style capture wrappers where appropriate

V1 does not rewrite:

- already-emitted streaming output
- TTY passthrough commands

## 11. Classifier Input Precedence

The budget classifier must inspect data in this order:

1. **exit code**
2. **raw stderr**
3. **raw stdout**
4. command label and known command family
5. command-specific filter metadata if available
6. final filtered output only as a sanity check / display input

Reason: filtered output can already have dropped evidence. Therefore raw output is authoritative for risk classification.

Rule:

> A filtered success-looking message must not downgrade a raw output classified as CriticalEvidence or WarningEvidence.

## 12. OutputBudgetReport Data Flow

`OutputBudgetReport` is not printed as JSON by default. It is an internal struct used to:

- choose final display text
- decide whether to tee raw output
- record omitted counts and risk tier for tests/evaluation
- optionally feed tracking/telemetry in aggregate-safe form

V1 user-facing output remains RTK-style compact text, with optional raw hint.

Example display:

```text
[FAIL] cargo test: 2 failures preserved, 128 passes omitted
[full output: ~/.local/share/rtk/tee/...]
```

## 13. Bounded Raw Retention Policy

“Always retain raw” becomes a bounded policy compatible with RTK tee:

| Case | Retention |
|---|---|
| CriticalEvidence + raw >= tee min size | force tee regardless of default failure/success mode |
| CriticalEvidence + raw small | no tee required if all evidence displayed |
| WarningEvidence + large raw | tee when configured or policy requests audit |
| NormalSuccess | tee only when `tee.mode = always` or explicit success-retention policy is enabled |
| NoSignal | no tee unless `tee.mode = always` |
| Unknown + large raw | prefer tee/hint because classifier is uncertain |

New config concept:

```toml
[output_budget]
enabled = false              # start opt-in
mode = "risk-aware-fallback" # command-specific-only | risk-aware-fallback | always-risk-aware
success_raw = "never"        # never | large | always
unknown_raw = "large"        # never | large | always
```

This avoids unbounded storage/IO growth while preserving recovery for high-risk cases.

## 14. Command Module Ownership

Command modules remain authoritative for command-specific parsing and evidence extraction.

The budgeter may:

- classify risk from raw output
- enforce that high-risk raw output is not hidden
- compress generic success/noise
- attach raw hints
- collect omitted counts

The budgeter must not:

- replace cargo/pytest/git/tsc-specific parsers
- reinterpret structured diagnostics worse than a command module
- force one generic summary over a high-quality command-specific filter

If a command module can emit metadata in the future, the budgeter can consume it, but V1 does not require changing every module.

## 15. Revised Rollout Recommendation

1. **Opt-in experimental config** for capture-based paths only.
2. Apply to generic wrappers and TOML fallback first.
3. Add per-command module adoption only after tests prove no evidence loss.
4. Add streaming-aware adapter later as a separate feature.
5. Never default `AlwaysRiskAware` until benchmarks show no planted-error loss.
