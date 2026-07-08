# TFY AI Agent Harness

This document expands the repo-root `AGENTS.md` for AI-agent-assisted development. `AGENTS.md` is the authoritative instruction surface; this file is the long-form reference.

## Product invariant

TFY saves model-visible tokens at agent I/O boundaries while preserving recoverable local evidence.

Never weaken these invariants:

## Code map

## Command routing for agents

Use targeted checks while editing, then run the full gate before claiming done:

```bash
./scripts/verify.sh
```

Useful focused checks:

## Review checklist

Before finalizing a change, verify:
