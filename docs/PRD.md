# PRD: TFY Whole-Workflow Token-Saving Service

## Goal

Build TFY as a release-ready token-saving service for AI coding work. TFY should reduce tokens across the full loop: code, docs, tool feedback, CI, Git/GitHub work, task state, patches, repeated provider context, and audit recovery.

The product is documented as the final architecture now. Future methods are expected and are added through the method registry.

## Product requirements

### Whole-workflow coverage

TFY must support token-saving methods for:

- source code and symbols
- project documentation and planning context
- command/tool output
- test/CI/Git/GitHub evidence
- AI patch/output representation
- conversation/task state
- repeated provider/model context through optional adapters
- raw/full fallback and audit recovery

### Method registry

Every method must declare:

- target artifact type
- mechanism
- prerequisites
- core vs optional-adapter status
- savings metric
- performance cost
- correctness/evidence risk
- fallback trigger
- validation gate

### Agent-neutral protocol

TFY core must work through CLI/JSON/text surfaces that any agent can call. Provider-specific features must not be required for correctness.

### Safety and recovery

TFY must preserve raw/full or deterministic recovery for high-risk artifacts and outputs. It must not hide decision-critical evidence to save tokens.

### Evaluation

Release claims must report net token savings, correctness against baseline, fallback frequency, missed-needed-context failures, missed-evidence failures, restoration/audit success, and local overhead.

Command raw refs are opaque append-only local evidence handles, not stable content-addressed IDs.

## Initial method registry

This is not a loose feature list. It is the first compact registry view; the full schema is defined in `TOKEN_SAVING_ARCHITECTURE.md`. Each row states target, status, main mechanism, and release gate.

| Method | Target | Status | Mechanism | Release gate |
|---|---|---|---|---|
| semantic index and skeletons | code/docs | core | structure before body text | skeleton vs full baseline |
| compact code and symbol maps | code | core | safe shortening + deterministic maps | roundtrip + task baseline |
| content-addressed artifact refs | repeated non-command artifacts | core | hash/ref instead of repeated content | ref recovery test |
| incremental deltas | artifact state | core | changed artifacts only | snapshot replay |
| compact schemas | machine payloads | core | short keys with debug schema | decode/roundtrip test |
| dependency-neighborhood slicing | code context | core | graph/locality-selected context | missed-context eval |
| patch/edit-script outputs | AI output | core | structured edits instead of full files | patch restore/diff test |
| boilerplate/generated suppression | repo artifacts | core | metadata/fingerprint for repeated regions | classifier + expansion eval |
| task-state compaction | conversation/workflow | core | ledger instead of raw transcript | transcript-vs-ledger audit |
| risk-aware tool feedback | command output | core | summary + raw ref by risk | fixture + raw recovery |
| output fingerprinting | repeated commands | core | status/hash/ref for unchanged output | repeat-output replay |
| error clustering | diagnostics | core | group duplicate failures | representative + raw refs |
| retrieval budget planner | context selection | core | cheapest safe ladder level | full-context comparison |
| adaptive compactness | all representations | core | risk-based light/symbol/skeleton/full | compactness A/B eval |
| local memoization | parser/hash/eval work | core | reuse validated cache | invalidation test |
| Test/CI selective evidence | verification output | specialized-core | preserve failures, elide passing noise | CI fixture replay |
| redaction with local refs | sensitive/long values | specialized-core | placeholder/ref for recoverable redaction | redaction recovery test |
| provider cache layouts | provider prompts | optional-adapter | stable prefix/cache-aware layout | neutral parity + usage evidence |

## Non-goals

- Do not require a single AI vendor or agent runtime.
- Do not treat provider cache behavior as core correctness.
- Do not accept irreversible lossy transforms where audit/restoration matters.
- Do not hide errors, review blockers, dirty repo state, or failed checks.
- Do not claim a method is release-ready without evaluation evidence.
- Do not frame TFY as only code minification, only command-output filtering, or a phased roadmap.

## Acceptance criteria

- Canonical docs describe the final architecture and method registry.
- Focused docs link back to the architecture spine.
- Every initial method family has a target, mechanism, risk, fallback, and evaluation gate.
- Provider adapters are optional and isolated.
- Implementation status is truthful: target architecture is not overclaimed as fully implemented.
- Test/evaluation docs define net savings and correctness gates.

## ADR

### Decision

Adopt the final whole-workflow token-saving architecture with an extensible method registry.

### Drivers

- The user wants TFY to save tokens in every part of AI work.
- Future token-saving methods should be expected, not bolted on.
- Safety and correctness require raw/full fallback and evaluation.
- Provider cache features are useful but unstable and vendor-specific.

### Alternatives rejected

- **Appendix-only method list:** rejected because it preserves the older narrow product center.
- **Phased roadmap docs:** rejected because the requested docs should be final-state now.
- **RTK-style command filter only:** rejected because TFY covers more than command output.
- **Provider-cache-centric product:** rejected because it would break the agent-neutral core.

### Consequences

- Docs and future code should organize around the registry schema.
- Specialized docs stay useful but must not redefine TFY narrowly.
- Future implementation planning can derive work from registry entries and evaluation gates.


## Agent middleware requirements

TFY is primarily consumed by AI-agent runtimes. Public docs and future implementation must distinguish:

- implemented Rust CLI/core primitives,
- implemented Tool Gateway CLI entrypoint,
- planned automatic runtime adapters.

Required gateway boundaries:

1. Tool Gateway — ordinary command execution is proxied through TFY and returns compact summary plus raw_ref.
2. Context Gateway — runtime context requests choose skeleton/compact/related/full representations.
3. Output Gateway — structured compact code/patch outputs are restored and validated before apply.
4. State Gateway — task/conversation state is compacted into an event-fed ledger.

The representation ladder and method registry remain the internal source of truth; gateways are integration boundaries.
