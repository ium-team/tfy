# TFY — Whole-Workflow Token-Saving Service for AI Coding

TFY is a token-saving service for the full AI coding workflow. Its intended product use is as AI-agent I/O middleware: an agent runtime routes command execution, context injection, model outputs, and task state through TFY so the model sees compact, recoverable representations while raw/full evidence remains available.

TFY is not a phased MVP and not a code minifier. The first public shape is the final product architecture: an extensible method registry plus an agent-neutral protocol that can absorb new token-saving methods as they are discovered.

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

- **Save tokens at AI-agent I/O boundaries:** code, docs, command output, CI, Git/GitHub work, task state, patches, schemas, and repeated context. Provider/API gateway proxying is intentionally out of scope for this product line.
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
- `docs/RUST_ONLY_MIGRATION.md` — Python runtime retirement record and current Rust authority-path implementation history.
- `docs/CONVERSATION_SUMMARY.md` — superseded discussion history and final decision summary.

Repository/development harness:

- `AGENTS.md` — repo-root Codex/AI-agent instructions, branch policy, commit policy, and verification rules.
- `docs/AGENT_HARNESS.md` — long-form AI-agent project map, invariants, safe edit rules, and focused checks.
- `docs/contributing/CONTRIBUTING.md` — contributor workflow expectations.
- `docs/contributing/GIT_POLICY.md` — Git Flow branch model, Conventional+Lore commit format, and PR policy.
- `docs/contributing/REPOSITORY_HARNESS.md` — module boundaries, adapter workflow, and release-readiness harness.
- `.github/` — PR template, issue templates, and Rust CI workflow.
- `scripts/verify.sh` — local full verification gate.


## Developer Preview installation

The source build remains the authority path, but the preview distribution is designed for dogfooding from a normal project directory instead of running `cargo run` inside the TFY checkout. The npm package name is `@ium/tfy-cli` because the unscoped `tfy` npm name is already occupied and unscoped `tfy-cli` is blocked by npm similarity policy; the installed command must still be `tfy`.

```sh
npm install -g @ium/tfy-cli@preview
tfy start --agent --host codex
tfy start --human
# interactive start offers this one-time future-shell hook bootstrap for marked repos:
# Install the one-time hook when prompted, or run later:
tfy setup --human --apply
# automation/non-interactive marker creation when not entering the managed shell:
tfy start --human --auto-activate
tfy status --json
```

The npm package is a thin installer/launcher. It downloads the matching GitHub Release archive plus checksum and exposes the `tfy` command. The public names are intentionally different:

| Layer | Name | Why |
| --- | --- | --- |
| Product/repo | `TFY` / `tfy` | Human-facing product and command identity. |
| Rust crate | `tfy-cli` | Cargo package name for the CLI implementation. |
| npm package | `@ium/tfy-cli` | Public npm installer package because unscoped `tfy` is occupied and unscoped `tfy-cli` is blocked by npm similarity policy. |
| Installed executable | `tfy` | The command users run after install. |

TFY uses two public install channels only:

- Stable channel: `npm install -g @ium/tfy-cli` installs the most tested release through the npm `latest` dist-tag, but only after the first stable release exists. Do not point `latest` at public-test builds.
- Public-test channel: `npm install -g @ium/tfy-cli@preview` installs the newest public testing build with current development work included. This is the only recommended npm install form while TFY has preview releases but no stable release.

See [`docs/RELEASE_READINESS.md`](docs/RELEASE_READINESS.md) for the canonical npm dist-tag cleanup and verification checklist.

Exact versions remain installable with standard npm syntax, for example `npm install -g @ium/tfy-cli@0.1.1` or `npm install -g @ium/tfy-cli@0.1.1-preview.0`; npm uses `@<version>`, not `/v<version>`. Public-test releases must use the npm `preview` dist-tag, not `latest`, until they graduate to the stable channel. Local validation of the install path is available with:

Supported prebuilt npm/GitHub Release platforms:

| OS | Architecture | Rust target | npm prebuilt |
| --- | --- | --- | --- |
| macOS | Apple Silicon arm64 | `aarch64-apple-darwin` | yes |
| Linux | x64 | `x86_64-unknown-linux-gnu` | yes |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | yes |
| Windows | x64 | `x86_64-pc-windows-msvc` | yes |

Intel Mac (`darwin:x64` / `x86_64-apple-darwin`) is not provided as a prebuilt npm/GitHub Release archive. Intel Mac users can still build from source with `git clone https://github.com/ium-team/tfy && cd tfy && cargo install --path crates/tfy-cli`, or run a locally built binary by setting `TFY_BINARY_PATH`.


```sh
./scripts/npm-preview-smoke.sh
```

For the plain-language version bump rules, see [`docs/releases/VERSIONING.md`](docs/releases/VERSIONING.md).

GitHub Releases are produced by the manual `.github/workflows/release.yml` workflow. It builds all npm-supported platform archives/checksums, refuses mismatched channel/version/source metadata, creates the GitHub Release when `dry_run` is false, and then publishes npm with the matching channel dist-tag once npm Trusted Publishing is configured for this repository workflow. Use the npm publish plan helper to inspect the exact publish and verification commands:

```sh
node scripts/npm-publish-plan.js --version 0.1.1-preview.0 --channel preview --source-ref develop
node scripts/npm-publish-plan.js --version 0.1.1 --channel stable --source-ref main
```

The npm package defaults its `publishConfig.tag` to `preview` as a safety rail; stable publishes must explicitly use the generated `--tag latest` command. The release workflow uses Trusted Publishing/OIDC rather than a long-lived npm token, and the user-owned npm-side Trusted Publisher setup is documented in `docs/RELEASE_READINESS.md`. The publish helper also prints the required `scripts/npm-dist-tag-check.js` guard; the canonical dist-tag cleanup checklist lives in `docs/RELEASE_READINESS.md`.

For AI-agent use, `tfy start --agent --host codex` writes project MCP configuration, but that only proves configuration. Launch support still requires real host invocation plus TFY raw/ledger/no-negative/positive-savings evidence. For human use on supported platform shells (Linux bash, macOS zsh, Windows PowerShell), interactive project-only `tfy start --human` records lifecycle intent, creates/refreshes the trusted repo-local future-shell marker/script as persistent repo state, and enters a TFY-managed project-scoped shell session where safely resolved command names are wrapped/proxied by TFY and summarized only after raw evidence is stored and only when beneficial. During that interactive start, TFY reports whether the one-time user rc/profile hook is already installed; if it is missing, TFY offers a default-No prompt to install it for future supported shells. Non-interactive plain `tfy start --human` remains lifecycle intent-only; automation that wants marker creation without entering a shell must use `tfy start --human --auto-activate`, and neither path silently edits shell startup files. `tfy setup --human --apply` remains the short manual command for the same explicit hook install; the longer `tfy human auto-activate install --shell <bash|zsh|powershell> --rcfile <path> --apply` remains supported for explicit startup-file control. New supported shell sessions that read that hook auto-activate only inside marked repos; TFY pins the absolute executable and validates deterministic activation content before sourcing. Outside managed/marked sessions, `tfy shell <command>` is raw passthrough convenience and `tfy shell -- <command>` is the TFY summarizing wrapper. TFY does not claim universal or global terminal interception.

## Implementation status

The current authority-path implementation is **Rust core + Rust CLI**. TFY is Rust-first where correctness, security, deterministic command behavior, raw evidence, redaction, proof validation, and workspace apply matter. Non-core integrations may use host-native or other best-fit technologies when that improves released-product quality, provided they do not become correctness dependencies for authority-path behavior without an explicit stack decision record. Python product/runtime surfaces have been retired: there is no root Python package, PyO3 binding crate, Python lockfile, or Python test suite in the release path. Python remains only a supported input language for code analysis through tree-sitter fixtures and may be used for non-runtime evaluation or research tooling only when it is not a product/runtime dependency.

Rust smoke commands:

```sh
cd tfy
cargo run -p tfy-cli -- init --codex --dry-run
cargo run -p tfy-cli -- doctor --codex
cargo run -p tfy-cli -- smoke --mcp
cargo run -p tfy-cli -- gain # no-data until adapter/tfy_tool_run command events exist
cargo run -p tfy-cli -- languages
cargo run -p tfy-cli -- index corpus/rust/fixture_01.rs
cargo run -p tfy-cli -- tool-gateway -- sh -c 'printf ok'
cargo run -p tfy-cli -- tool-gateway -- sh -c 'for i in $(seq 1 200); do echo "line $i"; done'
cargo run -p tfy-cli -- shell printf ok          # raw passthrough, no TFY summary/artifacts
cargo run -p tfy-cli -- shell -- sh -c 'printf ok'       # TFY Shell Gateway wrapper
cargo run -p tfy-cli -- tool-gateway --json -- sh -c 'printf ok' # debug/adapter/internal only
cargo run -p tfy-cli -- runtime-capabilities
cargo run -p tfy-cli -- adapter capabilities
cargo run -p tfy-cli -- adapter install --target generic-shell --dry-run
cargo run -p tfy-cli -- adapter run --session smoke -- sh -c 'printf ok'
cargo run -p tfy-cli -- adapter report --session smoke
cargo run -p tfy-cli -- agent capabilities
cargo run -p tfy-cli -- agent install --dry-run
cargo run -p tfy-cli -- setup --human          # dry-run explicit human rc hook setup
cargo run -p tfy-cli -- setup --human --apply  # one-time opt-in hook for trusted TFY-marked repos
cargo run -p tfy-cli -- setup --ai --codex --dry-run
cargo run -p tfy-cli -- setup --ai --host claude-code --dry-run
cargo run -p tfy-cli -- setup --ai --host cursor --dry-run
cargo run -p tfy-cli -- setup --ai --host codex --apply --project
cargo run -p tfy-cli -- setup --ai --host claude-code --apply --project
cargo run -p tfy-cli -- setup --ai --host cursor --apply --project
cargo run -p tfy-cli -- setup --ai --host opencode --dry-run
cargo run -p tfy-cli -- setup --ai --host hermes --dry-run
cargo run -p tfy-cli -- status --json
cargo run -p tfy-cli -- explain
cargo run -p tfy-cli -- restore-display --payload payload.json --json
cargo run -p tfy-cli -- restore-file --payload payload.json --output restored.js --json
cargo run -p tfy-cli -- workspace validate --payload workspace-plan.json --json
cargo run -p tfy-cli -- workspace refactor-plan --payload workspace-plan.json --chunk-size 5 --json
cargo run -p tfy-cli -- mcp capabilities
cargo run -p tfy-cli -- mcp install --target codex --dry-run
cargo run -p tfy-cli -- mcp install --target cursor --dry-run
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

Tool Gateway always stores exact raw stdout/stderr bytes locally first. The model sees a compact summary only when that summary is strictly smaller than the redacted public raw output; otherwise TFY passes through the redacted raw text. Repeated unchanged command output in the same session is elided only after the new raw output is stored and only when the elision text is smaller than raw output. This prevents negative token savings for tiny outputs such as `ok`. Raw refs remain available internally/debug-side and are included in model text when output is summarized, repeated-elided, truncated, or suppressed. The P0 command-output path adds command-family summaries for high-frequency development commands (`git status/diff/log`, `gh pr checks`, Cargo build/test/check/clippy/fmt-check, TypeScript no-emit checks, and common test runners) plus family-level adapter analytics, without claiming private Codex hooks or universal shell interception.

Current implementation status:

- Implemented: Rust core primitives, Rust CLI, `tfy-runtime` envelope/capability/event contract with explicit origin/provenance fields, Tool Gateway text-first net-savings entrypoint, explicit debug/adapter JSON/JSONL entrypoints, shell wrapper, AI-agent wrapper, Context Gateway CLI, Output Gateway preview/validate CLI plus content-addressed single-file selected-scope `--apply`, display/file restore formatters, WorkspaceApplyPlan validate/apply for proof-gated multi-file modify/add/delete/rename/move and unique-anchor fuzzy operations, refactor chunk planning, State Gateway append/project CLI, raw refs, redaction, code index/expand/full/restore, evaluation.
- Implemented adapter v1: `tfy adapter` generic-shell command-boundary shim, dry-run installer, session ledger, command-family-aware savings report, and P0 Tool Gateway summaries for Git, `gh pr checks`, Cargo, TypeScript no-emit, and common test-runner output.
- Implemented MCP foundation v2: `tfy mcp serve` stdio JSON-RPC server, MCP tool/resource discovery, raw/report/state resources, Codex MCP dry-run/setup snippet generation, and an agent-native Code I/O workflow (`tfy_scope_list`, enriched `tfy_context_get`, preview-only `tfy_output_validate`, proof-gated `tfy_output_apply`, display-only `tfy_restore_display`, and workspace `tfy_workspace_validate`/`tfy_workspace_apply` for validated plans).
- Implemented product UX P1: project/global lifecycle commands (`tfy start`, `tfy stop`, `tfy status --agent/--human`, `tfy fuckyou`, `tfy global start|stop|fuckyou`) plus global default aliases (`tfy use always|stop|cancel|fuckyou`) as configuration-intent state, arrow-key TUI selection for bare interactive lifecycle commands, target aliases (`agent`/`ai`, `human`, `both`), lifecycle `route_state`/`active` fields that stay below launch support without evidence, `tfy start --agent` / `tfy start agent` as safe default project auto-configuration for the Codex project MCP route (`.codex/config.toml`), `tfy start --agent --no-apply` as lifecycle-intent-only escape hatch, `tfy start --agent --host codex` as explicit Codex project `.codex/config.toml` writer, `tfy start --agent --host claude-code` as explicit Claude Code project `.mcp.json` writer, `tfy start --agent --host cursor` as explicit Cursor project `.cursor/mcp.json` writer, and `tfy start agent --host all --apply` as ordered Codex → Claude Code → Cursor safe-writer orchestration, `tfy setup --ai --codex`, named-host setup snippets for Codex, Claude Code, Cursor, OpenCode, and Hermes through `tfy setup --ai --host <host>` / `tfy mcp install --target <host> --dry-run`, safe reversible Codex, Claude Code, and Cursor project config apply/uninstall for `.codex/config.toml`, `.mcp.json`, and `.cursor/mcp.json`, `tfy hook capabilities` / `tfy hook install --dry-run` / `tfy hook run` test shim as an official-host-hook planning and equivalence surface, `tfy status` with `lifecycle_summary`, `tfy explain`, `tfy launch-report`, `tfy init` marker-bounded Codex guidance, `tfy doctor` local readiness diagnostics, `tfy smoke --mcp` automated Code I/O smoke, `tfy smoke --all` combined adapter+MCP evidence smoke, host checklist smoke, and `tfy gain` command-output ledger-derived savings reporting. `status --json` exposes a lifecycle_summary plus explicit effective project-over-global desired/configured/active state and next-action guidance, including desired_source=none in fresh projects and project_parse_error when local lifecycle state is malformed; `launch-report --json` exposes the minimum v1 host matrix (`mcp_stdio`, `tfy_agent_adapter`, `generic_shell`, plus non-required named hosts) with canonical evidence statuses plus a claim evidence ladder (`config_snippet_available` → `config_written` → `host_launched` → `verified_host_mcp_invocation` / `verified_host_hook` → `route_evidence_recorded` → `savings_verified` → `launch_supported`) and compatibility claim tiers (`not_configured`, `configurable`, `applied_unverified`, `smoke_routed`, `verified_local_mcp`, `launch_supported`, `planned_discovery`; lifecycle status additionally uses `configured_unverified` for a safe config write that still has no host evidence). The v1 pass gate requires `mcp_stdio`, `tfy_agent_adapter`, and `generic_shell` to become `launch_supported`; named hosts remain below launch support until real host invocation plus host-bound TFY ledger/raw/no-negative/positive-savings evidence exists. Hook targets remain dry-run/planned unless official host docs plus e2e route evidence exist; Codex hook support is unsupported without a public official hook route. OpenClaw is `planned_discovery`.
- Explicitly out of scope: provider/API request proxying and editor auto-integration. Not claimed: private/hidden Codex prompt hooks or universal human-shell interception. Supported AI routing is through wrapper/adapter/MCP host configuration, and fuzzy workspace apply is implemented only for proof-gated unique-anchor edits.

`tfy start --agent` automatically configures the highest-priority supported host route with an implemented safe reversible project writer: Codex project `.codex/config.toml`. Claude Code project `.mcp.json` and Cursor project `.cursor/mcp.json` are available through explicit `--host claude-code`, `--host cursor`, or `--host all`; Codex AGENTS.md guidance through `tfy init` remains optional guidance, not the MCP route itself. Use `tfy start --agent --no-apply` when lifecycle intent should be recorded without host config mutation. TFY should not claim automatic model input/output interception for a runtime until that runtime adapter exists and passes the relevant gates. The MCP foundation is a supported MCP tool/resource integration point; it still requires the host agent to route through MCP and is not a private Codex hook or universal shell interception layer. Hook support is a thin router posture only: no hook route may implement summarization, redaction, restore, apply, or claim promotion outside the shared TFY gateways, and host-specific hook writers stay dry-run/planned until official docs, kill-switch, uninstall, and e2e evidence exist. Setup/config success is not token-savings success: launch-supported status requires real host invocation plus route-bound TFY raw/ledger/no-negative/positive-savings/overhead evidence, and is demoted if the evidence is stale, reverify fails, or the host/route/config/version scope changes.

## MCP/Codex adapter foundation

Product-facing lifecycle path:

```bash
tfy start                 # interactive TUI wizard for agent/human/both when attached to a TTY
tfy start --agent         # project AI-agent lifecycle + safe default Codex MCP route configuration
tfy start --human         # enter supported platform-shell project-scoped human auto-intercept session and mark repo
tfy setup --human --apply  # manually install the same explicit user rc hook offered by interactive start
tfy start --human --auto-activate  # non-interactive/automation marker creation without entering a shell
tfy start ai              # positional alias for --agent; also safe-configures Codex by default
tfy start both            # records agent+human intent; run `tfy start --human` alone to enter the human session
tfy start --agent --no-apply # lifecycle intent only; no host config writes
tfy start --agent --host codex  # writes project .codex/config.toml, active=false until evidence
tfy start --agent --host claude-code # writes project .mcp.json, active=false until evidence/approval
tfy start --agent --host cursor # writes project .cursor/mcp.json, active=false until evidence
tfy status --agent --json
tfy stop --agent          # disable intent without deleting raw evidence
tfy fuckyou --agent --yes # scoped TFY-owned lifecycle cleanup; preserves .tfy/raw by default
tfy global start --agent  # user-global lifecycle intent; separate from project .tfy
tfy use always --agent    # convenience alias for user-global default-on intent
tfy use cancel --agent    # convenience alias for user-global default-off intent
```

Lifecycle commands write project state to `.tfy/lifecycle.json` and global state under `$TFY_HOME`, `$XDG_CONFIG_HOME/tfy`, or `~/.tfy`; global host options record default guidance only and do not mutate per-project host config; `start` prepares raw and ledger directories (`.tfy/raw`, `.tfy/state`, `.tfy/adapter`, `.tfy/agent`, `.tfy/mcp`) before later command/context evidence is recorded. Bare interactive `start`, `stop`, and `fuckyou` open an arrow-key TUI; piped stdin choices remain supported for scripts, and missing non-TTY choices fail closed. They do not prove host invocation or token savings by themselves: agent mode auto-configures only supported safe project routes, defaulting to Codex first, and still requires the host to reload/use the TFY route, while human mode does not globally intercept ordinary terminal commands. On supported platform shells (Linux bash, macOS zsh, Windows PowerShell), `tfy start --human` records lifecycle intent and, from an interactive project-only terminal run, creates/refreshes `.tfy/human/auto-activate.json` plus deterministic shell-specific activation content (`auto-activate.bash`, `auto-activate.zsh`, or `auto-activate.ps1`) before offering a default-No prompt to install the one-time user rc/profile hook when it is missing, then entering a TFY-managed project-scoped shell session. Non-interactive plain start remains lifecycle intent-only, while `tfy start --human --auto-activate` is the explicit automation path for marker creation without entering a shell; neither path silently mutates shell rcfiles. Only the accepted interactive prompt or an explicit one-time user rc/profile hook command makes new supported shell sessions auto-activate in trusted marked repos: use `tfy setup --human` to dry-run and `tfy setup --human --apply` to install later; the longer `tfy human auto-activate install --shell <bash|zsh|powershell> --rcfile <path> --apply` remains supported for power users. npm install prints this opt-in guidance but never mutates shell rcfiles. The hook uses a pinned absolute TFY executable, validates/regenerates deterministic repo activation content, and never trusts PATH-resolved `tfy`. Allowlisted commands route through raw-first command-output capture before summary selection; the current ledger stores a combined raw output ref. Shell-local functions, aliases, builtins, direct paths, explicit `tfy-human-bypass`, TFY gateway, and outside-scope commands run raw without a summary claim; automatic interactive/TUI/stateful subcommand classification is not claimed in v1. `tfy human shell --no-auto-intercept` keeps only the managed shell environment without allowlisted wrappers, and `tfy human install --dry-run|--output <path>` generates the sourceable script. `tfy start --agent` records lifecycle desire and, unless `--no-apply` is passed, writes the Codex project MCP route as `configured_unverified`; it still keeps `active=false` until lifecycle desire is on and route-bound raw/ledger/no-negative/positive-savings evidence exists. `tfy status` reports a user-facing lifecycle summary plus effective project-over-global desired/configured/active state and next action guidance. Outside a TFY-managed human session, `tfy shell <command>` runs raw without TFY savings/artifacts; use `tfy shell -- <command>` for the explicit TFY wrapper. Raw evidence remains managed through `tfy raw`; `fuckyou` preserves `.tfy/raw` and shared ledgers by default.

Product-facing happy path:

```bash
tfy init --codex --dry-run
tfy init --codex --project --apply
tfy setup --human --apply # opt-in human auto-activation hook for trusted marked repos
tfy setup --ai --host codex --apply --project
tfy setup --ai --host claude-code --apply --project
tfy setup --ai --host cursor --apply --project
tfy doctor --codex
tfy smoke --mcp
tfy smoke --all --json # emits adapter + agent + MCP ledger paths for launch-report evidence
tfy smoke --codex
tfy gain # reports no-data until command-output savings events exist
# smoke --all emits ledger= and host_evidence= entries that can be passed to launch-report.
tfy launch-report --all --ledger <adapter-ledger> --ledger <agent-ledger> --ledger <mcp-ledger> --host-evidence <host-evidence.json> --json
```

`tfy init` defaults to safe project dry-run behavior. `--apply` writes only a TFY-owned marker block between `<!-- TFY:CODEX:START -->` and `<!-- TFY:CODEX:END -->`; `tfy init --uninstall --codex --project --apply` removes only that block and preserves non-TFY content. Global mode targets `~/.codex/AGENTS.md` instruction guidance and still does not directly mutate `~/.codex/config.toml` in P0.

TFY can run as an MCP stdio server for agent hosts that support MCP:

```bash
tfy mcp capabilities
tfy mcp serve --session local-session --ledger .tfy/mcp/ledger.jsonl --raw-dir .tfy/raw
tfy mcp install --target codex --dry-run
```

The server exposes `tfy_tool_run`, `tfy_raw_get`, `tfy_scope_list`, `tfy_context_get`, `tfy_output_validate`, `tfy_output_apply`, `tfy_state_project`, and `tfy_adapter_report`, plus `tfy://raw/{raw_ref}`, `tfy://report/{session}`, and `tfy://state/{session}` resources. MCP stdout is JSON-RPC only; logs and warnings go to stderr or files. Non-zero child commands are returned as tool results and do not terminate the MCP server.

MCP Code I/O workflow:

1. `tfy_scope_list` returns bounded scope metadata and snapshot-stable `scope.id` selectors; names are display hints only.
2. `tfy_context_get` returns compact selected-scope code, symbol map, `base_compact_code`, `context_ref`, and `ApplyProof`.
3. `tfy_output_validate` restores compact output in preview mode and never mutates the workspace.
4. `tfy_output_apply` applies only through the same proof-gated single-file selected-scope semantics as `tfy output-gateway --apply`; parent event ids or ledger state alone are not authority.
5. `tfy_restore_display` turns compact/restored code into human-readable display text only; it does not create apply authority.
6. `tfy_workspace_validate` and `tfy_workspace_apply` validate/apply explicit WorkspaceApplyPlan operations with a plan hash plus per-operation proofs, including exact multi-file operations (one mutation per target file per plan) and conservative unique-anchor fuzzy edits with required base and preview proof hashes.

This is MCP tool/resource integration. It does not claim private Codex hook interception, provider prompt mutation, or universal shell interception without host MCP routing.

Launch reporting uses exact byte counts recorded from raw/model-visible gateway payloads and a conservative `ceil(bytes/4)` token proxy when tokenizer-specific counts are unavailable. Local smoke ledgers can raise required routes to `verified_local_mcp`, but `launch_supported` additionally requires `--host-evidence` JSON proving setup, real host invocation, config scope/path, route type, smoke id, ledger/raw artifacts, and no-negative plus positive savings for each route. `tfy explain` discloses the local raw-store contract: TFY stores raw command/context evidence under `.tfy/raw` or configured `--raw-dir` plus gateway ledgers such as `.tfy/mcp/ledger.jsonl` and `.tfy/adapter/ledger.jsonl`; TFY does not upload raw evidence. First-class raw lifecycle commands are available: `tfy raw --list`, `tfy raw <raw_ref> --inspect`, `tfy raw <raw_ref> --export <path>`, and `tfy raw --prune --dry-run/--apply`; ledger files remain explicit local files.

### Release readiness dry-run

Developer Preview evidence and RC evidence are separate gates generated from local/release checks, not broad claims:

```sh
./scripts/verify.sh
./scripts/release-dry-run.sh
SMOKE_JSON=.tfy/release/smoke.json
tfy smoke --all --json > "$SMOKE_JSON"
ARGS=(launch-report --all --release-evidence .tfy/release/release-evidence.json --json)
while IFS= read -r item; do
  case "$item" in
    ledger=*) ARGS+=(--ledger "${item#ledger=}") ;;
    host_evidence=*) ARGS+=(--host-evidence "${item#host_evidence=}") ;;
  esac
done < <(python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["evidence"]))' "$SMOKE_JSON")
tfy "${ARGS[@]}"
```

`tfy launch-report` now exposes release tiers (`developer_preview_ready`, `rc_ready`, `ga_ready`, `public_superiority_claim_ready`). GA and public external superiority claims remain blocked unless named-host and benchmark evidence gates pass.
