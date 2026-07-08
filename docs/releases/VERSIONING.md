# TFY Release Versioning Guide

TFY uses one product line with two forward release maturity states:

1. **Beta** — development/validation builds that are publicly testable but not the default stable install.
2. **Stable** — completed normal releases with no prerelease suffix.

The version number communicates the planned feature/release line. The suffix communicates maturity.

## Quick rule

| Situation | Version format | npm dist-tag | Install command |
| --- | --- | --- | --- |
| First beta for a planned stable version | `N.N.N-beta.0` | `beta` | `npm install -g @ium/tfy-cli@beta` |
| Another beta for the same planned stable version | `N.N.N-beta.1`, then `.2`, `.3`, ... | `beta` | `npm install -g @ium/tfy-cli@beta` |
| The beta has been validated enough and becomes the completed release | `N.N.N` | `latest` | `npm install -g @ium/tfy-cli` |
| Small fix after a stable release | Next patch, usually `N.N.(N+1)-beta.0` first | `beta` | `npm install -g @ium/tfy-cli@beta` |
| New user-visible feature set | Next minor, usually `N.(N+1).0-beta.0` first | `beta` | `npm install -g @ium/tfy-cli@beta` |
| Breaking CLI/config/API change | Next major, usually `(N+1).0.0-beta.0` first | `beta` | `npm install -g @ium/tfy-cli@beta` |

Examples:

```text
0.1.1-beta.0  # first beta for the 0.1.1 feature line
0.1.1-beta.1  # second beta for that same feature line
0.1.1         # completed stable 0.1.1
0.1.2-beta.0  # next patch/fix line begins in beta
```

Historical note: `0.1.0-preview.*` releases exist from the earlier naming policy. They remain historical exact-version releases, but new development/validation builds use `beta`.

## Stable versions

Stable versions have no suffix:

```text
N.N.N
```

Examples:

```text
0.1.1
0.2.0
1.0.0
```

Stable metadata:

| Field | Value |
| --- | --- |
| Release channel | `stable` |
| npm dist-tag | `latest` |
| GitHub tag | `vN.N.N` |
| Cargo workspace version | `N.N.N` |
| npm package version | `N.N.N` |

Important: npm calls the stable dist-tag `latest`. In TFY, `latest` means “our stable channel,” not “publish every newest beta here.” Do not publish beta builds to the npm `latest` tag.

## Beta versions

Beta versions use the SemVer prerelease suffix:

```text
N.N.N-beta.N
```

Examples:

```text
0.1.1-beta.0
0.1.1-beta.1
0.2.0-beta.0
```

Beta metadata:

| Field | Value |
| --- | --- |
| Release channel | `beta` |
| npm dist-tag | `beta` |
| GitHub tag | `vN.N.N-beta.N` |
| Cargo workspace version | base version: `N.N.N` |
| npm package version | full beta version: `N.N.N-beta.N` |

Example for `0.1.1-beta.0`:

| File/system | Value |
| --- | --- |
| `Cargo.toml` workspace package version | `0.1.1` |
| `npm/tfy-cli/package.json` version | `0.1.1-beta.0` |
| Manual Release workflow `version` input | `0.1.1-beta.0` |
| Manual Release workflow `channel` input | `beta` |
| npm dist-tag | `beta` |

Beta archive filenames use the base version, not the full beta suffix. For example, `0.1.1-beta.0` creates assets like:

```text
tfy-0.1.1-linux-x86_64.tar.gz
tfy-0.1.1-linux-x86_64.tar.gz.sha256
```

The GitHub Release tag still includes the full beta suffix:

```text
v0.1.1-beta.0
```

## Which number changes?

TFY is still pre-1.0, but the same intent rules apply.

### Patch bump

Use when the change is a bug fix, docs/release policy cleanup, small compatibility fix, or a narrow behavior correction.

```text
0.1.1-beta.0
0.1.1
0.1.2-beta.0
0.1.2
```

### Minor bump

Use when the change adds a meaningful user-visible feature set or new mode.

```text
0.1.0
0.2.0-beta.0
0.2.0
```

### Major bump

Use when the change breaks CLI/config/API compatibility or changes core trust/security contracts.

```text
1.0.0-beta.0
1.0.0
```

## How beta numbers work

The number after `beta.` counts validation builds for the same planned stable version.

```text
0.1.1-beta.0  # first beta candidate for 0.1.1
0.1.1-beta.1  # second beta candidate for 0.1.1
0.1.1-beta.2  # third beta candidate for 0.1.1
```

Do not publish `0.1.1-beta.0` again after it already exists. Every public test retry for the same base version gets the next beta number.

## Common flows

### First stable not released yet

```text
0.1.0-preview.0  # historical old naming
0.1.0-preview.1  # historical old naming
0.1.1-beta.0     # current forward beta naming
0.1.1-beta.1     # beta fix if needed
0.1.1            # stable once validated enough
```

While no stable exists, the recommended install command is:

```sh
npm install -g @ium/tfy-cli@beta
```

### Feature line with beta validation

```text
0.1.1-beta.0  # feature implementation enters beta
0.1.1-beta.1  # fixes from beta feedback
0.1.1         # stable promotion
```

### Next feature set

```text
0.2.0-beta.0
0.2.0-beta.1
0.2.0
```

## Choosing beta vs stable

Choose **beta** when:

- The change is new enough that real install/use validation is still needed.
- The release includes a new user flow, adapter, host route, or release automation change.
- CI/local checks pass but the release has not survived public/manual use.
- There is no stable release yet.

Choose **stable** when:

- The same version line has passed beta validation or equivalent manual/dogfood evidence.
- Release artifacts, npm install, and docs have been verified.
- Supported route claims remain evidence-backed.
- Maintainers are comfortable making it the default `npm install -g @ium/tfy-cli` version.

When unsure, choose beta first. Stable should be the version that has already survived beta/manual testing.

## Manual Release workflow examples

Beta release from `develop`:

```text
version: 0.1.1-beta.0
channel: beta
source_ref: develop
```

Beta release from a stabilization branch:

```text
version: 0.1.1-beta.1
channel: beta
source_ref: release/0.1.1
```

Stable release:

```text
version: 0.1.1
channel: stable
source_ref: main
```

## Files that must agree

For beta `0.1.1-beta.0`:

| Place | Expected value |
| --- | --- |
| `Cargo.toml` workspace package version | `0.1.1` |
| `npm/tfy-cli/package.json` version | `0.1.1-beta.0` |
| Manual Release workflow `version` input | `0.1.1-beta.0` |
| Manual Release workflow `channel` input | `beta` |
| npm publish dist-tag | `beta` |

For stable `0.1.1`:

| Place | Expected value |
| --- | --- |
| `Cargo.toml` workspace package version | `0.1.1` |
| `npm/tfy-cli/package.json` version | `0.1.1` |
| Manual Release workflow `version` input | `0.1.1` |
| Manual Release workflow `channel` input | `stable` |
| npm publish dist-tag | `latest` |

## Install examples

```sh
npm install -g @ium/tfy-cli             # stable channel after stable exists
npm install -g @ium/tfy-cli@beta        # beta channel; use this while beta-only
npm install -g @ium/tfy-cli@0.1.1       # exact stable version
npm install -g @ium/tfy-cli@0.1.1-beta.0  # exact beta version
```

Historical preview versions, if needed, also use exact npm version syntax:

```sh
npm install -g @ium/tfy-cli@0.1.0-preview.1
```

Do not use slash-style package versions:

```sh
npm install -g @ium/tfy-cli/v0.1.1       # wrong for npm package versions
```

## Release helper checks

```sh
node scripts/check-release-version.js --version 0.1.1-beta.0 --channel beta --source-ref develop --json
node scripts/check-release-version.js --version 0.1.1 --channel stable --source-ref main --cargo-version 0.1.1 --npm-version 0.1.1 --json
node scripts/npm-publish-plan.js --version 0.1.1-beta.0 --channel beta --source-ref develop
node scripts/npm-dist-tag-check.js --version 0.1.1-beta.0 --channel beta
```

The release helper rejects mismatched channel/version/source combinations before packaging or publishing.

## Final checklist

Before release:

1. Pick the next version line based on feature/change size.
2. Use `-beta.N` until the version is completed/stable.
3. Keep Cargo base version and npm package version aligned by channel.
4. Run the release helper checks.
5. Run the full verification gate.
6. Use beta dist-tag for beta and latest dist-tag for stable.
