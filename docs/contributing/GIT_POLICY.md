# TFY Git Policy

TFY uses **Git Flow** with Lore commit messages. This policy is mandatory for human and AI-agent changes.

## Permanent branches

| Branch | Role | Merge source | Merge target |
|---|---|---|---|
| `main` | Production/release history only | `release/*`, `hotfix/*` | none |
| `develop` | Integration branch for next release | `feature/*`, `bugfix/*`, `chore/*`, `docs/*`, `refactor/*`, `test/*` | `release/*` |

Rules:

- Do not commit directly to `main`.
- Do not commit directly to `develop` except repository bootstrap work explicitly approved by maintainers.
- Normal PRs target `develop`, not `main`.
- `main` receives only release and emergency hotfix merges.

## Working branch names

Use Git Flow branch classes:

| Prefix | Base | PR target | Use for | Example |
|---|---|---|---|---|
| `feature/` | `develop` | `develop` | Product feature or substantial capability | `feature/mcp-resource-cache` |
| `bugfix/` | `develop` | `develop` | Non-emergency bug fix for next release | `bugfix/raw-ref-invalid-utf8` |
| `hotfix/` | `main` | `main` and back-merge to `develop` | Emergency production fix | `hotfix/redaction-leak` |
| `release/` | `develop` | `main`, then back-merge/tag to `develop` | Release stabilization | `release/0.2.0` |
| `docs/` | `develop` | `develop` | Documentation-only change | `docs/git-flow-policy` |
| `refactor/` | `develop` | `develop` | Internal structure change without intended behavior change | `refactor/cli-modules` |
| `test/` | `develop` | `develop` | Test-only or fixture-only work | `test/mcp-session-scope` |
| `chore/` | `develop` | `develop` | CI, dependency, repo hygiene | `chore/github-templates` |

Deprecated/exception branch class:

- `codex/*` may exist only for legacy or scratch AI-agent branches. Do not open new long-lived PRs from `codex/*`. Convert real work to the proper Git Flow prefix before review.

## Branch rules

- One branch should represent one coherent story.
- Do not mix unrelated product work and repo-harness work unless the PR explicitly says it is a combined bootstrap PR.
- Do not force-push shared branches unless maintainers explicitly approve it.
- Do not create vague branch names such as `temp`, `test`, `fix`, `update`, or `agent-work`.
- If a branch starts from the wrong base, recreate it from the correct Git Flow base rather than hiding the mismatch in the PR body.

## Pull request direction

Normal flow:

```text
develop -> feature/<story> -> PR to develop -> release/<version> -> PR to main -> tag -> back-merge to develop
```

Hotfix flow:

```text
main -> hotfix/<story> -> PR to main -> tag -> back-merge/cherry-pick to develop
```


## Automatic work-unit closeout for agents

AI agents must treat Git/GitHub closeout as part of every completed work unit, not as a separate optional follow-up. Unless the user explicitly says not to commit, not to push, or not to touch GitHub, the agent must:

1. Verify the branch class and PR target match Git Flow.
2. Run the relevant local verification gate.
3. Stage only the completed work-unit files.
4. Commit with the required Conventional+Lore format.
5. Push the branch.
6. Create or update the draft PR.
7. Update the PR body with Git Flow metadata, scope, validation, and risk notes.
8. Check CI status and report whether it is passing, pending, or failing.

Safety limits:

- Do not merge PRs automatically unless the user/maintainer explicitly requests agent-driven merge for that PR.
- For explicit agent-driven merge, first recheck CI, merge state, target branch, and review evidence. GitHub blocks authors from approving their own PRs; agents must not claim self-approval as a GitHub approval. If the same agent authored the PR, document the self-approval limitation and cite independent review evidence collected outside GitHub review state before merging.
- Do not change branch protection or repository settings automatically.
- Do not delete remote branches or force-push shared history unless the task is explicitly history cleanup or branch hygiene.
- Do not include unrelated working-tree changes in a work-unit commit.

## Commit messages

TFY commits use a strict **Conventional intent line + Lore trailers** format.

### Subject line

```text
<type>(<scope>): <why this change exists>
```

Allowed `type` values:

- `feat` — user-visible capability
- `fix` — bug fix
- `docs` — documentation-only change
- `refactor` — internal structure, no intended behavior change
- `test` — tests/fixtures only
- `chore` — CI/repo/dependency/release maintenance
- `perf` — measured performance improvement
- `security` — security hardening or vulnerability fix

Subject rules:

- Use imperative, reason-focused wording.
- Keep it concise, ideally under 72 characters.
- Do not use vague subjects such as `update files`, `fix stuff`, `WIP`, `agent changes`, or `misc`.

### Required trailers

Every non-trivial commit must include these Lore trailers:

```text
Constraint: <external constraint that shaped the decision>
Rejected: <alternative considered> | <reason>
Confidence: <low|medium|high>
Scope-risk: <narrow|moderate|broad>
Directive: <future-facing warning>
Tested: <commands/evidence>
Not-tested: <known gaps>
```

`Rejected:` may be omitted only for tiny typo/docs-only commits where no real alternative exists. `Tested:` and `Not-tested:` are always required.

### Good commit example

```text
docs(git): make Git Flow the repo operating model

Codex-authored branches were previously allowed as primary PR branches. The repository needs a stable main/develop flow before more adapters and releases are added.

Constraint: Normal PRs must target develop while main remains release-only.
Rejected: Keep codex/* as the default branch class | it hides whether a change is feature, fix, docs, refactor, test, or chore work.
Confidence: high
Scope-risk: narrow
Directive: Convert AI-authored work to a Git Flow branch before review.
Tested: ./scripts/verify.sh
Not-tested: Branch protection settings were not changed through GitHub admin APIs.
```


## CI branch-protection target

The CI workflow uses path-aware heavy-job skipping for inert docs and static assets. If branch protection is enabled, require the stable aggregate `ci-required` check rather than optional lane jobs such as `rust`, `claim-docs-check`, or `npm preview smoke (...)`, because those jobs intentionally skip when their lane is not required.

Claim-bearing docs and matrices, such as command-support matrix files and public claim docs, are not treated as inert docs: CI routes them through the lightweight `claim-docs-check` lane (`node scripts/validate-command-support-matrix.js`) without forcing Rust or npm heavy lanes unless another changed path requires them.

Do not require skipped optional lane jobs directly unless the CI workflow is changed to make them always emit required statuses.

`assets/brand/**` is reserved for decorative brand artwork and other non-runtime identity assets. Do not place packaged runtime assets, test fixtures, release inputs, generated config, or behavior-affecting files there; use the appropriate code/package/test directory so CI routing can require Rust or npm validation.

## PR requirements

Every PR must include:

- source branch and target branch
- Git Flow class: feature, bugfix, hotfix, release, docs, refactor, test, or chore
- summary and rationale
- supported boundary and unsupported claims
- user/developer impact
- validation commands
- risk notes

Draft PR is preferred until CI is green and support claims are reviewed.
