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

Preview npm publishes must be public and use the `preview` dist-tag, not `latest`. The package must fail closed when the release archive or checksum is missing or mismatched. Release evidence should record at least `npm_package_name`, `npm_dist_tag`, `npm_bin`, `github_release_canonical`, asset platform/arch/name, and checksum file/hash.

Required local checks for the npm path:

```sh
./scripts/npm-preview-smoke.sh
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
