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


## Installation and first start

The source build remains the authority path today. The npm package name is `@ium/tfy-cli` because the unscoped `tfy` npm name is already occupied and unscoped `tfy-cli` is blocked by npm similarity policy; the installed command must still be `tfy`.

TFY is stable-first: the normal user path is `npm install -g @ium/tfy-cli` once the npm `latest` tag points at a reviewed stable release. Until that first stable release exists, use the explicit beta channel for development/validation builds only. Beta and stable are release maturity states for the same TFY product, not separate feature lines.

```sh
# Stable path after the first stable release exists:
npm install -g @ium/tfy-cli

# Current beta dogfood path before the first stable release:
npm install -g @ium/tfy-cli@beta

# Enable TFY in the current project:
tfy start          # interactive choice when attached to a TTY
tfy start --human  # human managed shell path
tfy start --agent  # AI-agent wrapper + Codex/Claude Code hook setup path

tfy status --json
```

The npm package is a thin installer/launcher. It downloads the matching GitHub Release archive plus checksum and exposes the `tfy` command. The public names are intentionally different:

| Layer | Name | Why |
| --- | --- | --- |
| Product/repo | `TFY` / `tfy` | Human-facing product and command identity. |
| Rust crate | `tfy-cli` | Cargo package name for the CLI implementation. |
| npm package | `@ium/tfy-cli` | Public npm installer package because unscoped `tfy` is occupied and unscoped `tfy-cli` is blocked by npm similarity policy. |
| Installed executable | `tfy` | The command users run after install. |

TFY uses two forward public install channels only:

- Stable channel: `npm install -g @ium/tfy-cli` installs the most tested completed release through the npm `latest` dist-tag, but only after the first stable release exists. Do not point `latest` at beta builds.
- Beta channel: `npm install -g @ium/tfy-cli@beta` installs the newest development/validation build with current work included. This is the only recommended npm install form while TFY has beta releases but no stable release.

See [`docs/RELEASE_READINESS.md`](docs/RELEASE_READINESS.md) for the canonical npm dist-tag cleanup and verification checklist.

Exact versions remain installable with standard npm syntax, for example `npm install -g @ium/tfy-cli@0.1.1` or `npm install -g @ium/tfy-cli@0.1.1-beta.0`; npm uses `@<version>`, not `/v<version>`. Historical `0.1.0-preview.*` versions remain installable by exact version if they exist, but new development builds use the beta channel. Beta releases must use the npm `beta` dist-tag, not `latest`, until they graduate to the stable channel. Local validation of the install path is available with:

Supported prebuilt npm/GitHub Release platforms:

| OS | Architecture | Rust target | npm prebuilt |
| --- | --- | --- | --- |
| macOS | Apple Silicon arm64 | `aarch64-apple-darwin` | yes |
| Linux | x64 | `x86_64-unknown-linux-gnu` | yes |
| Linux | arm64 | `aarch64-unknown-linux-gnu` | yes |
| Windows | x64 | `x86_64-pc-windows-msvc` | yes |

Intel Mac (`darwin:x64` / `x86_64-apple-darwin`) is not provided as a prebuilt npm/GitHub Release archive. Intel Mac users can still build from source with `git clone https://github.com/ium-team/tfy && cd tfy && cargo install --path crates/tfy-cli`, or run a locally built binary by setting `TFY_BINARY_PATH`.


```sh
./scripts/npm-beta-smoke.sh
```

For the plain-language version bump rules, see [`docs/releases/VERSIONING.md`](docs/releases/VERSIONING.md).

GitHub Releases are produced by the manual `.github/workflows/release.yml` workflow. It builds all npm-supported platform archives/checksums, refuses mismatched channel/version/source metadata, creates the GitHub Release when `dry_run` is false, and then publishes npm with the matching channel dist-tag once npm Trusted Publishing is configured for this repository workflow. Use the npm publish plan helper to inspect the exact publish and verification commands:

```sh
node scripts/npm-publish-plan.js --version 0.1.1-beta.0 --channel beta --source-ref develop
node scripts/npm-publish-plan.js --version 0.1.1 --channel stable --source-ref main
```

The npm package defaults its `publishConfig.tag` to `beta` as a safety rail; stable publishes must explicitly use the generated `--tag latest` command. The release workflow uses Trusted Publishing/OIDC rather than a long-lived npm token, and the user-owned npm-side Trusted Publisher setup is documented in `docs/RELEASE_READINESS.md`. The publish helper also prints the required `scripts/npm-dist-tag-check.js` guard; the canonical dist-tag cleanup checklist lives in `docs/RELEASE_READINESS.md`.

For AI-agent use, bare `tfy start --agent` records project intent, creates `.tfy/agent/tfy-agent-wrapper`, and prepares the safe project-local Codex `.codex/config.toml` plus Claude Code `.claude/settings.json` official hook routes. If the user later opens Codex in that project, the Codex route is ready; if they open Claude Code, the Claude route is ready. `tfy start --agent --host codex` and other named-host options remain explicit/narrow host setup routes, but setup only proves configuration. Launch support still requires real host invocation plus TFY raw/ledger/no-negative/positive-savings evidence. For human use on supported platform shells (Linux bash, macOS zsh, Windows PowerShell), interactive project-only `tfy start --human` records lifecycle intent, creates/refreshes the trusted repo-local future-shell marker/script as persistent repo state, and enters a TFY-managed project-scoped shell session where safely resolved command names are wrapped/proxied by TFY and summarized only after raw evidence is stored and only when beneficial. During that interactive start, TFY reports whether the one-time user rc/profile hook is already installed; if it is missing, TFY offers a default-No prompt to install it for future supported shells. Non-interactive plain `tfy start --human` remains lifecycle intent-only; automation that wants marker creation without entering a shell must use `tfy start --human --auto-activate`, and neither path silently edits shell startup files. `tfy setup --human --apply` remains the short manual command for the same explicit hook install; the longer `tfy human auto-activate install --shell <bash|zsh|powershell> --rcfile <path> --apply` remains supported for explicit startup-file control. New supported shell sessions that read that hook auto-activate only inside marked repos; TFY pins the absolute executable and validates deterministic activation content before sourcing. Outside managed/marked sessions, `tfy shell <command>` is raw passthrough convenience and `tfy shell -- <command>` is the TFY summarizing wrapper. TFY does not claim universal or global terminal interception.

## Implementation status

The current authority-path implementation is **Rust core + Rust CLI**. TFY is Rust-first where correctness, security, deterministic command behavior, raw evidence, redaction, proof validation, and workspace apply matter. Non-core integrations may use host-native or other best-fit technologies when that improves released-product quality, provided they do not become correctness dependencies for authority-path behavior without an explicit stack decision record. Python product/runtime surfaces have been retired: there is no root Python package, PyO3 binding crate, Python lockfile, or Python test suite in the release path. Python remains only a supported input language for code analysis through tree-sitter fixtures and may be used for non-runtime evaluation or research tooling only when it is not a product/runtime dependency.

Rust smoke commands:

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

Product-facing lifecycle path:

```bash
tfy start                 # interactive TUI wizard for agent/human/both when attached to a TTY
tfy start --agent         # project AI-agent lifecycle + wrapper + Codex/Claude Code hooks
tfy start --human         # enter supported platform-shell project-scoped human auto-intercept session and mark repo
tfy setup --human --apply  # manually install the same explicit user rc hook offered by interactive start
tfy start --human --auto-activate  # non-interactive/automation marker creation without entering a shell
tfy start ai              # positional alias for --agent; same default wrapper + Codex/Claude setup
tfy start both            # records agent+human intent; run `tfy start --human` alone to enter the human session
tfy start --agent --no-apply # lifecycle intent only; no host config writes
tfy start --agent --host codex  # writes project .codex/config.toml PreToolUse Bash hook, active=false until evidence
tfy start --agent --host claude-code # writes project .claude/settings.json PreToolUse Bash hook, active=false until evidence
tfy status --agent --json
tfy stop --agent          # disable intent without deleting raw evidence
tfy fuckyou --agent --yes # scoped TFY-owned lifecycle cleanup; preserves .tfy/raw by default
tfy global start --agent  # user-global lifecycle intent; separate from project .tfy
tfy use always --agent    # convenience alias for user-global default-on intent
tfy use cancel --agent    # convenience alias for user-global default-off intent
```

Product-facing happy path:

`tfy init` defaults to safe project dry-run behavior. `--apply` writes only a TFY-owned marker block between `<!-- TFY:CODEX:START -->` and `<!-- TFY:CODEX:END -->`; `tfy init --uninstall --codex --project --apply` removes only that block and preserves non-TFY content. Global mode targets `~/.codex/AGENTS.md` instruction guidance and still does not directly mutate `~/.codex/config.toml` in P0.

Supported CLI surfaces for that flow are `tfy context-gateway`, `tfy output-gateway`, `tfy restore-display`, `tfy restore-file`, `tfy restore-patch`, and `tfy workspace validate/apply`. These remain CLI/runtime-facing surfaces with explicit proof gates; they do not create workspace apply authority from parent event ids or ledger state alone.

### Release readiness dry-run

Beta evidence and RC evidence are separate gates generated from local/release checks, not broad claims:

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

`tfy launch-report` now exposes release tiers (`developer_preview_ready` legacy machine key / beta readiness label, `rc_ready`, `ga_ready`, `public_superiority_claim_ready`). GA and public external superiority claims remain blocked unless named-host and benchmark evidence gates pass.
