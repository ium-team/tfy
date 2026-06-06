# TFY Tool Feedback Method Family

This document specializes the final architecture for command, shell, test, CI, Git, GitHub, and generic tool outputs.

## Registry entries

### Risk-aware tool feedback compression

- **Target:** command/tool output.
- **Mechanism:** store raw locally, classify risk, build redacted raw and summary candidates, then emit the smaller safe model-visible text.
- **Savings:** success/no-action noise is reduced.
- **Risk:** missed errors.
- **Fallback:** raw/full or around-based expansion.

### Tool-output fingerprinting

- **Target:** repeated successful outputs.
- **Mechanism:** send status, hash, duration, and raw ref when output is unchanged.
- **Savings:** repeated test/build/status output becomes near-zero.
- **Risk:** false unchanged classification.
- **Fallback:** raw ref and changed-hash expansion.

### Error clustering

- **Target:** repeated diagnostics.
- **Mechanism:** group by file, symbol, assertion, error code, stack frame, or root-cause signature.
- **Savings:** avoids repeating duplicate failures.
- **Risk:** grouping distinct failures together.
- **Fallback:** representative evidence plus raw refs for every cluster.

### Test/CI selective evidence

- **Target:** test and CI logs.
- **Mechanism:** compress passing noise; preserve failing test names, assertion deltas, file/line refs, job/check names, and raw refs.
- **Savings:** large green logs shrink aggressively.
- **Risk:** hiding flaky/slow signals.
- **Fallback:** raw/ranged expansion.

## Implemented P0 command-family filters

The Tool Gateway now assigns a stable `command_family` in the shared command-output path and uses fixture-driven summaries for the RTK-overlapping P0 families:

- `git_status`, `git_diff`, `git_log`
- `gh_pr_checks`
- `cargo_test`, `cargo_clippy`, `cargo_build`, `cargo_check`
- `pytest`, `npm_test`, `pnpm_test`, `yarn_test`, `go_test`

Every family summary still passes through the no-negative-savings selector: TFY emits the family summary only when it is smaller than redacted public raw output, or emits a recoverable suppression notice for unsafe/binary-ish output. Raw bytes are stored first in all cases. Unsupported or low-confidence commands remain on the generic path and may pass through redacted raw output.

## Core policy

```text
all output -> store exact raw bytes locally first
public candidate -> redact secrets/credential URLs before model visibility
critical/error output -> preserve evidence; summarize only if shorter than public raw
warning/unknown output -> conservative summary only if shorter than public raw
success/no-action output -> pass through tiny raw or summarize/fingerprint noisy output
repeated unchanged output -> status/hash/ref only
binary/unsafe output -> short suppressed placeholder + raw ref
```

The default model-visible path is plain text, not JSON. JSON-like shapes below are design/debug examples for internal adapters and documentation only.

## Debug/internal summary shape

```json
{
  "command": "npm test",
  "exit_code": 1,
  "risk": "critical",
  "model_text": "2 tests failed, 128 passed\nraw_ref=cmdout_001",
  "rendering_kind": "summary",
  "summary": "2 tests failed, 128 passed",
  "evidence": [
    "auth.test.ts:42 expected 401, got 200",
    "token.test.ts:19 expired token accepted"
  ],
  "omitted": {
    "passing_tests": 128,
    "duplicate_lines": 340
  },
  "raw_ref": "cmdout_001"
}
```

## Raw fallback

```sh
tfy raw cmdout_001
tfy raw cmdout_001 --around auth.test.ts:42
tfy raw cmdout_001 --around "failed check"
```

Raw fallback is mandatory for every summarized, truncated, or suppressed command/tool result. Tiny pass-through output may omit `raw_ref` from model-visible text to avoid negative savings, while the raw store and structured debug/adapter metadata still preserve byte-exact recovery.

## Relationship to Git/GitHub

Git and GitHub are specialized high-frequency tool-feedback domains. `GIT_GITHUB_HARNESS.md` defines their evidence contract. That harness is a method-family specialization, not a separate product identity.


## Adapter session reporting

`tfy adapter run` records internal Tool Gateway events with raw/model-visible byte sizes, rendering kind, `command_family`, savings percentage, and negative-savings avoidance markers. `tfy adapter report --session <id>` aggregates those events so a developer can see whether command-boundary interception actually reduced model-visible tokens for the session.

Adapter reports use `raw_bytes` and `model_bytes` as the public size contract. They include deterministic `family_counts` and `families_by_saved_tokens` so savings can be audited by command family. Legacy runtime ledger fields such as `raw_chars` / `model_chars` are compatibility-only and are not emitted by `tfy adapter report`; legacy events without `command_family` are classified through the shared lightweight classifier when possible.
