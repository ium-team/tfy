# TFY Tool Feedback Method Family

This document specializes the final architecture for command, shell, test, CI, Git, GitHub, and generic tool outputs.

## Registry entries

### Risk-aware tool feedback compression

- **Target:** command/tool output.
- **Mechanism:** classify risk and emit compact summaries with raw refs.
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

## Core policy

```text
critical/error output -> preserve evidence + raw ref
warning/unknown output -> conservative summary + raw ref
success/no-action output -> aggressive summary or fingerprint + raw ref
repeated unchanged output -> status/hash/ref only
```

## Summary shape

```json
{
  "command": "npm test",
  "exit_code": 1,
  "risk": "critical",
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

Raw fallback is mandatory for every summarized command/tool result.

## Relationship to Git/GitHub

Git and GitHub are specialized high-frequency tool-feedback domains. `GIT_GITHUB_HARNESS.md` defines their evidence contract. That harness is a method-family specialization, not a separate product identity.
