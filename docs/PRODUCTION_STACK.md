# TFY Production Stack

## Purpose

This document maps the final token-saving architecture to the Rust-only implementation surface for the first releasable version.

## Target stack

The product/runtime target is **Rust-only**:

- Core: Rust (`crates/tfy-core`)
- CLI: Rust binary (`crates/tfy-cli`, `tfy`)
- Tool Gateway: Rust CLI entrypoint (`tfy tool-gateway -- <command>`) over the existing command-output core
- Parsers: Tree-sitter grammars where available
- Tool feedback policy: Rust policy objects under the core crate
- Evaluation and benchmarks: Rust tests plus `criterion`
- Optional provider/model adapters: Rust adapter crates or external integrations, never correctness dependencies

Python is **not** a product/runtime stack. The former Python product surfaces (`src/tfy`, root `pyproject.toml`, Python tests, `bindings/tfy-python`, `pyo3`, and `uv.lock`) have been removed from the release path. Python remains only a supported source-code language for analysis fixtures through tree-sitter.

## Required architecture primitives

The production core should support:

- token-saving method registry
- representation ladder and adaptive retrieval policy
- artifact/ref store
- parser-backed semantic skeletons
- compact code and restoration contracts
- raw command-output store and range expansion
- incremental/delta state model
- compact schema dictionaries
- task/conversation ledger compaction
- provider adapter interface
- evaluation gate reporting

## Production invariants

- CLI-first JSON/text protocol remains canonical.
- Core behavior remains agent-neutral.
- Optional adapters cannot be required for correctness.
- Parser-backed responses include confidence, parser identity, and fallback action.
- Raw/full fallback is always available for high-risk artifacts.
- Public summaries redact secrets; original command/output evidence remains local behind raw refs.
- Current implementation status must be reported honestly.

## Current Rust smoke commands

```sh
cargo run -p tfy-cli -- languages
cargo run -p tfy-cli -- index corpus/rust/fixture_01.rs
cargo run -p tfy-cli -- expand corpus/rust/fixture_01.rs fixture_01.rs:calculate_discount_1:5 --compactness symbol
cargo run -p tfy-cli -- restore --payload payload.json
cargo run -p tfy-cli -- tool-gateway -- sh -c 'printf ok'
cargo run -p tfy-cli -- raw <raw_ref>
cargo run -p tfy-cli -- eval-code corpus/rust/fixture_01.rs fixture_01.rs:calculate_discount_1:5
cargo test --quiet
```

## Retired Python product artifacts

The Rust-only implementation removed the former Python product/runtime surfaces from the release path:

- `src/tfy/*`
- `tests/*.py`
- root `pyproject.toml`
- `uv.lock`
- `bindings/tfy-python`
- `pyo3` workspace dependency

Do not reintroduce Python as a product/runtime dependency without a new explicit compatibility decision.

## Release readiness

A production surface is release-ready only when `EVALUATION_GATES.md` passes for its method families. Token savings without correctness/evidence gates are not sufficient.


## Agent middleware stack direction

TFY's production integration model is AI-agent I/O middleware. The Rust core owns correctness; adapters automate invocation for specific runtimes. Planned integration surfaces:

- Tool Gateway shell/tool proxy — first implementation target and now exposed as `tfy tool-gateway`.
- Context Gateway adapter — routes repo/file requests through index/expand/full/decide primitives.
- Output Gateway adapter — restores and validates structured compact patches/code before apply.
- State Gateway ledger — event-fed compact task state from all gateways.

Provider, Codex, MCP, shell, and editor integrations remain adapters around the Rust core.

## Implemented runtime-interception foundation

The production stack now includes `tfy-runtime`, a Rust runtime contract crate that defines versioned envelopes, adapter capabilities, negotiation, gateway events, provenance refs, validation status, and state projection primitives.

Implemented binaries/surfaces:

- `tfy runtime-capabilities`
- `tfy runtime-negotiate`
- `tfy tool-gateway --json|--jsonl`
- `tfy shell --json|--jsonl`
- `tfy context-gateway`
- `tfy output-gateway` preview/validate
- `tfy state-append`
- `tfy state-project`

Release boundary: local shell/tool/context/output/state gateway foundations are implemented. Codex, MCP, editor, and provider adapters are still separate integration packages to build and test before claiming automatic interception for those runtimes.
