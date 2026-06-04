import json
import tempfile
import unittest
from pathlib import Path

from tfy.context import decide_context_need
from tfy.code import CodeIndex


class ContextFallbackTests(unittest.TestCase):
    def test_selected_context_when_symbols_known(self):
        decision = decide_context_need("def f1(a):return a", {"hello": "f1", "name": "a"})
        self.assertEqual(decision.action, "selected")

    def test_related_context_for_unmapped_symbol(self):
        decision = decide_context_need("def f1(a):return z", {"hello": "f1", "name": "a"})
        self.assertEqual(decision.action, "related")
        self.assertIn("unmapped", decision.reasons[0])

    def test_full_context_for_unmapped_symbol_and_diagnostic(self):
        decision = decide_context_need("def f1(a):return z", {"hello": "f1", "name": "a"}, "NameError: z is not defined")
        self.assertEqual(decision.action, "full")

    def test_full_cli_scope_available(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "x.py"
            source.write_text("def hello(name):\n    return name\n")
            full = CodeIndex(source).full("hello")
            self.assertIn("def hello", full["code"])
