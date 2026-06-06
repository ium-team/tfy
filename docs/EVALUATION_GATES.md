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

## Documentation gate

Docs are release-ready when they:

- describe final architecture directly, not staged phases
- define future methods through the registry schema
- keep provider features optional
- distinguish implemented prototype behavior from target architecture
- link specialized docs back to the canonical architecture
- avoid framing TFY as only a code minifier or command-output filter
