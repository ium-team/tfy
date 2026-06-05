# TFY Token-Saving Architecture

## Definition

TFY is a whole-workflow token-saving service for AI coding. It manages how code, docs, command output, task state, patches, repeated prompts, and audit evidence are represented to AI agents.

The architecture is final-state from the start: token-saving methods are not staged buckets. They are registry entries that can be added, evaluated, and composed over time.

## Representation ladder

Every artifact can move through a ladder of representations:

| Level | Representation | Use when |
|---|---|---|
| L0 | raw/full | audit, high risk, low confidence, human review |
| L1 | fingerprint/ref | artifact is already known or unchanged |
| L2 | summary | success/no-action or orientation context |
| L3 | semantic skeleton | structure matters more than body text |
| L4 | selected compact detail | a bounded scope/output is relevant |
| L5 | related neighborhood | local context is insufficient |
| L6 | delta/change-only | prior state exists and only changes matter |
| L7 | adapter-specific layout | a provider cache/session feature can reduce repeated input cost |

Fallback can move upward toward raw/full at any time.

## Method registry schema

Each token-saving method should be documented with this shape:

```yaml
name: semantic-skeleton-mode
targetArtifact: code | docs | command-output | task-state | patch | provider-context | security-sensitive-data
mechanism: what is shortened, omitted, referenced, cached, or transformed
prerequisites: parser, hash store, raw ref, provider adapter, task ledger, etc.
coreStatus: core | specialized-core | optional-adapter
savingsMetric: gross tokens, net tokens, cache hit, repeated-output elision, etc.
performanceCost: parse, IO, graph walk, hash, storage, provider call, etc.
correctnessRisk: missing context, stale ref, evidence loss, semantic drift, etc.
fallbackTrigger: unresolved symbol, error output, changed hash, low confidence, user request, etc.
validationGate: tests/evals required before claiming release readiness
```

The schema is more important than the initial list. Future methods join TFY by filling this contract.

## Initial method registry

The first public registry is already shaped like the final extensibility contract. The table is compact, but every row includes the required release fields: target, mechanism, status, savings metric, cost, risk, fallback, and gate.

| Method | Target | Mechanism | Status | Savings metric | Cost | Main risk | Fallback trigger | Validation gate |
|---|---|---|---|---|---|---|---|---|
| Semantic index/skeletons | code/docs | names, signatures, imports, public API before bodies | core | bodies avoided | parse/index | incomplete model | unresolved/low confidence | skeleton vs full baseline |
| Compact code/symbol maps | code | safe shortening + deterministic maps | core | compact body net tokens | tokenize/map | comprehension loss | unmapped/failed restore | roundtrip + task baseline |
| Content-addressed refs | all artifacts | replace repeated content with hash/ref | core | repeated tokens avoided | hash/IO/store | stale ref | hash changed/missing ref | ref recovery test |
| Incremental deltas | artifact state | send changed artifacts only | core | full state minus delta | snapshot diff | state divergence | invalidated snapshot | baseline/final state replay |
| Compact schemas | machine payloads | short keys with schema debug form | core | payload key tokens | schema versioning | debug mismatch | unknown schema/version | decode/roundtrip test |
| Dependency-neighborhood slicing | code context | selected scope plus graph-local relations | core | full repo avoided | graph/index | missed related code | diagnostics outside slice | missed-context eval |
| Patch/edit-script output | AI output | structured edits instead of full files | core | output tokens avoided | apply/validate | ambiguous patch | apply failure/context mismatch | patch restore/diff test |
| Boilerplate/generated suppression | repo artifacts | metadata/fingerprint for repeated/generated regions | core | omitted repeated regions | classifier | hiding target artifact | user/task targets artifact | expansion and classifier eval |
| Task-state compaction | workflow state | compact ledger instead of raw conversation | core | history tokens avoided | ledger upkeep | lost intent | unresolved decision/low confidence | transcript-vs-ledger audit |
| Tool feedback compression | command output | risk summary + raw ref | core | raw log tokens avoided | classify/store | missed evidence | error/warning/unknown | fixture + raw recovery |
| Output fingerprinting | repeated commands | status/hash/ref for unchanged output | core | repeated output avoided | hash/store | false unchanged | hash changed/request raw | repeat-output replay |
| Error clustering | diagnostics | group duplicate failures by signature | core | duplicate lines avoided | clustering | merged distinct errors | cluster confidence low | representative + raw refs |
| Retrieval budget planner | context selection | choose cheapest safe ladder level | core | budget adherence/net savings | policy calc | under-fetching | unresolved/failed task | full-context comparison |
| Adaptive compactness | all representations | select light/symbol/skeleton/full by risk | core | risk-adjusted net savings | policy metrics | wrong compactness | degradation/failure | compactness A/B eval |
| Local memoization | parser/hash/eval work | reuse cached parse/token/classification | core | recomputation avoided | invalidation | stale cache | mtime/size/hash mismatch | cache invalidation test |
| Test/CI selective evidence | verification output | preserve failures, elide passing noise | specialized-core | passing-log tokens avoided | parser/regex | hidden flaky/slow signal | failures/warnings/low confidence | CI fixture replay |
| Redaction with local refs | sensitive/long values | placeholder/ref for secrets/blobs | specialized-core | long/sensitive tokens avoided | detector/store | recovery/security mistake | authorization/low confidence | redaction recovery tests |
| Provider cache layouts | provider context | stable prefix/cache-aware layout | optional-adapter | cached-token benefit | provider bookkeeping | vendor drift/lock-in | cache miss/provider unsupported | neutral parity + usage evidence |

## Core vs adapter boundary

TFY core owns deterministic, auditable, agent-neutral behavior:

- refs and raw/full fallback
- local method registry
- token/correctness/performance metrics
- representation selection policy
- command evidence contracts
- restoration and validation contracts

Optional adapters may optimize provider-specific behavior:

- prompt/context caching layout
- cache breakpoints or TTL choices
- cached-input accounting
- model/session memory hints
- provider-specific token counters

Adapters must never be required for correctness.

## Safety contract

TFY may save tokens aggressively only when recovery is cheap and risk is low. It must preserve or recover raw/full evidence for:

- non-zero command exits
- compiler/test failures
- security, permission, auth, rate-limit, or review-blocker output
- dirty Git state, conflicts, rejected pushes, failed checks
- generated/raw artifacts that are the target of the task
- code changes involving public API, reflection, macros, serialization, dynamic dispatch, or unresolved references

## Performance model

Token savings can create local cost. Heavy methods must be amortized:

- parser/index work should be cached and incremental
- hashes should be content-addressed and invalidated by mtime/size/hash
- graph slicing should start conservative and expand on diagnostics
- provider adapters should report cache hit/miss and never hide non-cached token cost
- evaluation must report both token savings and local overhead

## Release claim rule

A method is release-ready only when it reports:

1. net token savings after maps, refs, expansions, and raw requests
2. task correctness compared with full-context baseline
3. fallback frequency
4. missed-needed-context and missed-evidence failures
5. local performance overhead
6. recovery path for low-confidence or high-risk cases
