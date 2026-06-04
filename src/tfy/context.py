from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Dict, List


@dataclass(frozen=True)
class ContextDecision:
    action: str
    reasons: List[str]

    def to_dict(self) -> Dict[str, object]:
        return {"action": self.action, "reasons": self.reasons}


def decide_context_need(compact_code: str, known_symbols: Dict[str, str], diagnostics: str = "") -> ContextDecision:
    """Decide whether selected compact context is enough.

    action values:
    - selected: current compact body is likely enough
    - related: expand related call sites/neighborhood
    - full: request full scope/file context
    """
    reasons: List[str] = []
    reverse = {v: k for k, v in known_symbols.items()}
    tokens = set(re.findall(r"\b[A-Za-z_][A-Za-z0-9_]*\b", compact_code + "\n" + diagnostics))
    compact_names = set(reverse)
    suspicious = sorted(t for t in tokens if _looks_compact(t) and t not in compact_names and t not in {"def", "return", "for", "in", "if", "else"})
    if suspicious:
        reasons.append("unmapped compact symbols: " + ",".join(suspicious[:8]))
    if re.search(r"(NameError|ReferenceError|cannot find|not found|undefined|unresolved|E0425)", diagnostics, re.I):
        reasons.append("diagnostics mention unresolved or missing symbol")
    if re.search(r"(macro|dynamic dispatch|inheritance|global state|side effect|generated code)", diagnostics, re.I):
        reasons.append("diagnostics mention non-local behavior")
    if len(reasons) >= 2:
        return ContextDecision("full", reasons)
    if reasons:
        return ContextDecision("related", reasons)
    return ContextDecision("selected", ["selected compact scope appears sufficient"])


def _looks_compact(token: str) -> bool:
    return bool(re.fullmatch(r"(?:f\d+|[a-z]|v\d+)", token))
