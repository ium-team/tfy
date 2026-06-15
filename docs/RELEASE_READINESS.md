# TFY Release Readiness

TFY release claims are evidence-gated. The product can be shipped as a Developer Preview/RC when local supported routes and release artifacts pass; GA and public superiority claims require stricter evidence.

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

TFY uses two public npm install channels only: stable and public-test. Stable publishes use the `latest` dist-tag so `npm install -g token-fuck-you` resolves to the most tested release. Public-test publishes must be public and use the `preview` dist-tag, not `latest`, so `npm install -g token-fuck-you@preview` resolves to the newest public testing build. Exact historical installs use standard npm version specifiers such as `token-fuck-you@0.1.1` or `token-fuck-you@0.1.1-preview.0`; TFY does not use a slash form such as `token-fuck-you/v0.1.1` for npm installs. The package must fail closed when the release archive or checksum is missing or mismatched. Release evidence should record at least `npm_package_name`, `npm_dist_tag`, `npm_bin`, `github_release_canonical`, asset platform/arch/name, and checksum file/hash.

Required local checks for the npm path:

```sh
./scripts/npm-preview-smoke.sh
./scripts/release-dry-run.sh
```


## Manual GitHub Release workflow

A human-controlled GitHub Release workflow lives at `.github/workflows/release.yml`. It is intentionally manual-only (`workflow_dispatch`) and does not publish npm. The workflow must exist on the repository default branch before it appears in the GitHub Actions manual-run UI; choose the release source with the `source_ref` input.

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

Preview archive names intentionally omit the `-preview.N` suffix because the npm installer resolves the full tag while using base-version asset names. For example, `token-fuck-you@0.1.1-preview.0` downloads from tag `v0.1.1-preview.0` and expects `tfy-0.1.1-linux-x86_64.tar.gz` plus `.sha256`.

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
