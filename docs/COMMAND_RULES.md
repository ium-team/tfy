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

User TOML rules are additive by default. Existing TFY built-in Rust strategies and built-in DSL filters keep precedence unless a v3 rule explicitly opts in to a family-bound built-in override. If a user rule also matches a built-in family without a valid override, the built-in result is used and TFY records a `user_rule_shadowed_by_builtin` or `user_rule_override_family_mismatch` diagnostic.

## Rule locations

TFY loads rules in this order for custom candidates. Built-ins normally win; only trusted v3 rules with `[command.override] built_in = true` and an exact `family` match can replace a built-in candidate:

1. trusted repo-local `.tfy/commands.toml` discovered from the current directory or nearest ancestor;
2. legacy/manual user-global `~/.config/tfy/commands.toml` compatibility rules;
3. generic fallback / unsupported passthrough.

`tfy custom` is the recommended authoring path and is repo-local by default. User-global rules still load at runtime for compatibility, but TFY emits `user_global_rules_legacy_manual` diagnostics and `tfy custom verify` refuses to proceed by default when a global rule file exists. Use `--allow-legacy-global-rules` only when intentionally recording that external/manual influence. A future `tfy custom --global` flow should add provenance-backed trust for global rules.

## Repo-local trust

Repo-local `.tfy/commands.toml` is trusted only when `.tfy/trust.json` exists and its hash matches the current rule file bytes. Legacy v1 trust is still accepted for compatibility and emits `repo_rules_legacy_trust`:

```json
{
  "schema_version": 1,
  "command_rules": {
    "trusted": true,
    "rules_sha256": "<sha256 of .tfy/commands.toml>"
  }
}
```

`tfy custom trust` writes provenance-backed v2 trust instead:

```json
{
  "schema_version": 2,
  "command_rules": {
    "trusted": true,
    "rules_sha256": "<sha256 of .tfy/commands.toml>",
    "created_by": "tfy custom",
    "validated_at": "unix:<seconds>",
    "validated_with": ["validate", "preview", "compare-built-in"],
    "fixtures": [{"name": "quality", "fixture_sha256": "<sha256>", "path": ".tfy/rule-fixtures/quality.txt", "cmd": ["pnpm", "test"]}],
    "agent": {"kind": "user", "bounded_workspace": true},
    "override_evidence": []
  }
}
```

If `.tfy/commands.toml` or a trusted fixture changes after trust, TFY skips repo-local rules and emits `repo_rules_hash_mismatch`. Missing trust emits `repo_rules_untrusted`. `tfy custom status --json` reports `repo_local.state` as `trusted_v2`, `trusted_v1_legacy`, `stale`, `invalid`, or `untrusted`, and includes `trust_schema_version` so migration state is explicit.

This trust authorizes only declarative summarization. It does not authorize code execution, workspace apply, shell mutation, OS-level sandboxing, or official support claims.

## Safety pipeline

All rule versions compile into an internal normalized rule model before evaluation. User TOML does not directly write `model_text`.

```text
TOML schema
→ strict/non-strict parser
→ normalized command rule
→ rule evaluator
→ summary parts
→ shared plain-text renderer
→ public redaction/capping
→ raw_ref append
→ no-negative selector
```

This means sections, counters, captures, severity buckets, structured extracts, metrics, groups, and v1 line filters all share the same terminal redaction and no-negative gate.

## TOML v1 schema

Files without `schema_version` use v1-compatible semantics. `command.id` must be a safe identifier: ASCII letters, digits, `_`, or `-`, with length 1..64. TFY renders this id in summaries and diagnostics, so paths, spaces, and secrets are intentionally rejected.

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

## TOML v2 core schema

Use `schema_version = 2` for richer declarative summaries.

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

[[command.section]]
name = "errors"
title = "Errors"
keep_lines_matching = ["(?i)(error|failed|fatal)"]
max_lines = 20
truncate_lines_at = 220

[[command.section]]
name = "warnings"
title = "Warnings"
keep_lines_matching = ["(?i)(warning|deprecated)"]
max_lines = 10

[[command.counter]]
name = "errors"
match = "(?i)(error|failed|fatal)"
max_count = 10000

[[command.capture]]
name = "files"
pattern = '^(?<file>[^:\\s][^:]+):(?<line>\\d+):'
field = "file"
dedupe = true
max_items = 25

[[command.severity]]
level = "critical"
match = "(?i)(panic|fatal|segmentation fault)"
```

### Matching

- `match.argv_prefix` compares against the original argv vector and is preferred.
- `match.command_regex` is an optional fallback against the display command string.
- At least one matcher is required.

### v1 line filtering

- `preserve_lines_matching` copies important diagnostic lines into the summary.
- `strip_lines_matching` drops noise lines before selection.
- `keep_lines_matching` restricts output to matching lines unless a line is preserved.
- `head_lines`, `tail_lines`, `max_lines`, and `truncate_lines_at` bound visible output.
- `on_empty` is optional, capped, redacted, and defaults to `<rule_id>: no relevant output` so local paths are not exposed.

### v2 sections

`[[command.section]]` creates named plain-text sections from filtered output. Section fields intentionally mirror the v1 filter fields: `keep_lines_matching`, `preserve_lines_matching`, `strip_lines_matching`, `head_lines`, `tail_lines`, `max_lines`, and `truncate_lines_at`.

- Section `name` must be a safe identifier.
- `title` is optional and redacted/capped.
- Empty sections are omitted by default.
- Section lines are redacted and capped before model visibility.

### v2 scan budget

Counters, captures, and severity rules scan normalized public lines, not private raw bytes. Each scanned line is redacted and capped to 1,000 characters before regex evaluation. This keeps custom rules bounded and prevents model-visible or rule-derived metadata from depending on unbounded long single-line output; exact raw evidence remains recoverable through `raw_ref`.

### v2 counters

`[[command.counter]]` counts regex matches over normalized public lines within the v2 scan budget.

- Counter names must be safe identifiers.
- `max_count` caps runaway counts.
- Counters can add caution/evidence but cannot fabricate success.

### v2 captures

`[[command.capture]]` extracts a named regex group from normalized public lines within the v2 scan budget and renders capped/deduped values.

- `pattern` must contain the named group referenced by `field`.
- Captured values are redacted before output.
- `dedupe = true` preserves deterministic first-seen order.
- Captures never become command input or execution authority.

### v2 severity

`[[command.severity]]` can annotate output with custom severity by scanning normalized public lines within the v2 scan budget.

Allowed levels:

- `info`
- `warning`
- `error`
- `critical`

Custom severity may make a summary more cautious, but it cannot downgrade nonzero exits, built-in hard-failure evidence, or TFY risk decisions.


## TOML v3 parity-oriented schema

Use `schema_version = 3` when a custom rule needs structured extraction or aggregate views closer to built-in summaries. v3 is still declarative: it does **not** enable shell execution, arbitrary scripts, or official-support claims. Built-ins still win on family conflicts by default and emit `user_rule_shadowed_by_builtin`.

```toml
schema_version = 3

[[command]]
id = "quality_report"
match.argv_prefix = ["quality-report"]
strip_lines_matching = ["(?i)^progress"]
max_lines = 12
truncate_lines_at = 180

[[command.parse_ndjson]]
name = "files"
path = "errors[*].file"
max_items = 10

[[command.parse_kv]]
name = "duration"
key = "duration_ms"
separators = ["="]
max_items = 2

[[command.parse_table]]
name = "failures"
columns = ["file", "status"]
delimiter = "whitespace"
max_rows = 10

[[command.metric]]
name = "error_mentions"
op = "count"
match = "(?i)error|failed|fatal"
max_count = 10000

[[command.group]]
name = "by_file"
pattern = 'file=(?<file>[^\s]+)'
field = "file"
top_k = 10
```

### v3 structured extracts

Structured extracts render as capture blocks and are still redacted/capped before model visibility. JSON/NDJSON parsing is bounded to 1 MB of raw text and at most 100 extracted values per operation. KV and table extraction read normalized public lines, so they inherit the v2 public-line scan budget.

- `[[command.parse_json]]` parses the whole raw output as JSON.
- `[[command.parse_ndjson]]` parses each valid JSON line independently.
- `path` supports simple dotted fields plus array selectors such as `errors[*].file` or `items[0].name`.
- `[[command.parse_kv]]` extracts values from `key=value` or configured one-character separators.
- `[[command.parse_table]]` finds a header row containing requested columns, then renders bounded `column=value` rows. Delimiters are `whitespace` or `comma`.

### v3 metrics and groups

`[[command.metric]]` supports `op = "count"` and `op = "unique_count"` over normalized public lines. `max_count` bounds runaway outputs.

`[[command.group]]` counts a named regex capture and renders top values as a deterministic `group.<name>` section. `top_k` is capped at 100. Groups are useful for top failing files, tests, packages, shards, hosts, or error classes.

### v3 built-in override control plane

A v3 rule can intentionally replace a built-in summary only with explicit, family-bound metadata:

```toml
schema_version = 3

[[command]]
id = "project_df"
match.argv_prefix = ["df"]
keep_lines_matching = ["Filesystem|Use%|/dev/"]
max_lines = 12

[command.override]
built_in = true
family = "df"
reason = "prefer project disk pressure view"
```

Override rules are still custom/local support. The control plane is deliberately narrow:

- `schema_version = 3` alone never changes precedence.
- `[command.override] built_in = true` requires `family`, and `family` must exactly match TFY's classified command family such as `df`.
- If the family is wrong, TFY keeps the built-in candidate and emits `user_rule_override_family_mismatch`.
- If the override is valid, TFY uses the custom candidate, marks `strategy_kind = "user_toml"`, records `rule_id`, and emits `user_rule_overrode_builtin`.
- Raw-first storage, redaction, capping, and the no-negative selector still run after the custom render. If the custom text is not smaller than redacted public raw output, model-visible output passes through instead of showing a larger summary.
- Repo-local override rules still require `.tfy/trust.json`; user-global override rules remain legacy/manual local configuration until a provenance-backed global custom flow exists.
- If no built-in candidate exists for the matched command, the same rule behaves like a normal custom rule; `override.family` only controls replacement of an existing built-in candidate.
- Rule order is significant: TFY uses the first matching custom rule, so place a more specific override rule before broader custom rules for the same command.

Use `tfy rules compare-built-in ... --json` after adding override metadata to compare effective custom-rule behavior against TFY built-in/default behavior for representative fixtures. The JSON includes `comparison.override_active`, both strategy kinds, and model-visible character counts so an authoring agent can flag overrides that are less useful than the built-in/default result.

### Custom harness commands

Agents should use the official `tfy custom` harness instead of hand-editing without proof:

```bash
tfy custom init --repo .
tfy custom capture --repo . --name quality-report -- quality-report --json
tfy custom prompt --repo . --agent codex --name quality-report
tfy custom verify --repo . --name quality-report --json
tfy custom trust --repo . --name quality-report --json
```

For a pre-existing sample, use `tfy custom import-fixture --repo . --name quality-report --file sample.txt`. `tfy custom capture` runs the command from `--repo` and stores a merged stdout+stderr fixture view; use `import-fixture` when stream separation or secret-bearing argv would matter. `tfy custom verify` runs strict validation, preview, and custom-vs-built-in comparison evidence, and it fails unless the fixture actually exercises a `user_toml` rule with a concrete `rule_id`. It fails by default when `~/.config/tfy/commands.toml` exists; pass `--allow-legacy-global-rules` only to explicitly record that legacy/manual influence. If a built-in override is intentionally larger than the built-in/default result, `--accept-larger-than-built-in` must be passed to `verify` and is recorded in trust metadata.

Low-level `tfy rules validate`, `tfy rules preview`, `tfy rules compare-built-in`, and `tfy rules trust` remain expert primitives. Prefer `tfy custom` for user/agent-authored rules because it writes fixture metadata and v2 trust provenance.

### Safety metadata

`human_auto_safe`, `agent_safe`, and `interactive_risk` are metadata. They do not make a command officially supported and do not override built-in safety decisions.

Allowed `interactive_risk` values are `none`, `possible`, and `unknown`.

## Invalid configs and diagnostics

Runtime surfaces are non-strict by default: invalid user rule entries are skipped when possible, diagnostics are emitted, and valid entries from the same file can still apply. Whole-file TOML syntax errors skip that file and command execution continues through built-in/generic behavior.

Strict parser/test APIs fail on invalid TOML, invalid regex, duplicate ids, unsupported fields, unsafe limits, invalid sections, invalid counters, invalid captures, invalid severity, or unsupported schema versions.

Diagnostic codes include:

- `repo_rules_untrusted`
- `repo_rules_hash_mismatch`
- `user_rules_invalid_toml`
- `user_rules_invalid_regex`
- `user_rules_duplicate_id`
- `user_rules_unsafe_limit`
- `user_rules_unsupported_field`
- `user_rules_unsupported_schema_version`
- `user_rules_invalid_section`
- `user_rules_invalid_counter`
- `user_rules_invalid_capture`
- `user_rules_invalid_severity`
- `user_rules_invalid_extract`
- `user_rules_invalid_metric`
- `user_rules_invalid_group`
- `user_rules_invalid_override`
- `user_rule_shadowed_by_builtin`
- `user_rule_override_family_mismatch`
- `user_rule_overrode_builtin`

Gateway and ledger metadata include `rule_id`, `strategy_source_kind`, and `command_rule_diagnostics` when applicable. Adapter/MCP reports aggregate `rule_counts`, `strategy_source_counts`, and `command_rule_diagnostic_counts`.

## Authoring rules with an agent

Humans should not need to hand-write complex rules. Use the authoring workflow in `docs/CUSTOM_COMMAND_RULE_AUTHORING.md`, the `tfy rules ...` harness, or the project skill `.codex/skills/tfy-command-rule-author/SKILL.md`.

Authoring must be validation-gated:

1. capture or inspect representative raw evidence;
2. draft conservative declarative TOML;
3. run strict validation / parser tests;
4. preview through a TFY gateway;
5. verify redaction, no-negative behavior, diagnostics, and raw ref recovery;
6. update repo trust only after reviewing the final rule bytes.

## Human mode note

This feature does not by itself make `tfy start --human` intercept every command. Current managed human shell interception is still limited by the shell integration's wrapper behavior. User TOML rules work through TFY gateway paths such as `tfy tool-gateway`, `tfy shell`, and `tfy human run` when those paths receive the command argv.
