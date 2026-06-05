# RTK Reference Notes for TFY

## Supersession note

This is a reference document, not the TFY product definition. The canonical architecture is `TOKEN_SAVING_ARCHITECTURE.md`.

## Why RTK matters

RTK is useful evidence for one TFY method family: command/tool feedback compression. It demonstrates patterns that reduce LLM token waste from command output.

Useful RTK-like lessons:

- command proxying
- command-family filters
- raw output tee/recovery
- token savings tracking
- error-preserving summaries
- success/noise compression

## Boundary

RTK primarily optimizes:

```text
command output -> compact command output
```

TFY optimizes:

```text
whole AI coding workflow -> token-efficient, recoverable, evaluated representations
```

That includes code, docs, task state, command feedback, CI/Git/GitHub evidence, patches, provider context, refs, deltas, restoration, and evaluation gates.

## How TFY should use RTK lessons

RTK-inspired behavior belongs in the tool feedback registry entries:

- preserve errors and diagnostic evidence
- compress success/no-action output
- keep raw refs
- measure savings and missed signals
- use command-family policies

RTK should never narrow TFY into a command-output-only product.
