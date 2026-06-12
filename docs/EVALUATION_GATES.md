# TFY Evaluation Gates

## Purpose

TFY cannot claim a token-saving method is good merely because it emits fewer tokens. Release readiness requires net savings, preserved correctness, recoverability, and acceptable local overhead.

## Universal gate

Every method family must report:

| Metric | Requirement |
|---|---|
| Gross savings | raw tokens vs compact/view tokens |
| Net savings | savings after maps, refs, expansions, raw requests, and adapter overhead |
| Correctness | compact workflow succeeds against full-context baseline |
| Fallback frequency | how often related/full/raw expansion was needed |
| Missed-needed-context | count and classification of hidden required code/context |
| Missed-evidence | count and classification of hidden command/test/security/review evidence |
| Restoration/audit | raw/full or readable recovery works |
| Local overhead | parse/hash/graph/cache/storage/runtime cost |
| Adapter evidence | cache hit/miss/cost usage when provider adapters are used |

## Pass/fail rules

A method passes only if:

- median net savings is positive for at least one target workflow class
- compact workflow quality is within configured tolerance of the full-context baseline
- every missed-needed-context or missed-evidence failure is classified
- high-risk output has raw/full fallback
- local overhead does not erase practical benefit

A method fails if:

- apparent savings disappear after required expansions or raw requests
- compact mode repeatedly fails tasks that full context completes
- any evidence-hiding failure remains unclassified
- restoration accepts ambiguous or unmapped symbols
- provider adapter claims savings without provider usage evidence

## Evaluation workflow

1. Establish a full-context baseline.
2. Run TFY compact/referenced/adapted workflow on the same task.
3. Track representation path: skeleton, selected detail, related context, full fallback, raw fallback.
4. Record token counts and local overhead.
5. Compare task success and evidence preservation.
6. Classify failures and tune fallback policy.
7. Promote the method only when pass criteria hold.

## Evidence-tier claim reset gate

Launch and status claims must be generated from route-bound evidence tiers, not from setup text alone. The canonical promotion ladder is:

1. `config_snippet_available`
2. `config_written`
3. `host_launched`
4. `verified_host_mcp_invocation` or `verified_host_hook`
5. `route_evidence_recorded`
6. `savings_verified`
7. `launch_supported`

Provider/API prompt proxying, private Codex hook interception, universal terminal interception, and editor-internal auto hooks remain unsupported unless a separate official adapter, kill switch/uninstall path, and host e2e evidence are implemented and tested. MCP and hook shims may only route into the shared TFY gateways; they cannot independently promote claims.

## Command-output superiority benchmark gate

TFY may use RTK-style command-output reduction as an internal comparison lane, but public
superiority claims require a reproducible benchmark manifest. The manifest must record:

- fixture corpus path and command families covered
- TFY version/commit
- baseline mode and, when executable, RTK version/mode
- byte-count method and token estimate method
- correctness rubric for retained actionable failures
- redaction checks
- raw recovery checks
- tiny-output no-negative checks
- missed-evidence classifications
- latency/overhead measurements or explicit exceptions

Passing the internal command-output gate means:

- tiny outputs remain exact plain-text passthrough with no model-visible `raw_ref` overhead
- summarized noisy outputs are smaller than redacted public raw output
- every summarized/truncated/suppressed result stores raw evidence first and has recovery
- actionable failure evidence is retained: failing test/check name, first useful file/line
  location when present, error code/category when present, and the assertion/diagnostic headline
- redaction happens before public passthrough or summary text
- missed-evidence rate is no worse than baseline

Until that manifest exists and passes, docs may say TFY is **designed to benchmark against
RTK-overlapping command-output workflows**, not that TFY publicly surpasses RTK.

## Documentation gate

Docs are release-ready when they:

- describe final architecture directly, not staged phases
- define future methods through the registry schema
- keep provider features optional
- distinguish implemented prototype behavior from target architecture
- link specialized docs back to the canonical architecture
- avoid framing TFY as only a code minifier or command-output filter
