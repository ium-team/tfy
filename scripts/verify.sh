#!/usr/bin/env sh
set -eu
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm test --prefix npm/token-fuck-you
node scripts/validate-command-support-matrix.js
git diff --check
