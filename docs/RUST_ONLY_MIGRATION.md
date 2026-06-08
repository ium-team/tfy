# TFY Python Runtime Retirement and Rust Authority-Path Record

## Decision

TFY's current runtime authority path is **Rust core + Rust CLI**. Python is not part of the production runtime authority path.

The former Python package and PyO3 binding were retired from the product/runtime path after Rust parity coverage was added for the first releasable version. Python remains a source language TFY can analyze, and may be used for non-runtime evaluation, corpus, or research tooling when it is not a product/runtime dependency.

## Why Rust

TFY's core work is correctness-sensitive systems work:

- deterministic compression and restoration
- raw/ref storage and range recovery
- repository-scale parsing and context selection
- Git/GitHub, CI, and command-output evidence policy
- security redaction at every public summary boundary
- benchmarked net-savings and performance evaluation

Rust is the best fit for the current authority path because it provides predictable performance, strong data contracts, safe single-binary distribution, and direct integration with tree-sitter based language analysis.

This record is not a product-wide ban on non-Rust technologies. Non-core integrations may use host-native or other best-fit stacks when that improves released-product quality. Such integrations must not become correctness dependencies for authority-path behavior without an explicit stack decision record.

## Target stack

| Layer | Target owner | Notes |
|---|---|---|
| Core algorithms | `crates/tfy-core` | Method registry, representation ladder, compact/restore, raw refs, evaluation. |
| CLI | `crates/tfy-cli` | Canonical user and agent interface. |
| Language analysis | `crates/tfy-core::language` | Tree-sitter backed scope extraction and semantic skeletons. |
| Tool feedback | `crates/tfy-core::tool_feedback` | Command capture, raw refs, generic policy registry, Git/GitHub policy. |
| Evaluation | `crates/tfy-core::eval` | Net savings, correctness, fallback, performance gates. |
| Optional adapters | Rust, host-native, or other best-fit adapter implementations | Provider/model/cache/editor adapters remain optional and cannot affect authority-path correctness without an explicit stack decision record. |

## Python retirement record

| Former surface | Former role | Final status | Evidence |
|---|---|---|---|
| `pyproject.toml` root package | Python package metadata and `tfy` script | **Removed** | Rust CLI is the documented command surface. |
| `uv.lock` | Python package lockfile | **Removed** | No Python package install path remains. |
| `src/tfy/cli.py` | Python CLI prototype | **Removed** | Equivalent Rust module and Rust tests cover the release contract. |
| `src/tfy/code.py` | Code compaction prototype | **Removed** | Equivalent Rust module and Rust tests cover the release contract. |
| `src/tfy/context.py` | Context selection prototype | **Removed** | Equivalent Rust module and Rust tests cover the release contract. |
| `src/tfy/eval.py` | Evaluation prototype | **Removed** | Equivalent Rust module and Rust tests cover the release contract. |
| `src/tfy/languages.py` | Language metadata prototype | **Removed** | Equivalent Rust module and Rust tests cover the release contract. |
| `src/tfy/tool_feedback.py` | Tool feedback and Git/GitHub policy prototype | **Removed** | Equivalent Rust module and Rust tests cover the release contract. |
| `tests/*.py` | Legacy behavior and regression suite | **Removed** | Rust unit/integration tests are the release gate. |
| `bindings/tfy-python` | PyO3 binding scaffold | **Removed** | Workspace members are Rust core + Rust CLI only. |
| `pyo3` workspace dependency | Python binding support | **Removed** | Cargo workspace no longer depends on PyO3. |
| Python examples in docs | Prototype smoke commands | **Replaced** | Public smoke commands use Rust CLI only. |

## Completed migration sequence

- CLI command names and JSON/text output contracts are documented around `cargo run -p tfy-cli -- ...`.
- Rust raw refs, command-output store, tool feedback compression, Git/GitHub policy, code compaction/restore, context selection, language registry, and eval gates are implemented under `crates/tfy-core`.
- Critical legacy regression cases were moved into Rust tests, including Git/GitHub false-success prevention, status/diff classification, raw fallback, credential URL redaction, and malformed URL handling.
- Rust CLI is the only documented run path.
- Former Python product artifacts and PyO3 binding surfaces were removed from the workspace.

## Non-negotiable gates

The current Rust authority-path release remains acceptable only while these gates pass:

1. **Parity:** Rust output matches accepted fixtures or documents a deliberate contract improvement.
2. **Correctness:** deterministic restore or full/raw fallback works for every compact representation.
3. **Evidence:** errors, dirty repo state, failed checks, review blockers, and security/auth failures remain visible.
4. **Security:** credential-bearing URLs and command strings are redacted from public summaries while originals stay local behind raw refs.
5. **Performance:** Rust benchmark results are recorded for parse/index/compress/eval paths.
6. **Packaging:** documented user workflows run from the Rust CLI without Python product/runtime authority-path dependencies.

## Documentation rule

Public docs must use Rust commands for TFY authority-path execution. Python may appear as an analyzed source-code language or non-runtime evaluation/research tooling, never as a TFY product/runtime authority-path dependency without a new explicit stack decision record.
