import tempfile
import unittest
from pathlib import Path

from tfy.code import CodeIndex, restore_from_payload


class CodeTests(unittest.TestCase):
    def test_names_first_index_and_symbol_compaction(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "sample.py"
            source.write_text("""
def calculate_total_price(items, tax_rate):
    subtotal = 0
    for item in items:
        subtotal += item.price * item.quantity
    return subtotal + subtotal * tax_rate
""".strip()+"\n")
            index = CodeIndex(source)
            scopes = index.build()
            self.assertEqual(scopes[0].name, "calculate_total_price")
            compact = index.expand(scopes[0].id)
            self.assertNotIn("calculate_total_price", compact.compact_code)
            self.assertEqual(compact.symbol_map.symbols["items"], "a")
            self.assertGreater(compact.savings_pct, 0)

    def test_restore_from_compact_payload_is_deterministic(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "sample.py"
            source.write_text("def calculate_total_price(items, tax_rate):\n    return items + tax_rate\n")
            compact = CodeIndex(source).expand("calculate_total_price")
            payload = compact.to_dict()
            payload["compact_code"] = compact.compact_code
            restored = restore_from_payload(payload)["restored_code"]
            self.assertIn("items", restored)
            self.assertIn("tax_rate", restored)

    def test_light_compact_has_no_symbol_map(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "sample.py"
            source.write_text("def alpha(long_name):\n    return long_name\n")
            compact = CodeIndex(source).expand("alpha", compactness="light")
            self.assertEqual(compact.symbol_map.symbols, {})
            self.assertIn("long_name", compact.compact_code)


if __name__ == "__main__":
    unittest.main()

class SymbolMapScopeTests(unittest.TestCase):
    def test_symbol_maps_are_scope_aware_and_deterministic(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "two.py"
            source.write_text("""
def first_scope(shared_name, first_only):
    return shared_name + first_only


def second_scope(shared_name, second_only):
    return shared_name + second_only
""".strip()+"\n")
            index = CodeIndex(source)
            first = index.expand("first_scope")
            second = index.expand("second_scope")
            again = index.expand("first_scope")
            self.assertNotEqual(first.symbol_map.scope_id, second.symbol_map.scope_id)
            self.assertEqual(first.symbol_map.symbols, again.symbol_map.symbols)
            self.assertEqual(first.symbol_map.symbols["shared_name"], "a")
            self.assertEqual(second.symbol_map.symbols["shared_name"], "a")
            self.assertIn("first_scope", first.symbol_map.symbols)
            self.assertIn("second_scope", second.symbol_map.symbols)

class RestorationSafetyTests(unittest.TestCase):
    def test_restore_rejects_unmapped_compact_symbol(self):
        payload = {
            "scope_id": "x",
            "symbols": {"hello": "f1", "name": "a"},
            "compact_code": "def f1(a):return z",
        }
        with self.assertRaises(ValueError):
            restore_from_payload(payload)

class ReviewRegressionTests(unittest.TestCase):
    def test_short_identifiers_are_mapped_without_restore_collision(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "shorts.py"
            source.write_text("def uses_short(a, v1):\n    i = a + v1\n    return i\n")
            compact = CodeIndex(source).expand("uses_short")
            self.assertIn("a", compact.symbol_map.symbols)
            self.assertIn("v1", compact.symbol_map.symbols)
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            self.assertIn("uses_short", restored)
            self.assertIn("v1", restored)

    def test_restore_does_not_rewrite_string_literals(self):
        payload = {"scope_id": "x", "symbols": {"foo": "f1"}, "compact_code": 'def f1():return "f1"'}
        restored = restore_from_payload(payload)["restored_code"]
        self.assertIn('"f1"', restored)
        self.assertIn("foo", restored)

    def test_ambiguous_bare_scope_name_requires_id(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            (root / "a").mkdir()
            (root / "b").mkdir()
            (root / "a" / "same.py").write_text("def duplicate():\n    return 1\n")
            (root / "b" / "same.py").write_text("def duplicate():\n    return 2\n")
            index = CodeIndex(root)
            scopes = index.build()
            self.assertEqual(len([s for s in scopes if s.name == "duplicate"]), 2)
            with self.assertRaises(ValueError):
                index.expand("duplicate")
            selected = index.expand("a/same.py:duplicate:1")
            self.assertIn("return 1", index.full(selected.scope.id)["code"])

    def test_python_class_and_nested_function_scope_are_not_truncated(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "nested.py"
            source.write_text("""
class Greeter:
    def hello(self):
        return "hello"


def outer(value):
    def inner():
        return value
    return inner()
""".strip()+"\n")
            index = CodeIndex(source)
            class_code = index.full("Greeter")["code"]
            outer_code = index.full("outer")["code"]
            self.assertIn("def hello", class_code)
            self.assertIn("def inner", outer_code)
            self.assertIn("return inner()", outer_code)

class PythonRestoreValidityTests(unittest.TestCase):
    def test_restored_compacted_python_function_compiles(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "sample.py"
            source.write_text("""
def calculate_total_price(items, tax_rate):
    subtotal = 0
    for item in items:
        subtotal += item.price * item.quantity
    if subtotal > 0:
        return subtotal + subtotal * tax_rate
    return subtotal
""".strip()+"\n")
            compact = CodeIndex(source).expand("calculate_total_price")
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            compile(restored, "<restored>", "exec")
            self.assertIn("for item in items", restored)
            self.assertIn("if subtotal", restored)

class PythonCompactionSafetyTests(unittest.TestCase):
    def test_python_compaction_preserves_string_literal_contents(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "literal.py"
            source.write_text('def literal():\n    message = "a : b"\n    return message\n')
            compact = CodeIndex(source).expand("literal")
            self.assertIn('"a : b"', compact.compact_code)
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            self.assertIn('"a : b"', restored)
            compile(restored, "<restored>", "exec")

    def test_light_mode_python_restore_preserves_multistatement_layout(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "light.py"
            source.write_text("""
def add_items(items):
    total = 0
    for item in items:
        total += item
    return total
""".strip()+"\n")
            compact = CodeIndex(source).expand("add_items", compactness="light")
            self.assertEqual(compact.symbol_map.symbols, {})
            self.assertIn("\n    for item in items:", compact.compact_code)
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            compile(restored, "<restored>", "exec")
            self.assertIn("for item in items", restored)

class LightModeRoundTripContractTests(unittest.TestCase):
    def test_light_mode_python_short_identifiers_restore_without_symbol_map(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "short_light.py"
            source.write_text("def f(x):\n    return x\n")
            compact = CodeIndex(source).expand("f", compactness="light")
            self.assertEqual(compact.compactness, "light")
            self.assertEqual(compact.symbol_map.symbols, {})
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            self.assertIn("def f", restored)
            self.assertIn("return x", restored)
            compile(restored, "<restored>", "exec")

    def test_light_mode_generic_short_identifiers_restore_without_symbol_map(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "short.js"
            source.write_text("function f(x) {\n  return x + 1;\n}\n")
            compact = CodeIndex(source).expand("f", compactness="light")
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            self.assertIn("function f", restored)
            self.assertIn("return x+1", restored)

class NonPythonLiteralSafetyTests(unittest.TestCase):
    def test_javascript_symbol_compaction_preserves_string_literal_contents(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "greet.js"
            source.write_text('function greet(name) {\n  return "hello : " + name;\n}\n')
            compact = CodeIndex(source).expand("greet")
            self.assertIn('"hello : "', compact.compact_code)
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            self.assertIn('"hello : "', restored)
            self.assertIn("greet", restored)
            self.assertIn("name", restored)

    def test_javascript_template_literal_is_not_reformatted(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "tmpl.js"
            source.write_text('function greet(name) {\n  return `hello : ${name}`;\n}\n')
            compact = CodeIndex(source).expand("greet")
            self.assertIn('`hello : ${name}`', compact.compact_code)
            restored = restore_from_payload(compact.to_dict())["restored_code"]
            self.assertIn('`hello : ${name}`', restored)

class BraceScannerSafetyTests(unittest.TestCase):
    def test_javascript_scope_not_truncated_by_template_literal_brace(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "brace.js"
            source.write_text('function show() {\n  const x = `literal } brace`;\n  return x;\n}\n')
            full = CodeIndex(source).full("show")["code"]
            self.assertIn("return x", full)
            self.assertIn("literal } brace", full)
