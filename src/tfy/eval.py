from __future__ import annotations

from dataclasses import dataclass
from typing import Dict


def token_estimate(text: str) -> int:
    return max(1, (len(text) + 3) // 4) if text else 0


@dataclass(frozen=True)
class Savings:
    raw_tokens: int
    compact_tokens: int
    saved_tokens: int
    savings_pct: float

    def to_dict(self) -> dict:
        return self.__dict__.copy()


@dataclass(frozen=True)
class QualityScore:
    restoration_success: bool
    fallback_action: str
    missed_needed_code_failures: int
    missed_command_error_failures: int
    task_success_baseline: bool
    task_success_compact: bool

    def to_dict(self) -> dict:
        return self.__dict__.copy()


@dataclass(frozen=True)
class EvaluationReport:
    savings: Savings
    quality: QualityScore
    release_gate: str

    def to_dict(self) -> dict:
        return {"savings": self.savings.to_dict(), "quality": self.quality.to_dict(), "release_gate": self.release_gate}


def measure(raw: str, compact: str) -> Savings:
    raw_t = token_estimate(raw)
    compact_t = token_estimate(compact)
    saved = raw_t - compact_t
    pct = 0.0 if raw_t == 0 else round(saved / raw_t * 100, 2)
    return Savings(raw_t, compact_t, saved, pct)


def evaluate(raw: str, compact: str, quality: QualityScore) -> EvaluationReport:
    savings = measure(raw, compact)
    passed = (
        savings.saved_tokens >= 0
        and quality.restoration_success
        and quality.missed_needed_code_failures == 0
        and quality.missed_command_error_failures == 0
        and (not quality.task_success_baseline or quality.task_success_compact)
    )
    return EvaluationReport(savings, quality, "PASS" if passed else "FAIL")
