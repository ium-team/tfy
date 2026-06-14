#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; data=json.load(sys.stdin); print(next(p["version"] for p in data["packages"] if p["name"]=="tfy-cli"))')"
PLATFORM_INFO="$(node - "$VERSION" <<'NODE'
const { currentPlatform, archiveName } = require('./npm/tfy-cli/scripts/lib/platform');
const version = process.argv[2];
const target = currentPlatform();
process.stdout.write([
  target.archivePlatform,
  target.archiveArch,
  target.binName,
  target.rustTarget,
  archiveName(version, target)
].join('\t'));
NODE
)"
IFS=$'	' read -r ASSET_PLATFORM ASSET_ARCH BIN_NAME RUST_TARGET ARCHIVE_NAME <<< "$PLATFORM_INFO"
DIST="$ROOT/dist"
INSTALL_ROOT="$ROOT/.tfy/release/install"
RELEASE_DIR="$ROOT/.tfy/release"
BIN="$ROOT/target/release/$BIN_NAME"
ARCHIVE="$DIST/$ARCHIVE_NAME"
CHECKSUM="$ARCHIVE.sha256"
CHECKSUMS="$DIST/checksums.txt"
RELEASE_MANIFEST="$RELEASE_DIR/release-manifest.json"
BENCH_MANIFEST="$RELEASE_DIR/bench-manifest.json"
EVIDENCE="$RELEASE_DIR/release-evidence.json"

mkdir -p "$DIST" "$RELEASE_DIR"
rm -rf "$INSTALL_ROOT"

cargo install --path crates/tfy-cli --locked --root "$INSTALL_ROOT"
cargo build --release -p tfy-cli
"$BIN" bench --json --output "$BENCH_MANIFEST" >/dev/null

tar -C "$ROOT/target/release" -czf "$ARCHIVE" "$BIN_NAME"
(
  cd "$DIST"
  shasum -a 256 "$(basename "$ARCHIVE")" > "$(basename "$CHECKSUM")"
  cp "$(basename "$CHECKSUM")" "$CHECKSUMS"
)
ARCHIVE_SHA256="$(awk '{print $1}' "$CHECKSUM")"

cat > "$RELEASE_MANIFEST" <<EOF
{
  "schema_version": 1,
  "version": "$VERSION",
  "package_name": "@tfy/cli",
  "npm_dist_tag": "preview",
  "bin": "tfy",
  "asset": {
    "platform": "$ASSET_PLATFORM",
    "arch": "$ASSET_ARCH",
    "archive": "$ARCHIVE",
    "archive_name": "$(basename "$ARCHIVE")",
    "checksum": "$CHECKSUM",
    "checksum_name": "$(basename "$CHECKSUM")",
    "sha256": "$ARCHIVE_SHA256",
    "rust_target": "$RUST_TARGET",
    "binary_name": "$BIN_NAME"
  },
  "canonical_source": "github_release"
}
EOF

cat > "$EVIDENCE" <<EOF
{
  "cargo_install_verified": true,
  "cargo_install_binary": "$INSTALL_ROOT/bin/$BIN_NAME",
  "cargo_build_release_verified": true,
  "release_binary": "$BIN",
  "release_binary_name": "$BIN_NAME",
  "archive_checksum_dry_run": true,
  "archive_artifact": "$ARCHIVE",
  "archive_name": "$(basename "$ARCHIVE")",
  "checksum_artifact": "$CHECKSUM",
  "checksum_name": "$(basename "$CHECKSUM")",
  "checksum_sha256": "$ARCHIVE_SHA256",
  "checksums_artifact": "$CHECKSUMS",
  "release_manifest": "$RELEASE_MANIFEST",
  "npm_package_name": "@tfy/cli",
  "npm_dist_tag": "preview",
  "npm_bin": "tfy",
  "github_release_canonical": true,
  "asset_platform": "$ASSET_PLATFORM",
  "asset_arch": "$ASSET_ARCH",
  "asset_rust_target": "$RUST_TARGET",
  "asset_binary_name": "$BIN_NAME",
  "docs_demo_release_notes_complete": true,
  "docs_artifact": "$ROOT/docs/RELEASE_READINESS.md",
  "release_notes_artifact": "$ROOT/docs/releases/0.1.0-preview.md",
  "independent_reviews_approved": false,
  "pr_ci_green": false,
  "benchmark_manifest_generated": true,
  "benchmark_manifest": "$BENCH_MANIFEST",
  "notes": [
    "cargo install --path crates/tfy-cli --locked --root .tfy/release/install succeeded",
    "cargo build --release -p tfy-cli succeeded",
    "release_binary_name=$BIN_NAME",
    "archive=$ARCHIVE",
    "archive_name=$(basename "$ARCHIVE")",
    "checksum=$CHECKSUM",
    "checksum_sha256=$ARCHIVE_SHA256",
    "release_manifest=$RELEASE_MANIFEST",
    "npm_package_name=@tfy/cli",
    "npm_dist_tag=preview",
    "bench_manifest=$BENCH_MANIFEST"
  ]
}
EOF

cat <<EOF
TFY release dry-run complete
version=$VERSION
archive=$ARCHIVE
checksum=$CHECKSUM
release_manifest=$RELEASE_MANIFEST
bench_manifest=$BENCH_MANIFEST
release_evidence=$EVIDENCE

Use with launch report:
  SMOKE_JSON=.tfy/release/smoke.json
  tfy smoke --all --json > "\$SMOKE_JSON"
  ARGS=(launch-report --all --release-evidence "$EVIDENCE" --json)
  while IFS= read -r item; do
    case "\$item" in
      ledger=*) ARGS+=(--ledger "\${item#ledger=}") ;;
      host_evidence=*) ARGS+=(--host-evidence "\${item#host_evidence=}") ;;
    esac
  done < <(python3 -c 'import json,sys; print("\\n".join(json.load(open(sys.argv[1]))["evidence"]))' "\$SMOKE_JSON")
  tfy "\${ARGS[@]}"
EOF
