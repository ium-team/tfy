# Decision: RTK filter provenance for TFY command support

## Status

Accepted for the RTK-overlap command-support work unit.

## Evidence reviewed

- Reference repository: `/tmp/rtk-ai-rtk`
- Reference commit: `d8c550eefba41e112bd174d58844a803db6e432f`
- License evidence: RTK repository `LICENSE`, `Cargo.toml`, and README identify Apache License 2.0.
- Inventory evidence: RTK exposes Rust command modules plus built-in TOML filters across Git, language toolchains, cloud/infra, and system commands.

## Decision

TFY may use RTK as a **behavioral reference and coverage map** for command-output families, but TFY must not silently copy RTK filter contents into TFY.

For this work unit, the allowed path is:

1. Record RTK command/filter names and source locations in TFY's support matrix.
2. Reimplement behavior in TFY-owned Rust strategies or TFY-owned built-in filter definitions.
3. Preserve TFY invariants for every implementation: raw bytes first, redaction before model visibility, no-negative selector, recoverable raw refs, and plain-text default output.
4. If any RTK source text, exact regex, TOML rule, or code is copied or translated closely enough to be derivative, add explicit attribution/notice review before merging that implementation.

## Consequences

- RTK breadth is a target, not a product identity. TFY documentation must describe this as RTK-overlap coverage or RTK-informed support, not as an RTK clone.
- Public RTK comparison superiority claims remain blocked until the command support matrix and benchmark manifest both prove the specific rows being claimed.
- Built-in TFY filters can land only with TFY-owned fixtures and claim-gate metadata.
- User/project-provided filters are out of scope for this decision unless a separate trust/provenance gate is added.

## Rejected alternatives

- **Direct compatibility/import layer:** rejected for now because it creates provenance, NOTICE, and semantics drift risk before TFY has its own no-negative/evidence gate around each filter.
- **Untracked manual copying:** rejected because future maintainers would not know which behavior came from RTK or whether Apache-2.0 notice obligations apply.
- **LLM-generated arbitrary summaries:** rejected for the deterministic command-output core because it is harder to fixture-test and easier to overclaim.
