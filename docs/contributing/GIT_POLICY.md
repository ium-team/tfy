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
