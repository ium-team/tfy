<!-- Follow AGENTS.md and docs/contributing/GIT_POLICY.md. -->

## Git Flow

- Source branch: <!-- feature/... bugfix/... docs/... etc. -->
- Target branch: <!-- normally develop; main only for release/hotfix -->
- Change class: <!-- feature | bugfix | hotfix | release | docs | refactor | test | chore -->

## Summary

<!-- What changed and why? -->

## Supported boundary / claim discipline

<!-- What does this PR support? What does it explicitly not claim? -->

## User or developer impact

<!-- How does this affect TFY users, agent hosts, or contributors? -->

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `git diff --check`

## Risk notes

<!-- Output contract, token-savings, raw recovery, security/redaction, adapter compatibility risks. -->
