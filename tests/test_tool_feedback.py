import tempfile
import unittest

from tfy.tool_feedback import RawStore, ToolFeedbackCompressor


class ToolFeedbackTests(unittest.TestCase):
    def test_success_output_is_compressed_and_raw_ref_available(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            summary = ToolFeedbackCompressor(store).compress("git status", "nothing to commit, working tree clean\n", 0)
            self.assertEqual(summary.risk, "success")
            self.assertIn("raw_ref=", summary.summary)
            self.assertEqual(store.raw(summary.raw_ref), "nothing to commit, working tree clean\n")

    def test_error_output_preserves_evidence(self):
        with tempfile.TemporaryDirectory() as td:
            raw = "src/main.rs:42: error[E0308]: mismatched types\nlong noise\n"
            summary = ToolFeedbackCompressor(RawStore(td)).compress("cargo test", raw, 101)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("src/main.rs:42", summary.summary)
            self.assertIn("raw_ref=", summary.summary)

    def test_raw_range_expansion(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            ref = store.put("pytest", "a\nb\nfile.py:9: error\nc\nd\n", 1)
            selected = store.raw(ref, around="file.py:9", context=1)
            self.assertIn("b", selected)
            self.assertIn("file.py:9", selected)
            self.assertIn("c", selected)


if __name__ == "__main__":
    unittest.main()

class RawStoreSecurityRegressionTests(unittest.TestCase):
    def test_raw_ref_rejects_path_traversal(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            with self.assertRaises(ValueError):
                store.raw("../secret")

    def test_raw_ref_is_append_only_for_same_text_different_exit(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            ref_ok = store.put("cmd", "same output", 0)
            ref_bad = store.put("cmd", "same output", 1)
            self.assertNotEqual(ref_ok, ref_bad)
            self.assertEqual(store.get(ref_ok)["exit_code"], 0)
            self.assertEqual(store.get(ref_bad)["exit_code"], 1)

    def test_run_timeout_records_critical_summary(self):
        with tempfile.TemporaryDirectory() as td:
            summary = ToolFeedbackCompressor(RawStore(td)).run(["python3", "-c", "import time; time.sleep(2)"], timeout=0.1)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("timed out", store_text := RawStore(td).raw(summary.raw_ref))

class CommandOutputTruncationSafetyTests(unittest.TestCase):
    def test_run_preserves_stderr_error_when_stdout_is_large(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            summary = ToolFeedbackCompressor(store).run([
                "python3", "-c",
                "import sys; print('noise'*1000); sys.stderr.write('src/main.py:9: error: broken\\n'); sys.exit(1)"
            ], timeout=5, max_output_bytes=512)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("src/main.py:9", summary.summary)
            raw = store.raw(summary.raw_ref)
            self.assertIn("src/main.py:9", raw)
            self.assertIn("truncated", raw)

class StderrTailSafetyTests(unittest.TestCase):
    def test_run_preserves_late_stderr_error_when_stderr_exceeds_cap(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            summary = ToolFeedbackCompressor(store).run([
                "python3", "-c",
                "import sys; sys.stderr.write('noise\\n'*1000); sys.stderr.write('src/main.py:9: error: broken\\n'); sys.exit(1)"
            ], timeout=5, max_output_bytes=512)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("src/main.py:9", summary.summary)
            self.assertIn("src/main.py:9", store.raw(summary.raw_ref))

class RawFallbackCompletenessTests(unittest.TestCase):
    def test_run_preserves_late_stdout_error_and_classifies_critical(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            summary = ToolFeedbackCompressor(store).run([
                "python3", "-c",
                "print('ok'); print('noise'*1000); print('src/main.py:9: error: broken')"
            ], timeout=5, max_output_bytes=128)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("src/main.py:9", summary.summary)
            self.assertIn("src/main.py:9", store.raw(summary.raw_ref))

    def test_raw_ref_returns_full_output_not_bounded_snapshot(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            summary = ToolFeedbackCompressor(store).run([
                "python3", "-c",
                "print('BEGIN'); print('x'*2000); print('END_MARKER')"
            ], timeout=5, max_output_bytes=64)
            raw = store.raw(summary.raw_ref)
            self.assertIn("BEGIN", raw)
            self.assertIn("END_MARKER", raw)
            self.assertGreater(len(raw), 64)

class CommandLaunchAndSummaryBoundTests(unittest.TestCase):
    def test_run_launch_failure_returns_critical_summary_with_raw_ref(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            summary = ToolFeedbackCompressor(store).run(["does-not-exist-tfy-review"], timeout=5)
            self.assertEqual(summary.risk, "critical")
            self.assertEqual(summary.exit_code, 127)
            self.assertIn("raw_ref=", summary.summary)
            self.assertIn("command launch failed", store.raw(summary.raw_ref))

    def test_unknown_summary_caps_long_single_line_but_raw_is_full(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            raw = "x" * 2000 + "\n"
            summary = ToolFeedbackCompressor(store).compress("long-line", raw, 0)
            self.assertEqual(summary.risk, "unknown")
            self.assertLess(len(summary.summary), 600)
            self.assertIn("line capped", summary.summary)
            self.assertEqual(store.raw(summary.raw_ref), raw)

class CriticalEvidenceExcerptTests(unittest.TestCase):
    def test_critical_summary_excerpts_around_late_error_in_long_line(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            raw = "x" * 2000 + " src/main.py:9: error: broken\n"
            summary = ToolFeedbackCompressor(store).compress("long-critical", raw, 1)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("src/main.py:9", summary.summary)
            self.assertIn("src/main.py:9", store.raw(summary.raw_ref))
            self.assertLess(len(summary.summary), 600)

class CriticalEvidenceWhitespaceOffsetTests(unittest.TestCase):
    def test_critical_summary_excerpts_after_repeated_spaces_before_error(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            raw = ("x  " * 1000) + "src/main.py:9: error: broken\n"
            summary = ToolFeedbackCompressor(store).compress("spaced-critical", raw, 1)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("src/main.py:9", summary.summary)
            self.assertNotEqual(summary.evidence, ["…"])

class CriticalEvidencePriorityTests(unittest.TestCase):
    def test_tail_error_is_prioritized_after_many_file_refs(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            refs = "".join(f"src/noise{i}.py:1: note\n" for i in range(13))
            raw = refs + "tail.py:99: error: broken\n"
            summary = ToolFeedbackCompressor(store).compress("many-refs", raw, 1)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("tail.py:99", summary.summary)
            self.assertIn("tail.py:99", summary.evidence[0])

class CriticalSameLineEvidenceTests(unittest.TestCase):
    def test_same_long_line_preserves_file_ref_and_error(self):
        with tempfile.TemporaryDirectory() as td:
            store = RawStore(td)
            raw = "src/main.py:9: " + ("x" * 1000) + " error: broken\n"
            summary = ToolFeedbackCompressor(store).compress("same-line", raw, 1)
            self.assertEqual(summary.risk, "critical")
            self.assertIn("src/main.py:9", summary.summary)
            self.assertIn("error", summary.summary)
            self.assertLess(len(summary.summary), 700)
