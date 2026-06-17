# Custom Command Rule Authoring

This document is the policy for agents that create TFY custom command rules.

Custom rules are powerful, but they are still local declarative summarization rules. They must never bypass TFY's raw-first, no-negative, redaction, trust, plain-text, and truthful-claims model.

## What an authoring agent may do

An authoring agent may:

- inspect existing `docs/COMMAND_RULES.md` and project command patterns;
- run a representative command through a TFY gateway when safe and requested;
- inspect a supplied raw output sample or raw ref;
- draft `.tfy/commands.toml` or `~/.config/tfy/commands.toml` rules;
- add v2 sections, counters, captures, and severity buckets;
- validate strict parsing and preview model-visible output;
- update `.tfy/trust.json` only after the final rule bytes are reviewed.

## What an authoring agent must not do

An authoring agent must not:

- add script execution, shell hooks, subprocesses, or plugin code to TOML rules;
- claim a custom rule is official TFY support or RTK parity evidence;
- override built-in TFY strategies;
- mark a command `human_auto_safe = true` unless it is clearly noninteractive and low risk;
- hide failures by stripping all error evidence;
- skip validation because a TOML rule “looks right.”

## Required workflow

1. **Choose scope**
   - Use repo-local `.tfy/commands.toml` for project-specific commands.
   - Use user-global `~/.config/tfy/commands.toml` for personal commands across projects.

2. **Collect evidence**
   - Prefer a TFY route so raw evidence is stored first.
   - If running the command is unsafe or expensive, ask for or inspect a representative output sample.
   - Identify progress/noise lines, failure lines, file refs, counters, and sensitive values.

3. **Draft conservative TOML**
   - Use a safe `command.id`: ASCII letters, digits, `_`, or `-`, length 1..64.
   - Prefer `match.argv_prefix` over `match.command_regex`.
   - Use `schema_version = 2` for sections/counters/captures/severity.
   - Keep `max_lines`, `truncate_lines_at`, and `max_items` low enough for token savings.
   - Use captures only for display values, never execution input.

4. **Validate**
   - Strict parser must accept the rule.
   - Gateway preview must show redacted model-visible output.
   - Tiny outputs must still pass through under no-negative behavior.
   - Raw refs must remain recoverable.

5. **Trust repo-local rules**
   - Review final `.tfy/commands.toml` bytes.
   - Compute/update `.tfy/trust.json` only after review.
   - If the file changes later, trust must fail until updated.

6. **Report honestly**
   - State which rule id was added.
   - State whether it is repo-local or user-global.
   - State validation commands and results.
   - State unsupported assumptions and that this remains local/custom support.

## Example

```toml
schema_version = 2

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
cargo test -p tfy-core user_toml
cargo run -q -p tfy-cli -- tool-gateway -- <command> <args>
cargo run -q -p tfy-cli -- adapter report --session <session> --ledger <ledger> --json
```

For repo-local trust, compute the SHA-256 of `.tfy/commands.toml` and write it to `.tfy/trust.json` using the format in `docs/COMMAND_RULES.md`.
