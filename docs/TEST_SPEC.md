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
- Repeated unchanged output can be represented by status/hash/ref after storing fresh raw evidence and only when the repeat notice is smaller than raw output.
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
- Docs must preserve the Rust-first authority-path invariant while allowing best-fit non-core integrations that are not correctness dependencies without an explicit stack decision record.

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
- `tfy shell --json -- <command>` shell-adapter wrapper output, plus `tfy shell <command>` raw passthrough with no TFY artifacts.
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
- `tfy smoke --mcp --json` runs the local MCP Code I/O workflow end to end, records Tool/Context/Output/State evidence, and proves preview validation does not mutate while proof-gated apply does.
- `tfy smoke --all --json` runs generic-shell, agent-wrapper, and MCP smokes and emits ledger paths that can be supplied to `tfy launch-report --all --ledger ...`; without separate artifact-backed host setup/real-invocation and overhead evidence these routes are only `verified_local_mcp`.
- `tfy smoke --codex` is checklist/report-only and states that no Codex host invocation is claimed.
- `tfy gain` reports `No TFY savings data found yet` on empty ledgers and reports real bytes/tokens from adapter or MCP `tfy_tool_run` `ToolCommandCompleted` command events when present, including `repeat_elided` counts for unchanged repeated output.
- `tfy setup --ai --host <host> --dry-run` and `tfy mcp install --target <host> --dry-run` emit truthful MCP setup snippets for Codex, Claude Code, Cursor, OpenCode, and Hermes, while OpenClaw remains `planned_discovery`.
- `tfy setup --ai --host cursor --apply --project` safely writes project `.cursor/mcp.json`, creates a TFY backup when modifying an existing file, preserves unrelated JSON keys and MCP servers, is idempotent, supports TFY-only uninstall, dry-runs without writing, and fails closed on malformed existing JSON.
- `tfy status --json` exposes both canonical status and claim-tier taxonomy for required routes and named hosts.
- `tfy launch-report --json` exposes the `mcp_stdio`, `tfy_agent_adapter`, `generic_shell`, and named-host readiness matrix, blocks release while required v1 hosts (`mcp_stdio`, `tfy_agent_adapter`, `generic_shell`) lack `launch_supported` evidence, promotes local smoke only to `verified_local_mcp`, requires artifact-backed `--host-evidence` JSON plus overhead evidence before `launch_supported`, keeps named hosts non-blocking below launch support without host-bound route/config/ledger/raw/no-negative/positive-savings evidence, lists unsupported provider/editor/private-hook/universal-terminal paths, reports exact byte savings plus conservative token proxy estimates, and includes the seven required launch benchmark scenario names.

## Human `start --human` auto-intercept regression

- On supported Linux bash, interactive project-only `tfy start --human` creates/refreshes trusted repo future-shell auto-activation state and enters a TFY-managed project-scoped PATH-shim shell without requiring a second `tfy human shell` command; non-interactive plain invocations record lifecycle intent, do not create auto-activation state, and do not hang.
- `tfy human install --dry-run|--output <path>` emits or writes a TFY-owned bash script with ownership markers; uninstall refuses to remove non-TFY scripts.
- PATH-resolved ordinary external commands known to the generated `.tfy/human/bin` shim are routed once through TFY run, store combined raw command output first, and summarize only when smaller than redacted raw output.
- The shim refresh refuses non-TFY-owned pre-existing shim entries, rewrites marker-bearing shims from the deterministic template, records prompt status without clobbering refresh status, and treats newly created PATH executables as raw/not-claimed until prompt-time refresh discovers them.
- Direct paths, explicit `tfy-human-bypass`, TFY gateway, shell builtins/keywords, aliases/functions, outside-scope commands, and nested child-shell internals run raw and must not claim wrapped-command raw output or no-negative-savings; automatic stateful/TUI subcommand classification is not claimed in v1.
- Ordinary terminals outside the managed session remain `ordinary_terminal_interception=false`.
- Interactive project-only `tfy start --human` and explicit automation `tfy start --human --auto-activate` create/refresh `.tfy/human/auto-activate.json` and deterministic `.tfy/human/auto-activate.bash`; non-interactive plain `tfy start --human` and plain `tfy start both` do not create future-shell auto-activation state.
- `tfy human auto-activate install --shell bash --rcfile <path> --apply` writes only a TFY-owned marker-bounded rc hook; uninstall removes only that block; status reports normalized `repo_marker`, `selected_rcfile`, `active_shell`, and `support_status` groups.
- Startup validation and generated shims use the pinned absolute TFY executable, never PATH-resolved `tfy`; fake repo-local/PATH-earlier `tfy` binaries must not run during startup validation or routed command execution.
- Auto-activation rejects symlinked `.tfy`, malformed/disabled/wrong-root markers, unsafe permissions, and self-certifying script markers/hashes; validation byte-compares deterministic expected content or regenerates it before sourcing.
