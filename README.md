# TFY — Whole-Workflow Token-Saving Service for AI Coding

TFY is a token-saving service for the full AI coding workflow. Its intended product use is as AI-agent I/O middleware: an agent runtime routes command execution, context injection, model outputs, and task state through TFY so the model sees compact, recoverable representations while raw/full evidence remains available.

TFY is not a phased MVP, not a code minifier, and not an RTK clone. The first public shape is the final product architecture: an extensible method registry plus an agent-neutral protocol that can absorb new token-saving methods as they are discovered.

## Product contract

TFY gives agents the smallest representation that can still support correct work, and it keeps a deterministic path back to meaning:

```text
project artifacts / commands / task state
-> TFY representation ladder
-> compact agent-facing context
-> agent actions / compact patches
-> TFY restoration, expansion, and evidence recovery
-> human/project-ready output
```

Core invariants:

- **Save tokens everywhere:** code, docs, command output, CI, Git/GitHub work, task state, patches, schemas, repeated context, and provider adapters.
- **Preserve evidence:** errors, failing tests, security/permission issues, dirty repo state, review comments, and other decision-critical facts keep raw or directly recoverable evidence.
- **Prefer correctness over savings:** compact context is allowed only while work quality holds; uncertainty triggers related/full/raw fallback.
- **Stay agent-neutral:** the model-facing protocol is plain text plus local refs; JSON/envelopes are explicit debug/adapter/internal surfaces, never the default model payload.
- **Expect future methods:** token-saving methods are registry entries with metrics, risks, fallback triggers, and evaluation gates.

## Final architecture

TFY is organized around seven release concepts:

1. **Representation ladder** — raw/full, summary, semantic skeleton, selected compact detail, hash/ref, delta, and optional provider-adapter view.
2. **Token-saving method registry** — each method declares target artifact, savings mechanism, performance cost, correctness risk, fallback trigger, and evaluation metric.
3. **Artifact/ref store** — files, scopes, command outputs, summaries, task ledgers, and provider layouts can be referenced by stable IDs instead of repeated in full.
4. **Adaptive retrieval and compactness policy** — choose skeleton, summary, compact body, related context, full file, or raw output based on task risk and budget.
5. **Agent I/O middleware boundaries** — Tool, Context, Output, and State Gateways define where TFY sits between an agent runtime, tools, model context, model output, and long-running task state.
6. **Agent-neutral protocol** — default CLI gateway output is model-visible text selected by a net-savings gate; JSON primitives remain explicit debug/adapter/internal contracts for `tfy tool-gateway`, `index`, `expand`, `run`, `raw`, `restore`, and future registry/ref/delta commands.
7. **Evaluation gates** — every saving claim is measured as net token savings plus correctness, fallback frequency, missed-evidence risk, and performance overhead.

## Current method families

TFY's initial registry includes:

- semantic code indexing and skeletons
- compact code and deterministic symbol maps
- content-addressed context refs
- incremental delta protocol
- token-aware schema dictionaries
- dependency-neighborhood slicing
- patch-only / edit-script outputs
- boilerplate and generated-artifact suppression
- conversation/task-state compaction
- risk-aware tool feedback compression
- tool-output fingerprinting
- error clustering and diagnostic normalization
- retrieval budget planning
- adaptive compactness policy
- local memoization
- Test/CI selective evidence mode
- privacy/security redaction with local refs
- optional provider/model cache adapters

The list is intentionally open. New methods should be added through the method registry, not by rewriting TFY's identity.

## Safety model

Full context fallback is not a failure. It is the mechanism that keeps TFY honest.

TFY must expand or recover raw evidence when:

- compact symbols are unmapped or ambiguous
- diagnostics mention unresolved names or missing code
- a command emits errors, warnings, stack traces, failed checks, or review blockers
- a summary confidence is low
- a patch touches unsafe/public/dynamic behavior
- provider cache/adapters cannot guarantee equivalent model behavior

## Documentation map

Canonical docs:

- `docs/TOKEN_SAVING_ARCHITECTURE.md` — final architecture, representation ladder, and method registry schema.
- `docs/PRD.md` — product requirements for the release-ready architecture.
- `docs/PROTOCOL.md` — agent-neutral protocol contract.
- `docs/AGENT_MIDDLEWARE.md` — Tool/Context/Output/State Gateway integration model for AI-agent runtimes.
- `docs/EVALUATION_GATES.md` — net savings, correctness, fallback, and performance gates.
- `docs/ADAPTERS.md` — optional provider/model adapter policy.

Focused method-family docs:

- `docs/CODE_COMPRESSION.md` — code/context representations, maps, restoration, and skeletons.
- `docs/TOOL_FEEDBACK.md` — command/tool feedback, raw refs, fingerprinting, and error clustering.
- `docs/GIT_GITHUB_HARNESS.md` — Git/GitHub evidence specialization.

Reference/status docs:

- `docs/PRODUCTION_STACK.md` — production stack expectations and current implementation status.
- `docs/RUST_ONLY_MIGRATION.md` — Rust-only runtime target and completed Python retirement record.
- `docs/RTK_REFERENCE.md` — RTK lessons without inheriting RTK's product boundary.
- `docs/CONVERSATION_SUMMARY.md` — superseded discussion history and final decision summary.

Repository/development harness:

- `AGENTS.md` — repo-root Codex/AI-agent instructions, branch policy, commit policy, and verification rules.
- `docs/AGENT_HARNESS.md` — long-form AI-agent project map, invariants, safe edit rules, and focused checks.
- `docs/contributing/CONTRIBUTING.md` — contributor workflow expectations.
- `docs/contributing/GIT_POLICY.md` — Git Flow branch model, Conventional+Lore commit format, and PR policy.
- `docs/contributing/REPOSITORY_HARNESS.md` — module boundaries, adapter workflow, and release-readiness harness.
- `.github/` — PR template, issue templates, and Rust CI workflow.
- `scripts/verify.sh` — local full verification gate.

## Implementation status

The product/runtime stack is **Rust core + Rust CLI**. Python product/runtime surfaces have been retired: there is no root Python package, PyO3 binding crate, Python lockfile, or Python test suite in the release path. Python remains only a supported input language for code analysis through tree-sitter fixtures.

Rust smoke commands:

```sh
cd tfy
cargo run -p tfy-cli -- languages
cargo run -p tfy-cli -- index corpus/rust/fixture_01.rs
cargo run -p tfy-cli -- tool-gateway -- sh -c 'printf ok'
cargo run -p tfy-cli -- tool-gateway -- sh -c 'for i in $(seq 1 200); do echo "line $i"; done'
cargo run -p tfy-cli -- shell -- sh -c 'printf ok'
cargo run -p tfy-cli -- tool-gateway --json -- sh -c 'printf ok' # debug/adapter/internal only
cargo run -p tfy-cli -- runtime-capabilities
cargo run -p tfy-cli -- adapter capabilities
cargo run -p tfy-cli -- adapter install --target generic-shell --dry-run
cargo run -p tfy-cli -- adapter run --session smoke -- sh -c 'printf ok'
cargo run -p tfy-cli -- adapter report --session smoke
cargo run -p tfy-cli -- mcp capabilities
cargo run -p tfy-cli -- mcp install --target codex --dry-run
cargo test --quiet
```


## How TFY participates in an AI-agent loop

Humans can run the CLI directly, but the intended path is automatic runtime use:

```text
AI agent/runtime
  -> Tool Gateway: ordinary command -> tfy tool-gateway -- <command> -> model-visible text
  -> Context Gateway: repo/file request -> index/expand/full/decide -> compact context + fallback refs
  -> Output Gateway: compact patch/code -> restore/validate -> apply-ready output or fallback request
  -> State Gateway: turn history/tool evidence -> compact task ledger + refs
```

Tool Gateway always stores exact raw stdout/stderr bytes locally first. The model sees a compact summary only when that summary is strictly smaller than the redacted public raw output; otherwise TFY passes through the redacted raw text. This prevents negative token savings for tiny outputs such as `ok`. Raw refs remain available internally/debug-side and are included in model text when output is summarized, truncated, or suppressed. The P0 command-output path adds command-family summaries for high-frequency development commands (`git status/diff/log`, `gh pr checks`, Cargo build/test/check/clippy/fmt-check, TypeScript no-emit checks, and common test runners) plus family-level adapter analytics, without claiming private Codex hooks or universal shell interception.

Current implementation status:

- Implemented: Rust core primitives, Rust CLI, `tfy-runtime` envelope/capability/event contract, Tool Gateway text-first net-savings entrypoint, explicit debug/adapter JSON/JSONL entrypoints, shell wrapper, Context Gateway CLI, Output Gateway preview/validate CLI plus content-addressed single-file selected-scope `--apply`, State Gateway append/project CLI, raw refs, redaction, code index/expand/full/restore, evaluation.
- Implemented adapter v1: `tfy adapter` generic-shell command-boundary shim, dry-run installer, session ledger, command-family-aware savings report, and P0 Tool Gateway summaries for Git, `gh pr checks`, Cargo, TypeScript no-emit, and common test-runner output.
- Implemented MCP foundation v2: `tfy mcp serve` stdio JSON-RPC server, MCP tool/resource discovery, raw/report/state resources, and Codex MCP dry-run/setup snippet generation.
- Planned adapters: Codex private hooks/editor/provider automatic hook integrations and broader Output Gateway apply surfaces such as multi-file/fuzzy patch engines or deletion semantics.

TFY should not claim automatic model input/output interception for a runtime until that runtime adapter exists and passes the relevant gates. The MCP foundation is a supported MCP tool/resource integration point; it still requires the host agent to route through MCP and is not a private Codex hook or universal shell interception layer.

## MCP/Codex adapter foundation

TFY can run as an MCP stdio server for agent hosts that support MCP:

```bash
tfy mcp capabilities
tfy mcp serve --session local-session --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw
tfy mcp install --target codex --dry-run
```

The server exposes `tfy_tool_run`, `tfy_raw_get`, `tfy_context_get`, `tfy_output_validate`, `tfy_state_project`, and `tfy_adapter_report`, plus `tfy://raw/{raw_ref}`, `tfy://report/{session}`, and `tfy://state/{session}` resources. MCP stdout is JSON-RPC only; logs and warnings go to stderr or files. Non-zero child commands are returned as tool results and do not terminate the MCP server.

This is MCP tool/resource integration. It does not claim private Codex hook interception, provider prompt mutation, or universal shell interception without host MCP routing.
