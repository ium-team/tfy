# TFY Agent-Neutral Protocol

TFY is exposed as a CLI so any AI agent, shell wrapper, editor integration, or tool runner can call it without depending on one vendor-specific agent format.

## Commands

- `tfy index <path>`: returns semantic scope names first.
- `tfy expand <path> <scope-id-or-unique-name> [--compactness light|symbol]`: returns selected compact body, local symbol map, and token/char metrics. Duplicate bare names are rejected; use the scope `id` from `tfy index` when ambiguous.
- `tfy full <path> <scope>`: returns full source for fallback.
- `tfy decide-context`: reads payload JSON and returns `selected`, `related`, or `full`.
- `tfy restore`: reads compact payload JSON and deterministically restores readable code.
- `tfy run -- <command...>`: executes a command and prints risk-aware compact feedback.
- `tfy raw <raw_ref>`: returns full or ranged raw command output.
- `tfy languages`: lists language adapters.

## Agent Contract

1. Ask `index` before requesting full code.
2. Use `expand` for selected scopes; if a name is ambiguous, retry with the exact scope `id`.
3. If diagnostics or unmapped compact symbols appear, call `decide-context` and then `full` if needed.
4. Emit compact patches only with the symbol map that TFY provided.
5. Use `restore` before presenting code to humans or writing project-ready output. Restoration is token-aware for Python and rejects unmapped compact symbols instead of guessing.
6. Use `run` for bounded command feedback and `raw` when the summary is insufficient. Raw refs are validated, append-only records.

All data-bearing commands return JSON except `run` compact summaries and `raw` raw text recovery. `expand` payloads include `compactness` so `restore` can keep strict unmapped-symbol checks for `symbol` mode while treating `light` mode as symbol-free source normalization.

## Hardened Contracts

- Scope names are presentation hints; exact scope IDs are the selection contract when duplicate names exist.
- Python compaction preserves indentation/newlines and string literal contents; symbol shortening provides savings without flattening Python blocks.
- Non-Python compaction/restoration preserves string and template literal contents instead of formatting inside them.
- Compact symbol restoration does not rewrite Python string literals/comments or attribute names.
- Raw output references are validated and include unique append-only identity material to avoid overwriting prior evidence.
- Evaluation reports token savings and a quality gate; savings alone is not a release gate.
