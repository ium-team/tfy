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

- Prefer repo-local `.tfy/commands.toml` for project commands and user-global `~/.config/tfy/commands.toml` for personal commands.
- Use safe `command.id` values: ASCII letters, digits, `_`, or `-`, length 1..64.
- Prefer `match.argv_prefix`; use `match.command_regex` only when argv prefix is insufficient.
- Use `schema_version = 2` when sections/counters/captures/severity are useful.
- Use `schema_version = 3` when bounded JSON/NDJSON/KV/table extracts, metrics, or groups are useful.
- Never add arbitrary script execution or plugin hooks to TOML.
- Do not claim custom rules are official command support.
- Do not attempt to override built-in TFY summaries; v3 is additive and does not change precedence.
- Preserve raw-first, no-negative, redaction, trust, and plain text defaults.

## Workflow

1. Identify the command and desired scope.
2. Capture representative output through TFY when safe, or inspect supplied output.
3. Draft conservative TOML:
   - strip noise;
   - create sections for errors/warnings/failures;
   - add counters for important patterns;
   - add captures for file/test/package identifiers;
   - add severity only to increase caution, never to downgrade failures.
4. Validate with `tfy rules validate --file ...` and preview with `tfy rules preview --file ... --cmd ... --arg ... --fixture ...`, using repeated `--arg` for exact argv.
5. Check that model-visible output is shorter when summarized, secrets are redacted, and `raw_ref` is present when summarized.
6. For repo-local rules, update `.tfy/trust.json` with `tfy rules trust --file .tfy/commands.toml --repo .` only after reviewing the final rule file bytes.
7. Report changed files, rule id, validation evidence, and remaining limitations.

## Output contract

Final response must include:

- rule location;
- rule id(s);
- validation harness commands run;
- whether repo trust was updated;
- explicit note that the rule is local/custom support.
