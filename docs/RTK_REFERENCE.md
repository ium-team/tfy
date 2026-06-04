# RTK Reference Notes for TFY

## Why RTK Matters

RTK is a useful reference for TFY's tool feedback layer because it already focuses on reducing LLM token consumption from command outputs.

Observed RTK concepts worth learning from:

- command proxying
- hook-based command rewriting
- command-specific filters
- TOML filters
- raw output tee/recovery
- token savings tracking
- error-only and test-failure-only wrappers
- success/noise compression

## What TFY Should Not Copy Blindly

TFY should not become only an RTK clone.

RTK primarily does:

```text
command output -> compact command output
```

TFY should do:

```text
AI work context -> token-efficient AI interface
```

That includes code representation, semantic name maps, on-demand code expansion, command feedback compression, raw fallback, and readable restoration.

## RTK-Inspired TFY Tool Feedback

TFY should learn these patterns:

1. **Preserve errors**
   - Do not hide diagnostic lines.
   - Keep file/line/symbol/exit code.

2. **Compress success**
   - Passing tests and normal progress logs can be heavily summarized.

3. **Keep raw refs**
   - Store original output and allow on-demand raw expansion.

4. **Measure savings**
   - Compare raw vs compressed output.
   - Track whether compression caused missed signals.

5. **Use command families**
   - git/test/build/shell output need different policies.

## Independence Statement

RTK is a reference and benchmark. TFY is a separate product concept with its own architecture and identity.
