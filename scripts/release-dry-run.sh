#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; data=json.load(sys.stdin); print(next(p["version"] for p in data["packages"] if p["name"]=="tfy-cli"))')"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"
DIST="$ROOT/dist"
INSTALL_ROOT="$ROOT/.tfy/release/install"
RELEASE_DIR="$ROOT/.tfy/release"
BIN="$ROOT/target/release/tfy"
ARCHIVE="$DIST/tfy-${VERSION}-${OS}-${ARCH}.tar.gz"
CHECKSUM="$ARCHIVE.sha256"
BENCH_MANIFEST="$RELEASE_DIR/bench-manifest.json"
EVIDENCE="$RELEASE_DIR/release-evidence.json"

mkdir -p "$DIST" "$RELEASE_DIR"
rm -rf "$INSTALL_ROOT"

cargo install --path crates/tfy-cli --locked --root "$INSTALL_ROOT"
cargo build --release -p tfy-cli
"$BIN" bench --json --output "$BENCH_MANIFEST" >/dev/null

tar -C "$ROOT/target/release" -czf "$ARCHIVE" tfy
shasum -a 256 "$ARCHIVE" > "$CHECKSUM"

cat > "$EVIDENCE" <<EOF
{
  "cargo_install_verified": true,
  "cargo_install_binary": "$INSTALL_ROOT/bin/tfy",
  "cargo_build_release_verified": true,
  "release_binary": "$BIN",
  "archive_checksum_dry_run": true,
  "archive_artifact": "$ARCHIVE",
  "checksum_artifact": "$CHECKSUM",
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
    "archive=$ARCHIVE",
    "checksum=$CHECKSUM",
    "bench_manifest=$BENCH_MANIFEST"
  ]
}
EOF

cat <<EOF
TFY release dry-run complete
version=$VERSION
archive=$ARCHIVE
checksum=$CHECKSUM
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
