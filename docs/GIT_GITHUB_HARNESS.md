# TFY Git/GitHub Harness

## Status

This is a specialized method-family document under the final TFY architecture. It does not make TFY a Git-only or GitHub-only product. See `TOKEN_SAVING_ARCHITECTURE.md` for the canonical architecture.

## Purpose

Git and GitHub workflows are high-frequency AI coding tasks with high evidence risk. TFY must reduce their token cost without hiding dirty state, diffs, conflicts, rejected pushes, failed checks, auth/rate-limit/API failures, requested changes, or unresolved review comments.

Baseline interface:

```sh
tfy run -- git status --short
tfy run -- git diff --stat
tfy run -- gh pr checks 123
tfy raw <raw_ref>
tfy raw <raw_ref> --around path/to/file.py:42
```

## Registry entry

```yaml
name: git-github-harness
targetArtifact: command-output
coreStatus: specialized-core
mechanism: risk-aware summaries, evidence fields, fingerprints, raw refs, fixture replay
savingsMetric: net tokens after raw/around expansions
performanceCost: command capture, regex/classification, raw storage
correctnessRisk: hidden dirty state, missed failed check, lost review blocker
fallbackTrigger: dirty state, conflict, failed check, requested changes, auth/rate-limit/API error, low confidence
validationGate: fixture/replay evidence plus raw recovery; live credentials are a separate integration decision
```

## Evidence contract

Every Git/GitHub summary must preserve, when present:

| Evidence | Required behavior |
|---|---|
| command | identify command family and intent |
| exit code | preserve success/failure |
| risk tier | success, conservative, critical |
| raw ref | always include `raw_ref` |
| branch/upstream | preserve branch/ahead/behind info |
| dirty state | preserve staged/unstaged/untracked/conflict codes |
| changed paths | preserve paths and status codes |
| diffs | preserve stat/name-status/hunk/risky markers |
| push/pull errors | preserve rejected/auth/conflict/ref reason |
| checks | preserve check/job names, conclusion, failing excerpts |
| review feedback | preserve requested-changes/actionable/unresolved refs |
| API failures | preserve rate-limit/403/404/auth/error text |

## Risk tiers

| Output | Tier | Policy |
|---|---|---|
| clean status / all checks passed | success | compact status/fingerprint + raw ref |
| dirty status / normal diff / comments | conservative | preserve paths/status/refs + raw ref |
| conflict / rejected push / failed check / auth/API error | critical | preserve actionable evidence + raw ref |
| unknown output | conservative | cap noise, preserve head/tail/evidence + raw ref |

## Task families

### Local Git state

Preserve branch, clean/dirty state, staged/unstaged/untracked/conflict markers, changed paths, and raw ref.

### Change review

Preserve path lists, status codes, diff stats, hunk headers, generated/lockfile/large-diff markers, and raw ref.

### Commit preparation

TFY may summarize status, diff, verification, and evidence, but it does not imply commits are automatic. Destructive/history-changing actions remain explicit user/agent decisions.

### Push/pull/merge/rebase recovery

Classify conservatively or critically when output contains conflicts, rejected pushes, auth/permission failures, ref ambiguity, or rebase stop instructions.

### GitHub PR/issue/CI

Preserve PR/issue/check/run IDs, URLs, review state, failed job names, file/line excerpts, and API/auth/rate-limit errors.

## Fixture/replay verification

GitHub/PR/CI behavior must be tested with local raw-output fixtures or synthetic replay strings unless live integration is explicitly in scope. Fixture replay proves local evidence preservation and raw fallback; it does not prove complete live GitHub/API fidelity.

Required fixture classes:

- clean Git status
- dirty Git status
- diff/name-status output
- merge/rebase conflict
- push rejection/auth failure
- CI/check failure
- PR requested changes/review comments
- GitHub auth/rate-limit/API error

## Relationship to final TFY architecture

This harness uses these final TFY concepts:

- method registry
- risk-aware tool feedback
- raw refs and around expansion
- output fingerprinting
- error/evidence clustering
- Test/CI selective evidence
- evaluation gates for missed evidence and net savings
