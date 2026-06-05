# TFY — Whole-Workflow Token-Saving Service for AI Coding

TFY is a token-saving service for the full AI coding workflow. It reduces tokens across code context, project context, tool feedback, patches, conversation state, repeated provider prompts, and audit/fallback flows while preserving correctness and recoverability.

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
- **Stay agent-neutral:** the core protocol is CLI/JSON/ref based. Provider/model-specific caching is optional adapter behavior, never a core dependency.
- **Expect future methods:** token-saving methods are registry entries with metrics, risks, fallback triggers, and evaluation gates.

## Final architecture

TFY is organized around six release concepts:

1. **Representation ladder** — raw/full, summary, semantic skeleton, selected compact detail, hash/ref, delta, and optional provider-adapter view.
2. **Token-saving method registry** — each method declares target artifact, savings mechanism, performance cost, correctness risk, fallback trigger, and evaluation metric.
3. **Artifact/ref store** — files, scopes, command outputs, summaries, task ledgers, and provider layouts can be referenced by stable IDs instead of repeated in full.
4. **Adaptive retrieval and compactness policy** — choose skeleton, summary, compact body, related context, full file, or raw output based on task risk and budget.
5. **Agent-neutral protocol** — any agent can use `tfy index`, `expand`, `run`, `raw`, `restore`, and future registry/ref/delta commands.
6. **Evaluation gates** — every saving claim is measured as net token savings plus correctness, fallback frequency, missed-evidence risk, and performance overhead.

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

## Implementation status

The product/runtime stack is **Rust core + Rust CLI**. Python product/runtime surfaces have been retired: there is no root Python package, PyO3 binding crate, Python lockfile, or Python test suite in the release path. Python remains only a supported input language for code analysis through tree-sitter fixtures.

Rust smoke commands:

```sh
cd tfy
cargo run -p tfy-cli -- languages
cargo run -p tfy-cli -- index corpus/rust/fixture_01.rs
cargo run -p tfy-cli -- run -- sh -c 'printf ok'
cargo test --quiet
```
