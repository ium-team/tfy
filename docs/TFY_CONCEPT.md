# TFY Concept

## Definition

TFY is a **token-efficient AI work interface** for coding agents.

It is not only a code minifier and not only a command-output filter. It is a protocol and transformation layer that controls what the AI sees, how compactly it sees it, how it requests more context, and how compact outputs are restored into human/project-readable form.

## Product Identity

TFY should be independent from RTK.

- RTK is a reference for command-output compression.
- TFY owns the broader AI context loop.
- TFY should not be described as "RTK but renamed."
- TFY should be planned as a release-grade product from the start, not as a minimal-only MVP.
- TFY should support many languages through adapters and should be usable with many AI agent tools rather than being locked to one agent.

TFY's unique identity:

```text
semantic index
+ compact code
+ 1:1 symbol maps
+ on-demand expansion
+ compact tool feedback
+ raw/full fallback
+ readable restoration
```

## AI Context Flow

```text
1. TFY indexes project code into scopes.
2. AI first receives original semantic names of functions/scopes.
3. AI requests the scopes it wants to inspect.
4. TFY returns compact bodies and a local map.
5. AI writes compact code/patches.
6. TFY restores output into readable source code.
7. If uncertain, AI can request more code or full context.
```

## Tool Feedback Flow

```text
1. AI/agent runs a command.
2. TFY stores raw output.
3. TFY sends a risk-aware compressed view.
4. AI can request raw output by reference.
5. Errors stay evidence-rich; success/no-action output is compressed heavily.
```

## Design Principle

> Reduce tokens only while preserving decision quality. When uncertainty or risk rises, expand context or raw output instead of pretending compression is enough.

## Interface Principle

TFY should present a compact, agent-neutral work protocol:

```text
index/list -> request scope/raw -> compact body/summary + ref -> patch/action -> deterministic restoration or raw expansion
```

This lets different AI tools use TFY even if they have different prompt formats, shell integrations, or editor integrations.
