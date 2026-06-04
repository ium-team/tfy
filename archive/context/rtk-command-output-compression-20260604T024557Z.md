# Context Snapshot: RTK Command Output Compression Add-on

## Task
Use the actual RTK repository (`https://github.com/rtk-ai/rtk`, cloned to `.omx/tmp/rtk`) to concretize the TFY command-output compression feature.

## Desired Outcome
A grounded PRD/test-spec for adding a risk-aware, all-command-output compression layer compatible with RTK's current architecture.

## RTK Evidence Inspected
- `README.md`: RTK already filters/compresses command outputs before LLM context, claims 60-90% savings, supports 100+ commands, and documents four strategies: smart filtering, grouping, truncation, deduplication.
- `README.md`: hooks rewrite Bash commands such as `git status` to `rtk git status`; built-in non-Bash tools are not intercepted.
- `README.md`: tee saves full unfiltered output on failure and prints `[full output: ...]` recovery hint.
- `src/cmds/README.md`: command modules execute external tools, filter stdout/stderr, track savings, and preserve exit codes.
- `src/core/runner.rs`: shared execution wrapper supports `run_filtered`, `run_filtered_with_exit`, `run_streamed`, `run_passthrough`, `RunOptions::with_tee`, `skip_filter_on_failure`, stdout-only filtering, and tracking.
- `src/core/stream.rs`: provides streaming/capture modes, raw stdout/stderr capture, exit code handling, and `RAW_CAP`.
- `src/core/tee.rs`: raw output recovery already supports tee modes `failures`, `always`, `never`, size limits, rotation, and hints.
- `src/core/toml_filter.rs`: declarative filters support strip ANSI, regex replacement, match-output short circuits with `unless`, keep/strip lines, truncation, head/tail/max lines, on_empty, filter_stderr.
- `src/core/truncate.rs`: shared caps distinguish errors/warnings/lists/inventory.
- `src/cmds/system/summary.rs`: heuristic generic command summarizer already detects tests/build/log/list/json/generic.
- `src/cmds/rust/runner.rs`: generic `rtk err <cmd>` and `rtk test <cmd>` already preserve error/failure signals and tee full output.

## Current Gap
RTK already compresses many command outputs, but policy is command/filter-specific. The new TFY-inspired feature should define a shared risk-aware output-budget layer:

- error/important output: preserve actionable raw evidence, compress only low-risk formatting/noise
- success/no-action output: compress aggressively
- all output: retain raw output reference for fallback/audit

## Constraints
- Planning only; no RTK implementation changes in this session.
- Must preserve RTK's existing fail-safe, exit-code preservation, tracking, tee, and hook architecture.
- Must not hide important diagnostics.
- Should integrate with TOML filters and Rust command modules rather than requiring every command to be rewritten.
