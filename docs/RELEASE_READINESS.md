# TFY Release Readiness

TFY release claims are evidence-gated. Developer Preview and RC are separate gates: Developer Preview proves local supported routes plus installable artifacts, while RC additionally proves review/CI/release closeout. GA and public superiority claims require stricter evidence.

## Release tiers

- `developer_preview_ready`: `cargo install`/release build verified, npm preview install smoke passes when the npm channel is in scope, GitHub Release archive/checksum evidence exists, first-success quickstart passes, required routes (`generic_shell`, `tfy_agent_adapter`, `mcp_stdio`) reach `launch_supported`, raw lifecycle commands work, a TFY benchmark manifest exists, and unsupported claim audit passes.
- `rc_ready`: developer preview ready plus archive/checksum dry-run, docs/demo/release notes complete, independent reviews approved, and PR/CI green.
- `ga_ready`: RC ready plus at least one named AI host real invocation with route-bound ledger/raw/no-negative/positive-savings evidence and a reproducible named-host demo.
- `public_superiority_claim_ready`: GA/RC plus reviewed RTK comparator manifest with version, mode, corpus, reproducibility, correctness/no-lost-evidence proof, and overhead comparison.

## Commands

```sh
./scripts/verify.sh
./scripts/release-dry-run.sh
SMOKE_JSON=.tfy/release/smoke.json
tfy smoke --all --json > "$SMOKE_JSON"
tfy raw --list --json
ARGS=(launch-report --all --release-evidence .tfy/release/release-evidence.json --json)
while IFS= read -r item; do
  case "$item" in
    ledger=*) ARGS+=(--ledger "${item#ledger=}") ;;
    host_evidence=*) ARGS+=(--host-evidence "${item#host_evidence=}") ;;
  esac
done < <(python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["evidence"]))' "$SMOKE_JSON")
tfy "${ARGS[@]}"
```

`launch-report` accepts `--host-evidence` for route/host proofs and `--release-evidence` for packaging/docs/review/CI proofs. It keeps GA blocked unless a named AI host has real invocation evidence.


## Developer Preview distribution

Preview distribution is intentionally two-channel:

- GitHub Releases are the canonical binary source. Each supported platform archive must have a SHA-256 checksum and release evidence entry.
- npm is the convenience installer/launcher. The package name may be scoped or otherwise disambiguated from occupied npm names, but the installed binary command must be `tfy`.

TFY uses two public npm install channels only: stable and public-test. Stable publishes use the `latest` dist-tag so `npm install -g @ium/tfy-cli` resolves to the most tested release, but only after the first stable release exists. Public-test publishes must be public and use the `preview` dist-tag, not `latest`, so `npm install -g @ium/tfy-cli@preview` resolves to the newest public testing build. While no stable release exists, maintainers must document only the explicit `@preview` install form. If any preview appears on the npm `latest` dist-tag, remove it manually with `npm dist-tag rm @ium/tfy-cli latest`, then verify with `npm dist-tag ls @ium/tfy-cli` that preview releases are reachable through `preview` only. This registry cleanup is an authenticated maintainer action, not something the GitHub Release workflow performs. Exact historical installs use standard npm version specifiers such as `@ium/tfy-cli@0.1.1` or `@ium/tfy-cli@0.1.1-preview.0`; TFY does not use a slash form such as `@ium/tfy-cli/v0.1.1` for npm installs. The package must fail closed when the release archive or checksum is missing or mismatched. Release evidence should record at least `npm_package_name`, `npm_dist_tag`, `npm_bin`, `github_release_canonical`, asset platform/arch/name, and checksum file/hash. The package defaults `publishConfig.tag` to `preview` as a safety rail; stable publishes must explicitly use `--tag latest`, and preview packages must never be advertised through `latest`.

Supported prebuilt npm/GitHub Release platforms:

| OS | Architecture | Rust target | npm prebuilt |
| --- | --- | --- | --- |
| macOS | Apple Silicon arm64 | `aarch64-apple-darwin` | yes |
| Linux | x64 | `x86_64-unknown-linux-gnu` | yes |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | yes |
| Windows | x64 | `x86_64-pc-windows-msvc` | yes |

Intel Mac (`darwin:x64` / `x86_64-apple-darwin`) is not provided as a prebuilt npm/GitHub Release archive. Intel Mac users can still build from source with `git clone https://github.com/ium-team/tfy && cd tfy && cargo install --path crates/tfy-cli`, or run a locally built binary by setting `TFY_BINARY_PATH`.


Required local checks for the npm path:

```sh
./scripts/npm-preview-smoke.sh
./scripts/release-dry-run.sh
node scripts/npm-publish-plan.js --version 0.1.1-preview.0 --channel preview --source-ref develop
node scripts/npm-publish-plan.js --version 0.1.1 --channel stable --source-ref main
```

After an npm publish, the workflow runs the dist-tag guard printed by the publish plan. For preview releases it verifies that `preview` points at the release version; until the first stable release exists, the workflow explicitly tolerates the historical npm registry state where npm refused to delete a preview `latest` tag. Stable releases must move `latest` to the stable release version. Maintainers can also run the strict guard locally:

```sh
node scripts/npm-dist-tag-check.js --version 0.1.1-preview.0 --channel preview
node scripts/npm-dist-tag-check.js --version 0.1.1 --channel stable
```

External setup required before automated npm publishing can pass:

1. In npm, configure Trusted Publishing for package `@ium/tfy-cli` to trust GitHub organization/user `ium-team`, repository `tfy`, workflow filename `release.yml`, and the `npm publish` action.
2. Inspect `npm dist-tag ls @ium/tfy-cli`; if `latest` points at a preview version and npm allows removal, remove it once with `npm dist-tag rm @ium/tfy-cli latest`. If npm refuses to delete `latest`, keep documenting preview installs explicitly and let the first real stable release move `latest` to stable.
3. Do not add a long-lived npm publish token unless Trusted Publishing is unavailable and the repository explicitly accepts that operational risk.


See [TFY Release Versioning Guide](releases/VERSIONING.md) for the plain-language rules for choosing `N.N.N` vs `N.N.N-preview.N`, when to bump patch/minor/major, and which files must agree before release.

## Manual GitHub Release workflow

A human-controlled GitHub Release workflow lives at `.github/workflows/release.yml`. It is intentionally manual-only (`workflow_dispatch`) and, when `dry_run=false`, creates the GitHub Release first and then publishes the npm installer with the channel-derived dist-tag. `preview` publishes use `--tag preview`; stable publishes use `--tag latest`. The npm publish job uses npm Trusted Publishing / GitHub OIDC (`id-token: write`) and therefore requires the npm package to trust this repository workflow before the job can succeed. The workflow must exist on the repository default branch before it appears in the GitHub Actions manual-run UI; choose the release source with the `source_ref` input.

Inputs:

- `version`: stable versions use `N.N.N`; public-test versions use `N.N.N-preview.N`.
- `channel`: `stable` or `preview` only.
- `source_ref`: optional. Stable defaults to `main` and refuses any other source. Preview defaults to `develop` and also allows `release/*`.
- `dry_run`: when true, builds/verifies/packages all assets but does not create the GitHub Release.

Metadata contract:

| Channel | npm dist-tag | Git tag | Cargo version | npm version | Archive names |
| --- | --- | --- | --- | --- | --- |
| stable | `latest` | `vN.N.N` | `N.N.N` | `N.N.N` | `tfy-N.N.N-<platform>-<arch>.tar.gz` |
| preview | `preview` | `vN.N.N-preview.N` | `N.N.N` | `N.N.N-preview.N` | `tfy-N.N.N-<platform>-<arch>.tar.gz` |

Preview archive names intentionally omit the `-preview.N` suffix because the npm installer resolves the full tag while using base-version asset names. For example, `@ium/tfy-cli@0.1.1-preview.0` downloads from tag `v0.1.1-preview.0` and expects `tfy-0.1.1-linux-x86_64.tar.gz` plus `.sha256`.

The workflow fails closed when version metadata is inconsistent, when `source_ref` does not match the channel, when the tag or release already exists, or when any current npm-supported platform asset/checksum is missing. The supported matrix is shared with the npm installer in `npm/tfy-cli/scripts/lib/platform.js`.

Local preflight examples:

```sh
node scripts/check-release-version.js --version 0.1.0-preview.0 --channel preview --source-ref develop --json
node scripts/check-release-version.js --version 0.1.0 --channel stable --source-ref main --cargo-version 0.1.0 --npm-version 0.1.0 --json
./scripts/release-dry-run.sh
```

## Raw lifecycle

Raw evidence is local and recoverable:

- `tfy raw <raw_ref>` recovers exact bytes.
- `tfy raw --list --json` lists local raw refs.
- `tfy raw <raw_ref> --inspect --json` inspects metadata without printing raw bytes.
- `tfy raw <raw_ref> --export <dir>` exports raw evidence JSON.
- `tfy raw --prune --dry-run --json` previews deletion.
- `tfy raw --prune --apply --json` deletes selected raw evidence only after explicit apply.

## Claim boundaries

Do not claim provider/API prompt proxying, private Codex hook interception, universal terminal interception, editor auto-hook integration, named-host launch support, or RTK superiority unless the matching launch-report/benchmark evidence gate passes.
