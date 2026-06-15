# TFY CLI Developer Preview

This npm package installs the `tfy` command for TFY Developer Preview dogfood/testing.

Install preview builds explicitly:

```sh
npm install -g @ium/tfy-cli@preview
```

Name mapping:

| Layer | Name |
| --- | --- |
| npm package | `@ium/tfy-cli` |
| Installed command | `tfy` |
| Rust CLI crate | `tfy-cli` |

TFY uses two public install channels only:

- Stable channel: `@ium/tfy-cli` resolves to the most tested release through the npm `latest` dist-tag.
- Public-test channel: `@ium/tfy-cli@preview` resolves to the newest public testing build with current development work included.

Exact versions remain installable with standard npm syntax, for example `@ium/tfy-cli@0.1.1` or `@ium/tfy-cli@0.1.1-preview.0`; npm version installs use `@<version>`, not `/v<version>`.

Supported prebuilt npm/GitHub Release platforms:

| OS | Architecture | Rust target | npm prebuilt |
| --- | --- | --- | --- |
| macOS | Apple Silicon arm64 | `aarch64-apple-darwin` | yes |
| Linux | x64 | `x86_64-unknown-linux-gnu` | yes |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | yes |
| Windows | x64 | `x86_64-pc-windows-msvc` | yes |

Intel Mac (`darwin:x64` / `x86_64-apple-darwin`) is not provided as a prebuilt npm/GitHub Release archive. Intel Mac users can still build from source with `git clone https://github.com/ium-team/tfy && cd tfy && cargo install --path crates/tfy-cli`, or run a locally built binary by setting `TFY_BINARY_PATH`.


The npm package name is `@ium/tfy-cli` and differs from the command name because the unscoped `tfy` npm package is already occupied and unscoped `tfy-cli` is blocked by npm similarity policy. The installed executable remains `tfy`.

This is not GA/production-ready. TFY does not claim private Codex hook interception, provider prompt proxying, editor auto-hooks, or universal terminal interception. Setup success is not token-savings success; route-bound raw/ledger/no-negative/positive-savings evidence is still required.

The installer downloads from the matching GitHub Release tag and verifies the `.sha256` file before installing. Preview versions keep the full preview tag, for example `v0.1.1-preview.0`, while release asset names use the base version, for example `tfy-0.1.1-linux-x86_64.tar.gz`.


## Publishing note

This package intentionally defaults `publishConfig.tag` to `preview` so accidental `npm publish` does not promote a public-test build to stable. Maintainers should run the root helper before publishing:

```sh
node scripts/npm-publish-plan.js --version 0.1.1-preview.0 --channel preview --source-ref develop
node scripts/npm-publish-plan.js --version 0.1.1 --channel stable --source-ref main
```

The helper validates the channel/version/source metadata and prints the exact `npm publish` command; it does not publish.
