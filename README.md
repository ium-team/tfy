# TFY — Token-Efficient AI Work Interface

TFY is a standalone product concept. RTK is a reference for command-output compression, but TFY is not RTK and should not become an RTK clone.

TFY's goal is broader:

> Rebuild the whole AI coding work interface around token-efficient representations: compact code, semantic name indexes, deterministic symbol maps, on-demand context expansion, compact tool feedback, raw-output fallback, and human-readable restoration.

## Core Idea

AI does not always need human-friendly source code or full command logs. TFY gives AI the smallest representation that can still support correct work, while keeping deterministic escape hatches back to full meaning and raw evidence.

```text
Human code / tool output
-> TFY compact representation
-> AI reads and writes compact context
-> TFY expands or restores when needed
-> Human receives readable code / full evidence when needed
```

## Main Subsystems

1. **Semantic Code Index**
   - Shows original long function/scope names first.
   - AI chooses what it wants to inspect.

2. **Compact Code Representation**
   - AI-facing code is shortened where safe; brace-style languages can collapse whitespace more aggressively.
   - Python keeps indentation/newlines and string contents intact because correctness beats extra compression.
   - Non-Python compaction preserves string/template literal contents while safely compacting surrounding whitespace.
   - Variables/functions can become short symbols like `f1`, `a`, `b`.

3. **1:1 Symbol Map**
   - Compact symbols map deterministically to original semantic names.
   - Mapping is scope-aware and provided only when needed.

4. **On-Demand Code Expansion**
   - names -> signatures/structure -> selected compact bodies -> related context -> full context fallback.

5. **Readable Restoration**
   - AI outputs compact patches/code.
   - TFY maps symbols back to original names and formats output for humans/projects.

6. **Tool Feedback Layer**
   - Command/tool output is compressed by risk tier.
   - Errors/important evidence are preserved.
   - Success/no-action logs are compressed aggressively.
   - Raw output remains requestable.

## RTK Relationship

RTK is a strong reference for command-output filtering:

- command proxying
- per-command filters
- raw output tee/recovery
- token savings tracking
- hooks that rewrite shell commands

But TFY's identity is different:

| RTK | TFY |
|---|---|
| command-output token killer | AI work-context interface |
| filters command output | manages code, symbols, context, tool feedback, and restoration |
| command proxy/filter | context protocol + representation layer |
| mostly output compression | input/output/code/tool feedback loop |

## Documents

- `docs/TFY_CONCEPT.md` — product definition and architecture.
- `docs/CODE_COMPRESSION.md` — compact code, name maps, and restoration.
- `docs/TOOL_FEEDBACK.md` — command-output compression and raw-output fallback.
- `docs/RTK_REFERENCE.md` — what TFY should learn from RTK without copying its identity.
- `docs/PRD.md` — consolidated product requirements.
- `docs/TEST_SPEC.md` — test and evaluation specification.
- `docs/CONVERSATION_SUMMARY.md` — summary of the discussion so far.
- `archive/` — copied source artifacts from prior OMX interview/planning runs.

## Implementation Status

The standalone TFY implementation lives in this folder and is independent from the RTK clone under `.omx/tmp/rtk`.

### Install / Run Locally

```sh
cd tfy
PYTHONPATH=src python3 -m tfy.cli languages
```

### Agent-Neutral CLI

TFY exposes a CLI protocol any AI agent/tool can call:

```sh
# 1. names-first semantic index
PYTHONPATH=src python3 -m tfy.cli index examples/sample.py

# 2. selected compact body + local 1:1 map; payload includes compactness metadata
PYTHONPATH=src python3 -m tfy.cli expand examples/sample.py sample.py:calculate_total_price:1 --compactness symbol

# 3. full fallback when needed
PYTHONPATH=src python3 -m tfy.cli full examples/sample.py sample.py:calculate_total_price:1

# 4. decide selected/related/full fallback from compact payload + diagnostics
PYTHONPATH=src python3 -m tfy.cli decide-context --payload payload.json

# 5. restore compact output deterministically
PYTHONPATH=src python3 -m tfy.cli restore --payload compact-payload.json

# 6. run commands through risk-aware compact feedback, with raw refs
PYTHONPATH=src python3 -m tfy.cli run -- python3 -c 'print("ok")'
PYTHONPATH=src python3 -m tfy.cli raw cmdout_xxxxxxxxxxxx

# 7. measure raw vs compact token savings
PYTHONPATH=src python3 -m tfy.cli eval-code examples/sample.py sample.py:calculate_total_price:1
```

### Implemented Modules

- `src/tfy/code.py` — scope index, compact code, symbol maps, full expansion, restoration.
- `src/tfy/context.py` — missing-needed-code fallback decision policy.
- `src/tfy/tool_feedback.py` — RTK-inspired command capture/compression/raw refs.
- `src/tfy/languages.py` — extensible adapters for Python, JS/TS, Rust, Go, and C-family.
- `src/tfy/eval.py` — token estimate and savings measurement.
- `src/tfy/cli.py` — agent-neutral command surface.

### Verification

```sh
cd tfy
PYTHONPATH=src python3 -m unittest discover -s tests -v
```

Current suite covers compact code, deterministic restoration, scope-aware maps, fallback decisions, command-output compression/raw refs, multi-language adapters, CLI protocol, and savings evaluation.

### Safety Hardening

The implementation rejects ambiguous bare scope names, uses scope IDs for deterministic selection, preserves Python layout/string literals for syntactic safety, avoids rewriting Python string literals/comments/attribute names during symbol restoration, validates raw refs, uses append-only raw records, and reports a composite evaluation gate rather than treating token savings alone as success.


## Production Rust Stack

The Python implementation remains the reference prototype. The production stack is now prepared as a Rust workspace using Tree-sitter and a CLI-first JSON protocol. See `docs/PRODUCTION_STACK.md`.

Key production paths:

- `Cargo.toml` — Rust workspace root.
- `crates/tfy-core` — Rust production core.
- `crates/tfy-cli` — agent-neutral Rust CLI.
- `bindings/tfy-python` — PyO3/maturin binding scaffold.
- `oracle/fixtures` — frozen Python prototype oracle fixtures.
- `corpus/` — first-wave language fixtures.
