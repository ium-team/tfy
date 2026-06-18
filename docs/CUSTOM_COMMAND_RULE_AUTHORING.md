# Custom Command Rule Authoring

This document is the policy for agents that create TFY custom command rules.

Custom rules are powerful, but they are still local declarative summarization rules. They must never bypass TFY's raw-first, no-negative, redaction, trust, plain-text, and truthful-claims model.

## What an authoring agent may do

An authoring agent may:

- inspect existing `docs/COMMAND_RULES.md` and project command patterns;
- run a representative command through a TFY gateway when safe and requested;
- inspect a supplied raw output sample or raw ref;
- draft repo-local `.tfy/commands.toml` rules inside the `tfy custom` harness;
- add v2 sections, counters, captures, and severity buckets;
- add v3 bounded structured extracts, metrics, and groups when fixture evidence proves they are useful;
- add explicit v3 built-in override metadata only when the user asks to replace a built-in summary and `compare-built-in` evidence supports the replacement;
- validate strict parsing and preview model-visible output;
- update `.tfy/trust.json` only through `tfy custom trust` after verification evidence matches the final rule bytes.

## What an authoring agent must not do

An authoring agent must not:

- add script execution, shell hooks, subprocesses, or plugin code to TOML rules;
- claim a custom rule is official TFY support or RTK parity evidence;
- override built-in TFY strategies unless the user explicitly asks for a v3 `[command.override]` rule and fixture comparison proves the replacement is better; never imply `schema_version = 3` alone changes precedence;
- mark a command `human_auto_safe = true` unless it is clearly noninteractive and low risk;
- hide failures by stripping all error evidence;
- skip validation because a TOML rule “looks right.”

## Required workflow

1. **Choose scope**
   - Run bare `tfy custom` to open the scope wizard. Choose repo for the active provenance-backed harness.
   - Use repo-local `.tfy/commands.toml` through `tfy custom` for project-specific commands.
   - Treat user-global `~/.config/tfy/commands.toml` as legacy/manual compatibility until a provenance-backed global custom flow exists; the bare wizard's global choice fails closed today instead of creating active global trust.

2. **Collect evidence**
   - Prefer a TFY route so raw evidence is stored first.
   - If running the command is unsafe or expensive, ask for or inspect a representative output sample.
   - Identify progress/noise lines, failure lines, file refs, counters, and sensitive values.

3. **Draft conservative TOML**
   - Use a safe `command.id`: ASCII letters, digits, `_`, or `-`, length 1..64.
   - Prefer `match.argv_prefix` over `match.command_regex`.
   - Use `schema_version = 2` for sections/counters/captures/severity.
   - Use `schema_version = 3` for bounded JSON/NDJSON/KV/table extracts, metrics, groups, or explicit built-in override metadata.
   - Add `[command.override] built_in = true` only with exact `family = "<classified_family>"` and a short redaction-safe `reason`.
   - Put specific override rules before broader custom rules for the same command because first match wins.
   - Keep `max_lines`, `truncate_lines_at`, and `max_items` low enough for token savings.
   - Use captures only for display values, never execution input.

4. **Validate with the harness**
   - Bare `tfy custom` with the repo choice or `tfy custom init --repo .` must create the bounded workspace.
   - `tfy custom capture --repo . --name <name> -- <command...>` or `tfy custom import-fixture --repo . --name <name> --file <sample>` must create fixture metadata. `capture` runs from `--repo` and records a merged stdout+stderr fixture; use `import-fixture` for stream-specific samples or secret-bearing commands.
   - `tfy custom prompt --repo . --agent <codex|claude|generic> --name <name>` must generate bounded agent instructions.
   - `tfy custom verify --repo . --name <name> --json` must accept the rule, prove the fixture exercised a concrete `user_toml` rule id, and record validate/preview/compare evidence.
   - Tiny outputs must still pass through under no-negative behavior.
   - Raw refs must remain recoverable.
   - If overriding a built-in, preview/compare output must show `strategy_kind = "user_toml"`, the expected `rule_id`, a `user_rule_overrode_builtin` diagnostic, and `comparison.override_active = true`; family mismatches must keep the built-in.
   - If `comparison.with_rules_larger_chars_vs_without_rules` is nonzero, report the token-saving regression and revise the rule unless the user explicitly accepts the local tradeoff.

5. **Trust repo-local rules**
   - Review final `.tfy/commands.toml` bytes and `.tfy/custom/<name>.verify.json` evidence.
   - Run `tfy custom trust --repo . --name <name> --json`; this writes schema v2 trust provenance.
   - If the rule file or fixture changes later, trust must fail until `verify` and `trust` are rerun.

6. **Report honestly**
   - State which rule id was added.
   - State whether it is repo-local or user-global.
   - State validation commands and results.
   - State unsupported assumptions and that this remains local/custom support.

## Example

```toml
schema_version = 3

[[command]]
id = "project_build"
description = "Summarize project build diagnostics"
match.argv_prefix = ["pnpm", "build"]
strip_lines_matching = ["(?i)^(progress|cache|download)"]
head_lines = 8
tail_lines = 8
max_lines = 20
truncate_lines_at = 220
human_auto_safe = true
agent_safe = true
interactive_risk = "none"

[[command.section]]
name = "errors"
title = "Errors"
keep_lines_matching = ["(?i)(error|failed|fatal)"]
max_lines = 20

[[command.counter]]
name = "errors"
match = "(?i)(error|failed|fatal)"

[[command.capture]]
name = "files"
pattern = '^(?<file>[^:\\s][^:]+):(?<line>\\d+):'
field = "file"
dedupe = true
max_items = 25
```

## Suggested validation commands

```bash
cargo run -q -p tfy-cli -- custom init --repo .
cargo run -q -p tfy-cli -- custom capture --repo . --name <fixture> -- <command> <arg>
cargo run -q -p tfy-cli -- custom prompt --repo . --agent codex --name <fixture>
cargo run -q -p tfy-cli -- custom verify --repo . --name <fixture> --json
cargo run -q -p tfy-cli -- custom trust --repo . --name <fixture> --json
cargo test -p tfy-core user_toml
```

Use low-level `tfy rules validate/preview/compare-built-in` only for expert debugging. For repo-local trust, prefer `tfy custom trust` because it records fixture and validation provenance.
