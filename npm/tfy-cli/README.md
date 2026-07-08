# TFY CLI Beta

This npm package installs the `tfy` command for TFY beta and stable releases. Beta and stable are maturity states for the same TFY product.

Install beta builds explicitly until the first stable release exists:

```sh
npm install -g @ium/tfy-cli@beta
```

Name mapping:

| Layer | Name |
| --- | --- |
| npm package | `@ium/tfy-cli` |
| Installed command | `tfy` |
| Rust CLI crate | `tfy-cli` |

TFY uses two forward public install channels only:

- Stable channel: `@ium/tfy-cli` resolves to the most tested completed release through the npm `latest` dist-tag, but only after the first stable release exists.
- Beta channel: `@ium/tfy-cli@beta` resolves to the newest development/validation build with current work included. Use this explicit `@beta` form while TFY has no stable release.

See the repository `docs/RELEASE_READINESS.md` for the canonical npm dist-tag cleanup and verification checklist.

Exact versions remain installable with standard npm syntax, for example `@ium/tfy-cli@0.1.1` or `@ium/tfy-cli@0.1.1-beta.0`; npm version installs use `@<version>`, not `/v<version>`. Historical `0.1.0-preview.*` releases remain exact-version history, not the forward release policy.

Supported prebuilt npm/GitHub Release platforms:

| OS | Architecture | Rust target | npm prebuilt |
| --- | --- | --- | --- |
| macOS | Apple Silicon arm64 | `aarch64-apple-darwin` | yes |
| Linux | x64 | `x86_64-unknown-linux-gnu` | yes |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | yes |
| Windows | x64 | `x86_64-pc-windows-msvc` | yes |

Intel Mac (`darwin:x64` / `x86_64-apple-darwin`) is not provided as a prebuilt npm/GitHub Release archive. Intel Mac users can still build from source with `git clone https://github.com/ium-team/tfy && cd tfy && cargo install --path crates/tfy-cli`, or run a locally built binary by setting `TFY_BINARY_PATH`.

The npm package name is `@ium/tfy-cli` and differs from the command name because the unscoped `tfy` npm package is already occupied and unscoped `tfy-cli` is blocked by npm similarity policy. The installed executable remains `tfy`.

This is not GA/production-ready until stable evidence gates pass. TFY does not claim private Codex hook interception, provider prompt proxying, editor auto-hooks, or universal terminal interception. Setup success is not token-savings success; route-bound raw/ledger/no-negative/positive-savings evidence is still required.

The installer downloads from the matching GitHub Release tag and verifies the `.sha256` file before installing. Beta versions keep the full beta tag, for example `v0.1.1-beta.0`, while release asset names use the base version, for example `tfy-0.1.1-linux-x86_64.tar.gz`. Historical preview versions use their historical tags.

After a successful install, npm prints opt-in human setup guidance only. It does **not** edit `.bashrc`, `.zshrc`, PowerShell profiles, fish config, or any other shell startup file. On supported platform shells (Linux bash, macOS zsh, Windows PowerShell), interactive `tfy start --human` marks the current repo for human mode, then offers a default-No prompt to install the one-time future-shell hook if it is missing. You can also run `tfy setup --human` to dry-run that hook install, or `tfy setup --human --apply` to enable auto-activation later only for trusted repos that have a TFY marker from `tfy start --human`.


## Publishing note

This package intentionally defaults `publishConfig.tag` to `beta` so accidental `npm publish` does not promote a beta build to stable. Maintainers should run the root helper before publishing:

```sh
node scripts/npm-publish-plan.js --version 0.1.1-beta.0 --channel beta --source-ref develop
node scripts/npm-publish-plan.js --version 0.1.1 --channel stable --source-ref main
```

The helper validates the channel/version/source metadata and prints the exact `npm publish` command; it does not publish. Beta publishes must keep using `--tag beta`; `latest` remains stable-only.
