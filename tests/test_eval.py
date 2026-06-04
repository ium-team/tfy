import unittest
from tfy.eval import measure


class EvalTests(unittest.TestCase):
    def test_measure_net_savings(self):
        savings = measure("a" * 100, "b" * 20)
        self.assertGreater(savings.saved_tokens, 0)
        self.assertGreater(savings.savings_pct, 0)


if __name__ == "__main__":
    unittest.main()
from tfy.eval import QualityScore, evaluate

class QualityEvaluationTests(unittest.TestCase):
    def test_release_gate_fails_when_compact_task_regresses(self):
        quality = QualityScore(
            restoration_success=True,
            fallback_action="selected",
            missed_needed_code_failures=0,
            missed_command_error_failures=0,
            task_success_baseline=True,
            task_success_compact=False,
        )
        report = evaluate("long raw", "x", quality)
        self.assertEqual(report.release_gate, "FAIL")
