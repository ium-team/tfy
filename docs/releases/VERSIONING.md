# TFY Release Versioning Guide

This guide is the plain-language rulebook for choosing TFY release versions.
Use it before running the manual GitHub Release workflow or publishing npm.

TFY intentionally has only two public release channels:

1. **Stable** — the safer, already-tested public release.
2. **Public-test / preview** — the newest public testing release for dogfooding.

There is no third channel for nightly, canary, alpha, or experimental releases unless the project explicitly adds one later.

## Quick decision tree

Use this table first:

| Situation | Version to make | Channel | npm install users run |
| --- | --- | --- | --- |
| First public testing build for a planned stable version | `N.N.N-preview.0` | `preview` | `npm install -g token-fuck-you@preview` |
| Another public testing build for the same planned stable version | `N.N.N-preview.1`, then `.2`, `.3`, ... | `preview` | `npm install -g token-fuck-you@preview` |
| The preview has been tested enough and should become the safer release | `N.N.N` | `stable` | `npm install -g token-fuck-you` |
| Small fix after a stable release | Next patch, usually `N.N.(N+1)-preview.0` first | `preview` | `npm install -g token-fuck-you@preview` |
| New user-visible feature set | Next minor, usually `N.(N+1).0-preview.0` first | `preview` | `npm install -g token-fuck-you@preview` |
| Breaking CLI/config/API change | Next major, usually `(N+1).0.0-preview.0` first | `preview` | `npm install -g token-fuck-you@preview` |

## Version formats

### Stable versions

Stable versions must look like this:

```text
N.N.N
```

Examples:

```text
0.1.0
0.1.1
0.2.0
1.0.0
```

Stable release metadata:

| Field | Rule |
| --- | --- |
| Release channel | `stable` |
| npm dist-tag | `latest` |
| GitHub tag | `vN.N.N` |
| Git source ref | `main` only |
| Cargo workspace version | `N.N.N` |
| npm package version | `N.N.N` |

Important: npm calls the stable dist-tag `latest`. In TFY, `latest` means “our stable channel”, not “publish every newest preview here”. Do not publish preview builds to the npm `latest` tag.

### Public-test / preview versions

Preview versions must look like this:

```text
N.N.N-preview.N
```

Examples:

```text
0.1.0-preview.0
0.1.0-preview.1
0.1.1-preview.0
0.2.0-preview.0
```

Preview release metadata:

| Field | Rule |
| --- | --- |
| Release channel | `preview` |
| npm dist-tag | `preview` |
| GitHub tag | `vN.N.N-preview.N` |
| Git source ref | `develop` or `release/*` |
| Cargo workspace version | base version only: `N.N.N` |
| npm package version | full preview version: `N.N.N-preview.N` |

Example:

| File/system | Value for `0.1.0-preview.2` |
| --- | --- |
| Cargo workspace version | `0.1.0` |
| npm package version | `0.1.0-preview.2` |
| GitHub tag | `v0.1.0-preview.2` |
| npm dist-tag | `preview` |

Preview archive filenames use the base version, not the full preview suffix. For example, `0.1.0-preview.2` creates assets like:

```text
tfy-0.1.0-linux-x86_64.tar.gz
tfy-0.1.0-linux-x86_64.tar.gz.sha256
```

The GitHub Release tag still includes the full preview suffix:

```text
v0.1.0-preview.2
```

## What number should increase?

TFY uses the normal `MAJOR.MINOR.PATCH` idea:

```text
MAJOR.MINOR.PATCH
```

For `0.1.3`:

- `0` is MAJOR.
- `1` is MINOR.
- `3` is PATCH.

### Increase PATCH for small fixes

Use a patch bump when the change fixes or sharpens the current release line without adding a large feature set.

Examples:

- Fix npm installer behavior.
- Fix GitHub Release packaging.
- Fix Windows archive behavior.
- Drop or document an unsupported prebuilt target.
- Fix docs or release metadata.

Example flow:

```text
0.1.0-preview.0
0.1.0-preview.1
0.1.0
0.1.1-preview.0
0.1.1
```

### Increase MINOR for new features

Use a minor bump when users get new behavior, but existing commands/configs are still intended to work.

Examples:

- Add a new `tfy` subcommand.
- Add a new supported host integration.
- Add a new gateway capability.
- Add a meaningful user-visible workflow.

Example flow:

```text
0.1.1
0.2.0-preview.0
0.2.0-preview.1
0.2.0
```

### Increase MAJOR for breaking changes

Use a major bump when existing users may need to change commands, config, or automation.

Examples:

- Remove or rename an existing command.
- Change config file format incompatibly.
- Change npm package/install contract incompatibly.
- Break public CLI or machine-readable output contracts.

Example flow:

```text
0.9.4
1.0.0-preview.0
1.0.0-preview.1
1.0.0
```

While TFY is still `0.x`, avoid major bumps unless there is a very clear breaking public contract decision.

## How preview numbers work

The number after `preview.` counts public testing builds for the same planned stable version.

Example:

```text
0.1.1-preview.0  # first public test candidate for 0.1.1
0.1.1-preview.1  # second public test candidate for 0.1.1
0.1.1-preview.2  # third public test candidate for 0.1.1
0.1.1            # stable release after enough testing
```

Do not publish `0.1.1-preview.0` again after it already exists. Every public test retry for the same base version gets the next preview number.

## Recommended TFY release flow

For early TFY releases, use this simple flow:

```text
0.1.0-preview.0  # first public test
0.1.0-preview.1  # fix issues found in first public test
0.1.0-preview.2  # fix more issues if needed
0.1.0            # stable once tested enough
0.1.1-preview.0  # next small fix cycle
0.1.1            # stable patch release
0.2.0-preview.0  # next feature cycle
0.2.0            # stable feature release
```

When unsure, choose preview first. Stable should be the version that has already survived public-test/manual testing.

## Manual Release workflow input examples

### Preview dry-run

Use this before creating the real GitHub Release:

```text
version: 0.1.0-preview.0
channel: preview
source_ref: develop
dry_run: true
```

### Preview real GitHub Release

Use this after the dry-run passes:

```text
version: 0.1.0-preview.0
channel: preview
source_ref: develop
dry_run: false
```

### Stable dry-run

Stable releases must come from `main`:

```text
version: 0.1.0
channel: stable
source_ref: main
dry_run: true
```

### Stable real GitHub Release

Use this after the stable dry-run passes:

```text
version: 0.1.0
channel: stable
source_ref: main
dry_run: false
```

## Files that must agree

Before a release, the version metadata must agree with the channel.

For preview `0.1.0-preview.0`:

| Location | Required value |
| --- | --- |
| `Cargo.toml` workspace package version | `0.1.0` |
| `npm/token-fuck-you/package.json` version | `0.1.0-preview.0` |
| Manual Release workflow `version` input | `0.1.0-preview.0` |
| Manual Release workflow `channel` input | `preview` |
| Manual Release workflow `source_ref` input | `develop` or `release/*` |

For stable `0.1.0`:

| Location | Required value |
| --- | --- |
| `Cargo.toml` workspace package version | `0.1.0` |
| `npm/token-fuck-you/package.json` version | `0.1.0` |
| Manual Release workflow `version` input | `0.1.0` |
| Manual Release workflow `channel` input | `stable` |
| Manual Release workflow `source_ref` input | `main` |

The helper script checks this:

```sh
node scripts/check-release-version.js --version 0.1.0-preview.0 --channel preview --source-ref develop --json
node scripts/check-release-version.js --version 0.1.0 --channel stable --source-ref main --json
```

## npm install forms

Use npm's `@version` or `@tag` syntax:

```sh
npm install -g token-fuck-you              # stable channel, npm dist-tag latest
npm install -g token-fuck-you@preview      # public-test channel
npm install -g token-fuck-you@0.1.0        # exact stable version
npm install -g token-fuck-you@0.1.0-preview.0  # exact preview version
```

Do not use slash-style version installs:

```sh
npm install -g token-fuck-you/v0.1.0       # wrong for npm package versions
```

## Naming reminder

The npm package name and command name are different on purpose:

| Layer | Name |
| --- | --- |
| npm package | `token-fuck-you` |
| installed command | `tfy` |
| GitHub Release tag | `vN.N.N` or `vN.N.N-preview.N` |
