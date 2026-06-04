# TFY Code Compression, Symbol Maps, and Restoration

## Problem

Human-friendly code costs tokens:

- long semantic names
- indentation
- newlines
- repeated boilerplate
- whole-file context sent when only one function matters

But semantic names help AI understand code. TFY therefore separates compact transport from semantic meaning.

## Compact Code

AI-facing code can remove unnecessary formatting and shorten identifiers.

Human-readable source:

```js
function calculateUserDiscount(userProfile, orderItems) {
  const membershipLevel = getMembershipLevel(userProfile);
  return applyDiscount(membershipLevel, orderItems);
}
```

TFY compact form:

```js
f1(a,b){c=f2(a);return f3(c,b)}
```

## 1:1 Symbol Map

TFY provides a deterministic mapping when needed:

```json
{
  "f1": "calculateUserDiscount",
  "a": "userProfile",
  "b": "orderItems",
  "c": "membershipLevel",
  "f2": "getMembershipLevel",
  "f3": "applyDiscount"
}
```

This map is produced by TFY, not invented by AI.

## Names-First Discovery

Instead of sending all code, TFY first sends semantic names:

```text
Available functions:
- calculateUserDiscount
- validatePaymentMethod
- createOrderSummary
- syncInventoryAfterOrder
```

AI picks likely relevant scopes by name.

Then TFY sends only selected compact bodies and the local mapping.

## Expansion Ladder

```text
semantic names
-> signatures / structure
-> selected compact bodies + local map
-> related callers/callees/types/tests
-> full file/module
-> full context fallback
```

## Restoration

Restoration means converting AI compact output back into human/project-readable source.

```text
AI compact patch
-> resolve f1/a/b via symbol map
-> apply to original scope/file
-> restore original names
-> run formatter
-> validate parse/diff
-> readable code output
```

Restoration is deterministic. AI should not be responsible for guessing original names or formatting.

## Important Rule

Full context fallback is not a failure. It is a safety mechanism when compact context is insufficient.
