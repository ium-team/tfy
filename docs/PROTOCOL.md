# TFY Agent-Neutral Protocol

TFY exposes an agent-neutral protocol so any AI agent, editor, shell wrapper, or orchestration layer can use the same token-saving service.

## Current implemented CLI surface

## Final protocol capabilities

The final architecture extends the protocol around registry methods:

- `registry` — list available token-saving methods and their gates.
- `ref put/get` — content-addressed artifact references for stable/repeated artifacts; command raw refs remain opaque append-only evidence handles.
- `delta` — changed-artifact views since a known snapshot.
- `skeleton` — semantic structure without full bodies.
- `budget` — choose cheapest sufficient representation for a token budget.
- `ledger` — compact task/conversation state.
- `adapter` — optional provider/model layout and usage accounting.

Names may change during implementation, but the concepts are part of the first public architecture.

## Agent contract

1. Start with registry/index/skeleton before requesting full context.
2. Use selected compact views only with TFY-provided maps/refs.
3. Expand related/full/raw context when diagnostics, ambiguity, or risk appears.
4. Emit patch/edit-script outputs when possible; use restoration before human/project output.
5. Preserve raw refs for command, CI, Git/GitHub, security, and review evidence.
6. Treat provider adapters as optional optimizations; the neutral protocol remains sufficient.
7. Do not claim token savings without evaluation-gate evidence.

## Data contract

Machine payloads may use compact schema dictionaries for token efficiency. Every compact schema must have:

- schema version
- human-readable debug form
- stable field mapping
- compatibility/fallback behavior

## Hardened contracts

- Scope IDs, not display names, are the snapshot-stable exact selection contract for the current indexed snapshot.
- Command raw refs are validated, append-only, opaque evidence handles; they are not a stable content-addressing contract.
- Python layout/string literals are preserved when needed for correctness.
- Non-Python compaction preserves strings/template literals.
- Restoration rejects unmapped compact symbols.
- Provider adapter usage must report hit/miss/usage evidence when available.


## Model-visible Tool Gateway protocol

The default Tool Gateway contract is text-first:

```sh
tfy tool-gateway -- <ordinary command>
tfy shell -- <ordinary command>   # TFY Shell Gateway wrapper
```

`tfy shell <ordinary command>` without the separator is deliberately outside this Tool Gateway contract: it is raw passthrough convenience for humans, with native stdout/stderr/exit code and no TFY summary, raw store, or ledger artifact.

Execution flow:

1. Run the original argv.
2. Store exact stdout/stderr bytes locally behind an opaque `raw_ref`.
3. Build a redacted public raw candidate.
4. Build a redacted compact summary candidate.
5. Return the summary only when it is strictly smaller than the public raw candidate.
6. Otherwise return the redacted raw text unchanged.

This means small outputs such as `ok` stay `ok`; TFY does not add JSON, envelope keys, or `raw_ref` overhead when doing so would increase model-visible tokens. Summarized, truncated, suppressed, or unsafe output includes a recovery hint/ref. Redaction happens before any pass-through reaches the model.

## Middleware protocol boundary

The CLI is both a manual debug surface and the contract used by adapters. Production usage should prefer gateway names where available:

- Tool Gateway: `tfy tool-gateway -- <ordinary command>` wraps command execution and returns the smallest safe model-visible text chosen by the net-savings gate.
- Context Gateway: `tfy context-gateway` runtime envelope over `index`, `expand`, `full`, and `decide-context`; automatic external hooks remain adapter work.
- Output Gateway: `tfy output-gateway` structured preview/validate API over `restore`; `--apply` supports local single-file selected-scope replacement only when a content-addressed `ApplyProof` validates the exact path, byte range, source hash, compactness, language, and parser confidence. Parent event ids alone are not authoritative; apply payloads must also carry the captured source context_ref and base compact code so TFY can bind the proposed edit to the proven context.
- State Gateway: `tfy state-append` / `tfy state-project` ledger/ref API fed by gateway events.

Savings reporting separates exact byte accounting from token estimates. Byte fields are authoritative (`raw_bytes`, `model_bytes`, `saved_bytes`); token fields are a conservative `ceil(bytes/4)` proxy unless a future adapter records tokenizer-specific counts. Raw evidence is local by default under `.tfy/raw` or configured `--raw-dir` plus gateway ledgers; TFY stores it for audit/recovery and does not upload it.

## Runtime envelope protocol

Structured runtime interception uses the `tfy-runtime` crate. Runtime envelopes are **debug/adapter/internal only** unless an integration explicitly needs machine-readable transport outside the model prompt. Adapters must not forward envelopes, ledger JSON, `--json`, or `--jsonl` output into model context by default.

For Tool Gateway envelopes, `payload.model_text` is the selected model-visible text and `payload.rendering_kind` describes how it was produced (`pass_through`, `summary`, or `suppressed`). The legacy-compatible `payload.summary` mirrors `model_text` for now and must not be interpreted as always being a synthesized summary.

Every runtime-facing request, response, and event is wrapped in `RuntimeEnvelope<T>` with:

- `protocol_version`
- `adapter_kind` and `adapter_version`
- `supported_gateways`
- `authority_mode`
- `session_id`, `request_id`, `trace_id`, optional `turn_id`, optional `parent_event_id`
- `provenance`
- `origin` (`kind`, `host`, `invocation`, `intercepted`, `user_shell_mutated`)
- `policy`
- gateway-specific `payload`

Current runtime-facing CLI surfaces:

## Product lifecycle protocol

`tfy init`, `tfy doctor`, `tfy smoke`, and `tfy gain` are product-facing wrappers around the lower-level protocol surfaces:

## Adapter v1 protocol

`tfy adapter` is the first implemented automatic-interception surface. It is automatic only after a host agent/runtime configures its command execution path to call the adapter wrapper.

`tfy hook` is the official-host-hook router for Codex and Claude Code plus a test-shim equivalence surface. For PreToolUse, it rewrites Bash input to a TFY command route and does not execute the pending command inside the hook process. It fails closed with `TFY_HOOK_DISABLE=1`; setup remains `configured_unverified` until host-bound e2e route evidence proves the host ran the rewritten TFY command and preserved raw-first/no-negative behavior.

Supported today:

- `generic-shell`: supported command-boundary interception through `tfy adapter run` or a dry-run/generated shell shim.

Not yet claimed as supported automatic interception:

Adapter v1 keeps the same model-visible contract as Tool Gateway: stdout is plain selected text by default, JSON is explicit debug/internal only, raw bytes are stored locally first, and session reports measure baseline raw vs model-visible output.

Adapter reports use `raw_bytes` and `model_bytes` as the public size contract. Legacy runtime ledger fields such as `raw_chars` / `model_chars` are compatibility-only and are not emitted by `tfy adapter report`.

## Release tier evidence reducer

`tfy launch-report --json` includes four evidence-gated release tiers:

`--release-evidence <json>` accepts packaging/docs/review/CI proof fields, but each boolean must be backed by existing artifact paths (`cargo_install_binary`, `release_binary`, archive/checksum artifacts, docs/release-notes artifacts, review/CI artifacts) before it is trusted. `benchmark_manifest_generated` additionally requires an attached benchmark manifest with `status=pass`, no-negative savings, positive savings, and raw refs for every scenario. This reducer is evidence reporting only; it does not promote provider/API proxying, private Codex hooks, universal terminal interception, editor auto-hooks, or named-host launch support without matching route evidence.

## Raw evidence lifecycle

```sh
tfy raw --list --json
tfy raw <raw_ref> --inspect --json
tfy raw <raw_ref> --export ./raw-export
tfy raw --prune --dry-run --json
tfy raw --prune --apply --json
```

Prune is dry-run unless `--apply` is supplied. These commands manage raw evidence JSON files; gateway ledgers remain explicit local files.
