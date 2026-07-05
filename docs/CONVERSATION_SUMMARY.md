# TFY Conversation Summary

## Supersession note

This document preserves discussion history. The final product definition is now the whole-workflow token-saving architecture in `TOKEN_SAVING_ARCHITECTURE.md`, `PRD.md`, and `../README.md`.

## Original idea

The starting idea was to reduce AI token usage by letting AI operate on compact code:

- fewer newlines/indentation where safe
- short symbols such as `f1`, `a`, `b`
- deterministic maps back to semantic names
- readable restoration before human/project output

## Key evolution

The concept expanded beyond code:

1. AI should see semantic names before compact bodies.
2. Missing context should trigger related/full fallback.
3. Command/tool output also wastes tokens and needs risk-aware compression.
4. Git/GitHub and CI need explicit evidence contracts.
5. Provider/model prompt caching can help but belongs in optional adapters.
6. TFY should save tokens in every part of the AI coding workflow.
7. Future methods should be expected and added through a method registry.

## Final decision

TFY is a whole-workflow token-saving service with:

- representation ladder
- method registry
- agent-neutral protocol
- deterministic restoration and raw/full fallback
- optional provider adapters
- net savings and correctness evaluation gates

Older narrow framings such as “code minifier” or “command-output compressor” are superseded.
