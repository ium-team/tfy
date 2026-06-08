# TFY Test and Evaluation Specification

## Purpose

TFY tests validate the final whole-workflow token-saving architecture. Tests must prove not only that output is shorter, but that correctness, evidence, restoration, and fallback behavior survive compression.

## Universal method tests

Every registry method should have tests or evaluation evidence for:

- gross token savings
- net token savings after refs/maps/expansions/raw requests
- fallback behavior
- missed-needed-context failures
- missed-evidence failures
- local performance overhead
- restoration/audit recovery
- method metadata completeness

## Registry metadata tests

Each method entry must declare:

- target artifact type
- mechanism
- prerequisites
- core vs optional-adapter status
- savings metric
- performance cost
- correctness/evidence risk
- fallback trigger
- validation gate

## Code/context tests

- Build semantic index and skeletons from supported source files.
- Generate compact code with safe symbol maps.
- Restore no-op compact roundtrip to readable source.
- Reject ambiguous bare scopes and unmapped compact symbols.
- Preserve Python layout/string literals where required.
- Expand selected -> related -> full context when diagnostics require it.
- Measure compactness against full-context baseline.

## Tool feedback tests

- Failed commands preserve actionable evidence.
- Successful/no-action commands compress or fingerprint aggressively.
- Raw output is stored and requestable by raw ref.
- Around expansion recovers nearby evidence.
- Unknown output uses conservative summaries.
- Repeated unchanged output can be represented by status/hash/ref.
- Error clustering preserves representative evidence and raw recovery.

## Git/GitHub harness tests

`GIT_GITHUB_HARNESS.md` is the canonical specialized contract. Required fixture/replay classes:

- clean status
- dirty status
- diff/name-status/hunk output
- conflict output
- push rejection/auth failure
- CI/check failure
- PR requested changes/review comments
- API/rate-limit/403/404 error

Tests must not require live GitHub credentials, network access, or mutable remote writes unless live integration is explicitly in scope.

## Provider adapter tests

Provider adapters are optional. Adapter tests must prove:

- neutral TFY behavior works without the adapter
- stable/volatile context bands are laid out correctly
- provider usage metadata is captured when available
- cache hit/miss or cached-token benefit is reported honestly
- adapter miss/failure degrades to neutral protocol
- no provider-specific feature is required for correctness

## Security/redaction tests

- Secrets, long tokens, and sensitive blobs can be replaced with local refs.
- Redacted values can be recovered locally only when authorized.
- Redaction does not corrupt evidence or restoration.
- False positives/negatives are classified.

## Release gates

A release-ready method passes only when:

- net token savings is positive for at least one target workflow class
- compact workflow quality matches full-context baseline within tolerance
- raw/full fallback works for high-risk cases
- every missed-needed-context or missed-evidence failure is classified
- local overhead is acceptable
- documentation states implementation status truthfully

## Documentation consistency tests

- Docs describe the final architecture directly, not 1st/2nd/3rd phases.
- README and all public docs under `docs/*.md` align with the method registry.
- Provider/model cache behavior is optional adapter behavior.
- No public doc frames TFY as only a code minifier, only an RTK-style command-output filter, or a phased MVP.
- Specialized docs link back to canonical architecture.


## Agent middleware tests

Documentation gates:

- Docs must not imply automatic model input/output interception is implemented before a runtime adapter exists.
- Docs must include a gateway-to-registry crosswalk.
- Docs must preserve the Rust-only runtime invariant.

Tool Gateway gates:

- `tfy tool-gateway -- sh -c 'printf ok'` emits plain `ok` with no JSON/raw_ref overhead.
- Long/noisy or suppressed output emits shorter model-visible text with a recoverable raw_ref.
- `tfy raw --around <needle> --context <n>` returns only the requested text range for valid UTF-8 output and fails closed for invalid UTF-8 ranged requests.
- Non-zero exits preserve exit status and critical evidence.
- Credential-bearing URLs are redacted publicly and preserved only behind raw_ref.
- Unicode and tiny summary caps do not panic and preserve raw_ref.

Future Context/Output/State Gateway gates are defined in the ralplan handoff and should become executable tests when those adapters are implemented.

## Runtime-interception foundation tests

Implemented test coverage now includes:

- `tfy-runtime` envelope round-trip and required field validation.
- Capability negotiation acceptance and neutral-degrade/fail-closed cases.
- State projection non-authoritative fallback when lineage/validation is missing.
- `tfy tool-gateway --jsonl` structured event + response output.
- `tfy shell --json` shell-adapter wrapper output.
- `tfy state-project` compact ledger projection.
- `tfy context-gateway` runtime envelope for compact context.
- `tfy output-gateway` preview/validate envelope for structured compact code plus proof-gated single-file selected-scope apply success/fail-closed cases.
- `tfy agent` configured-AI-wrapper origin/provenance behavior, proving agent-origin interception without mutating ordinary human shell startup files.
- `tfy restore-display` display-only readable restoration that restores original symbols and carries no apply authority.
- `tfy workspace validate/apply` exact multi-file WorkspaceApplyPlan success/fail-closed cases for base hashes, plan hashes, per-operation proof, and explicit delete semantics.

Remaining adapter tests before stronger claims:

- Codex hook interception e2e.
- MCP stdio server e2e: implemented in `crates/tfy-cli/tests/mcp_server.rs` for tools/resources; broader host-specific proxy routing remains follow-up.
- Editor file/context adapter e2e.
- Provider cache/layout hit/miss/accounting e2e.
- Private Codex hook/provider/editor interception.
- Fuzzy workspace mutation beyond fail-closed/preview-only behavior.


## Adapter v1 verification

Adapter v1 is verified by `crates/tfy-cli/tests/adapter_gateway.rs` and these smoke commands:

```sh
cargo run -p tfy-cli -- adapter capabilities
cargo run -p tfy-cli -- adapter install --target generic-shell --dry-run
cargo run -p tfy-cli -- adapter run --session smoke -- sh -c 'printf ok'
cargo run -p tfy-cli -- adapter run --session smoke -- sh -c 'for i in $(seq 1 200); do echo "line $i"; done'
cargo run -p tfy-cli -- adapter report --session smoke
```

Required evidence:

- tiny output remains plain text with no JSON leakage;
- long output summarizes only when shorter and includes raw recovery;
- failures preserve original exit code;
- session report shows raw/model-visible byte size, estimated token savings, rendering counts, and raw refs;
- dry-run install does not write files;
- docs do not claim Codex/MCP/editor/provider automatic interception without matching adapter e2e tests.

Adapter reports use `raw_bytes` and `model_bytes` as the public size contract. Legacy runtime ledger fields such as `raw_chars` / `model_chars` are compatibility-only and are not emitted by `tfy adapter report`.

## MCP/Codex adapter foundation tests

`crates/tfy-cli/tests/mcp_server.rs` verifies:

- `tfy mcp capabilities` reports stdio support and does not claim private hooks/provider gateway/universal interception.
- `tfy mcp install --target codex --dry-run` writes nothing and prints both a concrete `codex mcp add tfy -- tfy mcp serve ...` command and TOML snippet.
- MCP `initialize` declares both `capabilities.tools` and `capabilities.resources`.
- `tools/list` exposes TFY tool names.
- `resources/list` returns concrete session resources, while `resources/templates/list` returns URI templates.
- `tfy_tool_run` preserves failing command exit metadata without terminating the MCP server.
- `resources/read` recovers raw output and reports byte-only adapter metrics.

Additional MCP hardening tests verify:

- JSON-RPC notifications do not emit response objects on stdout.
- `tfy://state/{session}` and `tfy_state_project` are scoped to the requested session and do not leak evidence from other sessions sharing the same ledger file.
- Adapter reports expose the byte contract fields `raw_bytes`, `model_bytes`, `saved_bytes`, and `net_savings_ratio` while still omitting public `raw_chars` / `model_chars` fields.
- MCP tools list includes `tfy_restore_display`, `tfy_workspace_validate`, and `tfy_workspace_apply` for host-routed display and exact workspace apply paths.

## Product UX P0 verification

`crates/tfy-cli/tests/product_ux.rs` verifies the product-facing lifecycle layer:

- `tfy init --codex --dry-run` writes nothing and prints the `mcp_host_routed` / `instruction_guidance` tiers, TFY marker names, and `codex mcp add tfy -- tfy mcp serve ...`.
- `tfy init --codex --project --apply` creates or replaces exactly one marker-bounded `AGENTS.md` block and preserves non-TFY content.
- `tfy init --show` reports marker state; `tfy init --uninstall --codex --project --apply` removes only the TFY-owned block.
- `tfy doctor --codex --json` starts a local MCP child, verifies `initialize` and required tools, checks writable local dirs, and warns rather than overclaims Codex config state.
- `tfy smoke --mcp --json` runs the local MCP Code I/O workflow end to end and proves preview validation does not mutate while proof-gated apply does.
- `tfy smoke --codex` is checklist/report-only and states that no Codex host invocation is claimed.
- `tfy gain` reports `No TFY savings data found yet` on empty ledgers and reports real bytes/tokens from adapter or MCP `tfy_tool_run` `ToolCommandCompleted` command events when present.
