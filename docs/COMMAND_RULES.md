# Command Rules

TFY command rules let a user add local command summaries without changing TFY Rust code.
They are an extension layer over the existing TFY command summary pipeline.

## Scope and guarantees

User command rules are **local/custom support**, not official TFY command support and not RTK-comparison evidence.
They never bypass TFY's core invariants:

1. exact raw stdout/stderr bytes are stored first;
2. public/model-visible lines are redacted;
3. summaries are shown only when smaller than redacted public raw output;
4. plain text remains the default model-visible output;
5. repo-local rules require an explicit trust record.

In v1, user TOML rules are **additive-only**. Existing TFY built-in Rust strategies and built-in DSL filters keep precedence. If a user rule also matches a built-in family, the built-in result is used and TFY records a `user_rule_shadowed_by_builtin` diagnostic.

## Rule locations

TFY loads rules in this order after built-in strategies do not produce a candidate:

1. trusted repo-local `.tfy/commands.toml` discovered from the current directory or nearest ancestor;
2. user-global `~/.config/tfy/commands.toml`;
3. generic fallback / unsupported passthrough.

User-global rules are user-owned and load by default when present. Repo-local rules are ignored until trusted.

## Repo-local trust

Repo-local `.tfy/commands.toml` is trusted only when `.tfy/trust.json` exists and its hash matches the current rule file bytes:

```json
{
  "schema_version": 1,
  "command_rules": {
    "trusted": true,
    "rules_sha256": "<sha256 of .tfy/commands.toml>"
  }
}
```

If `.tfy/commands.toml` changes after trust, TFY skips repo-local rules and emits `repo_rules_hash_mismatch`. Missing trust emits `repo_rules_untrusted`.

This trust authorizes only declarative summarization. It does not authorize code execution, workspace apply, shell mutation, or official support claims.

## TOML v1 schema

```toml
[[command]]
id = "internal_build"
description = "Summarize internal build diagnostics"
match.argv_prefix = ["my-build", "run"]
match.command_regex = '^my-build\\s+run(?:\\s|$)'

strip_ansi = true
preserve_lines_matching = ["(?i)(error|failed|fatal|warning)"]
strip_lines_matching = ["(?i)^(downloading|progress|debug)"]
keep_lines_matching = []
head_lines = 16
tail_lines = 12
max_lines = 48
truncate_lines_at = 220
on_empty = "internal_build: no relevant output"

human_auto_safe = true
agent_safe = true
interactive_risk = "none"
```

### Matching

- `match.argv_prefix` compares against the original argv vector and is preferred.
- `match.command_regex` is an optional fallback against the display command string.
- At least one matcher is required.

### Filtering

- `preserve_lines_matching` copies important diagnostic lines into the summary.
- `strip_lines_matching` drops noise lines before selection.
- `keep_lines_matching` restricts output to matching lines unless a line is preserved.
- `head_lines`, `tail_lines`, `max_lines`, and `truncate_lines_at` bound visible output.
- All visible lines are redacted before output.
- `on_empty` is optional, capped, redacted, and defaults to `<rule_id>: no relevant output` so local paths are not exposed.

### Safety metadata

`human_auto_safe`, `agent_safe`, and `interactive_risk` are metadata. They do not make a command officially supported and do not override built-in safety decisions.

Allowed `interactive_risk` values are `none`, `possible`, and `unknown`.

## Invalid configs and diagnostics

Runtime surfaces are non-strict by default: invalid user rule entries are skipped when possible, diagnostics are emitted, and valid entries from the same file can still apply. Whole-file TOML syntax errors skip that file and command execution continues through built-in/generic behavior.

Strict parser/test APIs fail on invalid TOML, invalid regex, duplicate ids, unsupported fields, or unsafe limits.

V1 diagnostic codes:

- `repo_rules_untrusted`
- `repo_rules_hash_mismatch`
- `user_rules_invalid_toml`
- `user_rules_invalid_regex`
- `user_rules_duplicate_id`
- `user_rules_unsafe_limit`
- `user_rules_unsupported_field`
- `user_rule_shadowed_by_builtin`

Gateway and ledger metadata include `rule_id`, `strategy_source_kind`, and `command_rule_diagnostics` when applicable. Adapter/MCP reports aggregate `rule_counts`, `strategy_source_counts`, and `command_rule_diagnostic_counts`.

## Human mode note

This feature does not by itself make `tfy start --human` intercept every command. Current managed human shell interception is still limited by the shell integration's wrapper behavior. User TOML rules work through TFY gateway paths such as `tfy tool-gateway`, `tfy shell`, and `tfy human run` when those paths receive the command argv.
