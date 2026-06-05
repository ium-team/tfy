# TFY Test and Evaluation Specification

## Purpose

TFY tests validate the final whole-workflow token-saving architecture. Tests must prove not only that output is shorter, but that correctness, evidence, restoration, and fallback behavior survive compression.

## Universal method tests

Every registry method should have tests or evaluation evidence for:

- gross token savings
- net token savings after refs/maps/expansions/raw requests
- fallback behavior
- missed-needed-context failures
- missed-evidence failures
- local performance overhead
- restoration/audit recovery
- method metadata completeness

## Registry metadata tests

Each method entry must declare:

- target artifact type
- mechanism
- prerequisites
- core vs optional-adapter status
- savings metric
- performance cost
- correctness/evidence risk
- fallback trigger
- validation gate

## Code/context tests

- Build semantic index and skeletons from supported source files.
- Generate compact code with safe symbol maps.
- Restore no-op compact roundtrip to readable source.
- Reject ambiguous bare scopes and unmapped compact symbols.
- Preserve Python layout/string literals where required.
- Expand selected -> related -> full context when diagnostics require it.
- Measure compactness against full-context baseline.

## Tool feedback tests

- Failed commands preserve actionable evidence.
- Successful/no-action commands compress or fingerprint aggressively.
- Raw output is stored and requestable by raw ref.
- Around expansion recovers nearby evidence.
- Unknown output uses conservative summaries.
- Repeated unchanged output can be represented by status/hash/ref.
- Error clustering preserves representative evidence and raw recovery.

## Git/GitHub harness tests

`GIT_GITHUB_HARNESS.md` is the canonical specialized contract. Required fixture/replay classes:

- clean status
- dirty status
- diff/name-status/hunk output
- conflict output
- push rejection/auth failure
- CI/check failure
- PR requested changes/review comments
- API/rate-limit/403/404 error

Tests must not require live GitHub credentials, network access, or mutable remote writes unless live integration is explicitly in scope.

## Provider adapter tests

Provider adapters are optional. Adapter tests must prove:

- neutral TFY behavior works without the adapter
- stable/volatile context bands are laid out correctly
- provider usage metadata is captured when available
- cache hit/miss or cached-token benefit is reported honestly
- adapter miss/failure degrades to neutral protocol
- no provider-specific feature is required for correctness

## Security/redaction tests

- Secrets, long tokens, and sensitive blobs can be replaced with local refs.
- Redacted values can be recovered locally only when authorized.
- Redaction does not corrupt evidence or restoration.
- False positives/negatives are classified.

## Release gates

A release-ready method passes only when:

- net token savings is positive for at least one target workflow class
- compact workflow quality matches full-context baseline within tolerance
- raw/full fallback works for high-risk cases
- every missed-needed-context or missed-evidence failure is classified
- local overhead is acceptable
- documentation states implementation status truthfully

## Documentation consistency tests

- Docs describe the final architecture directly, not 1st/2nd/3rd phases.
- README and all public docs under `docs/*.md` align with the method registry.
- Provider/model cache behavior is optional adapter behavior.
- No public doc frames TFY as only a code minifier, only an RTK-style command-output filter, or a phased MVP.
- Specialized docs link back to canonical architecture.


## Agent middleware tests

Documentation gates:

- Docs must not imply automatic model input/output interception is implemented before a runtime adapter exists.
- Docs must include a gateway-to-registry crosswalk.
- Docs must preserve the Rust-only runtime invariant.

Tool Gateway gates:

- `tfy tool-gateway -- sh -c 'printf ok'` emits a compact summary and raw_ref.
- Non-zero exits preserve exit status and critical evidence.
- Credential-bearing URLs are redacted publicly and preserved only behind raw_ref.
- Unicode and tiny summary caps do not panic and preserve raw_ref.

Future Context/Output/State Gateway gates are defined in the ralplan handoff and should become executable tests when those adapters are implemented.
