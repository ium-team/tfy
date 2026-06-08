# TFY Code and Context Method Family

This document specializes the final architecture for code, symbols, docs, and project context. It does not define TFY by itself; see `TOKEN_SAVING_ARCHITECTURE.md` for the canonical architecture.

## Registry entries

### Semantic index and skeletons

- **Target:** code files, docs, module structure.
- **Mechanism:** expose names, signatures, imports, public API, type shapes, doc headings, and call/reference hints before full bodies.
- **Savings:** avoids whole-file/repo context.
- **Cost:** parser/index work.
- **Risk:** incomplete or stale skeleton.
- **Fallback:** selected body, related neighborhood, full file/module.

### Compact code and symbol maps

- **Target:** selected scopes.
- **Mechanism:** shorten safe identifiers and compact whitespace while preserving deterministic maps.
- **Savings:** selected code bodies become smaller.
- **Cost:** parser/tokenization and map storage.
- **Risk:** model loses semantic hints or restoration accepts ambiguity.
- **Fallback:** readable-light compactness or full source.

### Dependency-neighborhood slicing

- **Target:** related code context.
- **Mechanism:** expand by graph/locality: selected scope -> callers/callees/types/tests -> module -> full.
- **Savings:** avoids full repo dumps.
- **Cost:** graph/index work.
- **Risk:** missing dynamic relation.
- **Fallback:** full file/module or full context.

### Patch/edit-script outputs

- **Target:** AI code output.
- **Mechanism:** emit compact patches, edit scripts, or scope replacements instead of full files.
- **Savings:** output tokens drop sharply.
- **Cost:** patch validation.
- **Risk:** ambiguous application.
- **Fallback:** full restored patch with diff evidence.

## Representation ladder for code

```text
file/ref fingerprint
-> semantic index
-> skeleton/signature/imports
-> selected compact scope + local map
-> related neighborhood
-> full file/module
-> full repo fallback when required
```

## Deterministic symbol maps

Symbol maps are TFY-owned artifacts:

```json
{
  "scope_id": "src/cart.ts:calculateUserDiscount:12",
  "symbols": {
    "calculateUserDiscount": "f1",
    "userProfile": "a",
    "orderItems": "b"
  },
  "reverse": {
    "f1": "calculateUserDiscount",
    "a": "userProfile",
    "b": "orderItems"
  }
}
```

AI must not invent or guess maps.

## Restoration contract

```text
compact patch
-> validate compact symbols against map
-> restore names
-> apply to original scope/file
-> run formatter/parser when available
-> compare diff
-> produce human/project-readable output
```

Reject unmapped, ambiguous, public/unsafe, or non-renamable symbol changes unless policy explicitly allows them.

## Code fallback triggers

- unresolved identifier diagnostics
- unmapped compact symbols
- dependency outside selected scope
- public API, reflection, macro, serialization, generated code, dynamic dispatch, or global side-effect involvement
- failed parse/restore/diff validation
- user/agent requests full context
