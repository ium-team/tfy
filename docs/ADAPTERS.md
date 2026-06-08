# TFY Optional Provider and Model Adapters

## Purpose

TFY core is agent-neutral. Provider/model-specific optimization belongs in optional adapters because cache semantics, pricing, token thresholds, TTLs, and usage fields change across vendors.

Adapters are performance/cost optimizers, not correctness foundations.

## Adapter responsibilities

An adapter may:

- arrange stable prompt prefixes for cache reuse
- split static instructions, repository summaries, task ledgers, and volatile user turns
- expose provider-specific token counters or cached-token usage fields
- choose cache breakpoints or TTL when the provider supports them
- report cache hit/miss and estimated cost impact
- degrade to TFY core representations when the provider feature is unavailable

An adapter must not:

- make TFY require one vendor
- remove raw/full fallback
- hide evidence because it was cached or not cached
- claim savings without usage evidence
- change the semantic content of prompts to fit a cache

## Cache-aware prompt layout

TFY should divide agent context into stability bands:

1. **Stable prefix:** system instructions, method registry, project summary, stable docs refs.
2. **Semi-stable context:** current task ledger, known decisions, selected artifact refs.
3. **Volatile context:** current user request, newest command output, changed scopes, fresh diagnostics.
4. **Expansion payloads:** raw/full/related context requested after uncertainty appears.

Adapters can map those bands to provider-specific cache behavior.

## Provider notes

These notes are intentionally non-binding. Use official provider docs before implementing an adapter.

- OpenAI prompt caching reports cached input through usage fields such as cached prompt tokens and benefits stable repeated prompt prefixes. See <https://platform.openai.com/docs/guides/prompt-caching>.
- Anthropic prompt caching supports cacheable prompt prefixes/breakpoints and is useful for repetitive tasks and long conversations. See <https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching>.
- Gemini context caching supports implicit and explicit caching with model-specific thresholds and TTL/cost mechanics. See <https://ai.google.dev/gemini-api/docs/caching>.

## Adapter registry entry

Provider adapters use the same method schema:

```yaml
name: openai-prompt-cache-layout
targetArtifact: provider-context
coreStatus: optional-adapter
mechanism: stable prefix layout and cached-token accounting
prerequisites: provider API support and usage metadata
savingsMetric: cached input tokens / total input tokens
performanceCost: prompt layout bookkeeping and provider-specific token counting
correctnessRisk: cache miss, stale static prefix, accidental vendor dependency
fallbackTrigger: missing usage metadata, low cache hit, provider unsupported, changed static context
validationGate: same answer quality as neutral layout plus measurable cached-token benefit
```
