# TFY Production Stack

## Purpose

This document maps the final token-saving architecture to the current Rust-first authority-path implementation and the product-grade technology selection policy for future surfaces.

## Technology selection policy

TFY is not Rust-only as a product dogma. TFY is **Rust-first on the authority path**: the components that own correctness, security, deterministic command behavior, raw evidence, redaction, proof validation, and workspace apply should remain Rust unless an explicit stack decision proves another technology is better for released-product quality.

Non-core integrations may use host-native or other best-fit technologies when that makes the released product better:

- editor integrations may use the editor's native extension stack;
- web or dashboard surfaces may use web-native technology;
- host adapters may use the host ecosystem when it improves reliability, distribution, or user experience;
- plugin/sandbox boundaries may use portable component technology when it improves isolation or extensibility.

Development speed or prototype convenience is not sufficient justification for a product/runtime stack. Any non-Rust integration must preserve TFY's protocol, provenance, fallback, and truthful-claim contracts. Non-Rust integrations must not become correctness dependencies for authority-path behavior without an explicit stack decision record.

## Current authority-path stack

The current authority-path implementation is:

- Core: Rust (`crates/tfy-core`)
- CLI: Rust binary (`crates/tfy-cli`, `tfy`)
- Tool Gateway: Rust CLI entrypoint (`tfy tool-gateway -- <command>`) over the existing command-output core
- Parsers: Tree-sitter grammars where available
- Tool feedback policy: Rust policy objects under the core crate
- Evaluation and benchmarks: Rust tests plus `criterion`
- Optional provider/model adapters: Rust adapter crates, host-native adapters, or external integrations, never authority-path correctness dependencies without an explicit stack decision record

Python is **not** part of the production runtime authority path. The former Python product surfaces (`src/tfy`, root `pyproject.toml`, Python tests, `bindings/tfy-python`, `pyo3`, and `uv.lock`) have been removed from the release path. Python remains a supported source-code language for analysis fixtures through tree-sitter and may be used for non-runtime evaluation, corpus, or research tooling only when it is not a correctness dependency.

## Required architecture primitives

The production core should support:

- token-saving method registry
- representation ladder and adaptive retrieval policy
- artifact/ref store
- parser-backed semantic skeletons
- compact code and restoration contracts
- byte-preserving raw command-output store and range expansion
- incremental/delta state model
- compact schema dictionaries
- task/conversation ledger compaction
- provider adapter interface
- evaluation gate reporting

## Production invariants

- CLI-first text protocol remains canonical for model-visible output; JSON/envelopes remain explicit debug/adapter/internal surfaces.
- Core behavior remains agent-neutral.
- Optional adapters cannot be required for authority-path correctness unless an explicit stack decision record changes that contract.
- Parser-backed responses include confidence, parser identity, and fallback action.
- Raw/full fallback is always available for high-risk artifacts.
- Public model-visible output redacts secrets before pass-through or summary; original command/output bytes remain local behind raw refs.
- Current implementation status must be reported honestly.

## Current Rust smoke commands

```sh
cargo run -p tfy-cli -- languages
cargo run -p tfy-cli -- index corpus/rust/fixture_01.rs
cargo run -p tfy-cli -- expand corpus/rust/fixture_01.rs fixture_01.rs:calculate_discount_1:5 --compactness symbol
cargo run -p tfy-cli -- restore --payload payload.json
cargo run -p tfy-cli -- tool-gateway -- sh -c 'printf ok'
cargo run -p tfy-cli -- tool-gateway -- sh -c 'for i in $(seq 1 200); do echo "line $i"; done'
cargo run -p tfy-cli -- raw <raw_ref>
cargo run -p tfy-cli -- eval-code corpus/rust/fixture_01.rs fixture_01.rs:calculate_discount_1:5
cargo test --quiet
```

## Retired Python product artifacts

The current Rust authority-path implementation removed the former Python product/runtime surfaces from the release path:

- `src/tfy/*`
- `tests/*.py`
- root `pyproject.toml`
- `uv.lock`
- `bindings/tfy-python`
- `pyo3` workspace dependency

Do not reintroduce Python as a product/runtime authority-path dependency without a new explicit stack decision record.

## Release readiness

A production surface is release-ready only when `EVALUATION_GATES.md` passes for its method families. Token savings without correctness/evidence gates are not sufficient.


## Agent middleware stack direction

TFY's production integration model is AI-agent I/O middleware. The Rust core owns current authority-path correctness; adapters automate invocation for specific runtimes and may use host-native stacks when that improves released-product quality. Planned integration surfaces:

- Tool Gateway shell/tool proxy — first implementation target and now exposed as `tfy tool-gateway`.
- Context Gateway adapter — routes repo/file requests through index/expand/full/decide primitives.
- Output Gateway adapter — restores and validates structured compact patches/code before apply.
- State Gateway ledger — event-fed compact task state from all gateways.

Provider, Codex, MCP, shell, and editor integrations remain adapters around the Rust core unless a future stack decision explicitly moves an authority-path responsibility.

## Implemented runtime-interception foundation

The production stack now includes `tfy-runtime`, a Rust runtime contract crate that defines versioned envelopes, adapter capabilities, negotiation, gateway events, provenance refs, validation status, and state projection primitives.

Implemented binaries/surfaces:

- `tfy runtime-capabilities`
- `tfy runtime-negotiate`
- `tfy tool-gateway -- <command>` and `tfy shell -- <command>` as text-first model-visible wrappers; `tfy shell <command>` is raw passthrough only
- `tfy tool-gateway --json|--jsonl` and `tfy shell --json|--jsonl -- <command>` as debug/adapter/internal wrappers
- `tfy context-gateway`
- `tfy output-gateway` preview/validate
- `tfy state-append`
- `tfy state-project`

Release boundary: local shell/tool/context/output/state gateway foundations, generic-shell adapter, and MCP stdio tool/resource server are implemented. Codex private hooks, editor, and provider adapters are still separate integration packages to build and test before claiming automatic interception for those runtimes. The Codex-facing MCP support is a setup snippet for Codex MCP configuration, not private Codex hook interception.

## MCP/Codex adapter foundation v2

The production stack now includes `tfy mcp serve`, a stdio MCP server that exposes existing TFY gateway capabilities as tools/resources. It is intentionally bounded:

- stdout is JSON-RPC only;
- logs and warnings use stderr/files;
- `initialize` declares tools and resources;
- `resources/list` returns concrete session resources;
- `resources/templates/list` returns `tfy://raw/{raw_ref}`, `tfy://report/{session}`, and `tfy://state/{session}` templates;
- child command failures are tool results and do not terminate the MCP process;
- `tfy mcp install --target codex --dry-run` prints a concrete `codex mcp add` command and TOML snippet without writing config.

This is the first supported agent-native integration boundary after the generic-shell adapter. It does not replace future Codex private hook/provider/editor adapters.

## Human managed-session auto-intercept (Linux bash, macOS zsh, Windows PowerShell)

`tfy start --human` creates/refreshes `.tfy/human/auto-activate.json` plus deterministic shell-specific activation content (`auto-activate.bash`, `auto-activate.zsh`, or `auto-activate.ps1`) as persistent current-directory local state and enters a TFY-managed current-directory-scoped session on supported hosts when invoked as the only project target from an interactive terminal. If the selected startup-file hook is missing, that interactive path offers a default-No prompt to install the one-time future-shell hook; accepting it reuses the same safe writer as `tfy setup --human --apply`. Linux bash and macOS zsh use generated TFY-owned startup scripts plus `.tfy/human/bin` PATH shims; Windows PowerShell uses TFY-owned proxy functions and rejects non-application shadowing with `Get-Command -All`. Non-interactive plain `tfy start --human` remains lifecycle intent-only; `tfy start --human --auto-activate` is the explicit automation path for marker creation without entering a shell. npm install and non-interactive start never mutate shell startup files. The user can still install one explicit rc/profile hook later with `tfy setup --human --apply` or the lower-level `tfy human auto-activate install --shell <bash|zsh|powershell> --rcfile <path> --apply`. The hook is scoped to supported shell sessions that read the selected startup file, invokes only a pinned absolute TFY executable, checks only `$PWD/.tfy/human/auto-activate.json`, validates/regenerates deterministic activation content before sourcing, and exports `TFY_HUMAN_TFY_BIN` so generated shims/proxies never execute PATH-resolved `tfy`. Safely resolved ordinary external commands store raw output before summary selection; shell-local functions, aliases, builtins, direct paths, explicit bypass, TFY gateway, outside-current-directory-scope commands, and nested child-shell internals remain raw/not-claimed unless separately routed; newly created PATH executables may run raw until the next prompt-time/proxy refresh; automatic interactive/TUI/stateful subcommand classification is not claimed in v1. This is not global terminal interception and does not mutate an already-running parent shell.
