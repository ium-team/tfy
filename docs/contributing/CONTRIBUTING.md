# Contributing to TFY

TFY is a Rust-first agent middleware project. Contributions must follow the mandatory Git Flow branch model and Conventional+Lore commit format in `docs/contributing/GIT_POLICY.md`.

## Quick workflow

```bash
git checkout develop
git pull --ff-only origin develop
git checkout -b feature/<short-story>
# edit, test, commit
git push -u origin feature/<short-story>
# open a PR to develop
```

Use `hotfix/<short-story>` from `main` only for emergency production fixes.

## Branches

Normal work targets `develop`:

- `feature/<short-story>` for product features.
- `bugfix/<short-story>` for non-emergency fixes.
- `docs/<short-story>` for documentation-only changes.
- `refactor/<short-story>` for structural changes without intended behavior changes.
- `test/<short-story>` for tests/fixtures.
- `chore/<short-story>` for CI, dependency, release, or repo hygiene.

Release/emergency work:

- `release/<version>` from `develop` to `main`, then back-merge/tag to `develop`.
- `hotfix/<short-story>` from `main` to `main`, then back-merge/cherry-pick to `develop`.

`codex/*` is scratch/legacy only. Convert real AI-authored work to the correct Git Flow prefix before review.

## Commit messages

Use the Conventional+Lore format from `docs/contributing/GIT_POLICY.md`.

Minimal example:

```text
docs(git): make Git Flow the repo operating model

Constraint: Normal PRs should target develop while main remains release-only.
Rejected: Keep codex/* as the default PR branch class | it hides the change type.
Confidence: high
Scope-risk: narrow
Directive: Convert AI-authored work to a Git Flow branch before review.
Tested: ./scripts/verify.sh
Not-tested: Branch protection settings were not changed through GitHub admin APIs.
```

## Pull requests

Every PR should describe:

- source and target branch
- Git Flow class
- what changed
- why it changed
- supported vs unsupported runtime claims
- validation commands
- user/developer impact
- risk notes

Prefer draft PRs while an adapter boundary or output contract is still being reviewed.


## Automatic Git/GitHub closeout

For agent-authored work, committing, pushing, and updating the draft PR are part of “done.” A separate user reminder is not required unless the user explicitly opted out of Git/GitHub actions.

## Verification gate

Run before requesting review:

```bash
./scripts/verify.sh
```

Equivalent commands:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

If any command cannot run, document the reason and the next-best evidence.

## Architecture expectations

- `main.rs` should stay dispatch-focused.
- Transport-specific code belongs in a transport module such as `mcp.rs`.
- Adapter-specific install/report behavior belongs in `adapter.rs`.
- Reusable token-saving behavior belongs in `tfy-core`.
- Runtime envelope types belong in `tfy-runtime`.

## Claim discipline

Do not claim that TFY automatically intercepts a runtime unless the runtime adapter exists and has e2e tests. Current supported boundaries are CLI/generic-shell routing and MCP host routing.
