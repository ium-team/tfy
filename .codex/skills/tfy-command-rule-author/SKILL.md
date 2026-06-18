---
name: tfy-command-rule-author
description: Create, validate, and document TFY custom command rules without breaking raw-first/no-negative/redaction/trust invariants.
---

# TFY Command Rule Author

Use this skill when the user asks to create or improve a TFY custom command summary rule for a project or personal command.

## Required reading

Before editing rules, read:

- `docs/COMMAND_RULES.md`
- `docs/CUSTOM_COMMAND_RULE_AUTHORING.md`

## Operating rules

- Start with bare `tfy custom` when setting up authoring; choose repo scope for the active provenance-backed harness. Treat user-global `~/.config/tfy/commands.toml` as legacy/manual compatibility until TFY has a provenance-backed global custom flow.
- Use safe `command.id` values: ASCII letters, digits, `_`, or `-`, length 1..64.
- Prefer `match.argv_prefix`; use `match.command_regex` only when argv prefix is insufficient.
- Use `schema_version = 2` when sections/counters/captures/severity are useful.
- Use `schema_version = 3` when bounded JSON/NDJSON/KV/table extracts, metrics, or groups are useful.
- Never add arbitrary script execution or plugin hooks to TOML.
- Do not claim custom rules are official command support.
- Do not override built-in TFY summaries unless the user explicitly asks for replacement; then use v3 `[command.override]` with exact family matching and `compare-built-in` evidence. `schema_version = 3` alone never changes precedence.
- Preserve raw-first, no-negative, redaction, trust, and plain text defaults.

## Workflow

1. Identify the command and desired scope.
2. Capture representative output through TFY when safe, or inspect supplied output.
3. Draft conservative TOML:
   - strip noise;
   - create sections for errors/warnings/failures;
   - add counters for important patterns;
   - add captures for file/test/package identifiers;
   - add severity only to increase caution, never to downgrade failures;
   - add `[command.override] built_in = true` only for explicit built-in replacement requests, with exact `family` and a short reason;
   - put specific override rules before broader custom rules because first match wins.
4. Use bare `tfy custom` to initialize repo scope, then `tfy custom capture` or `tfy custom import-fixture`, and `tfy custom prompt` before editing rules. Use `import-fixture` instead of `capture` for secret-bearing commands or when stdout/stderr stream separation matters.
5. Validate with `tfy custom verify --repo . --name <fixture> --json`; it must record validate/preview/compare evidence, redaction, raw_ref, no-negative behavior, override comparison, and a concrete `user_toml` rule id. If a global rule file exists, do not pass `--allow-legacy-global-rules` unless the user explicitly accepts that legacy/manual influence. If a built-in replacement is larger, do not pass `--accept-larger-than-built-in` unless the user explicitly accepts the local tradeoff.
6. For repo-local rules, update `.tfy/trust.json` only with `tfy custom trust --repo . --name <fixture>` after reviewing final `.tfy/commands.toml` and `.tfy/custom/<fixture>.verify.json`.
7. Report changed files, rule id, validation evidence, trust status, and remaining limitations.

## Output contract

Final response must include:

- rule location;
- rule id(s);
- validation harness commands run;
- whether repo trust was updated;
- explicit note that the rule is local/custom support.
