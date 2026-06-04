import tempfile
import unittest
from pathlib import Path

from tfy.code import CodeIndex
from tfy.languages import supported_languages


class LanguageAdapterTests(unittest.TestCase):
    def test_supported_language_list(self):
        langs = supported_languages()
        self.assertIn("python", langs)
        self.assertIn("javascript-typescript", langs)
        self.assertIn("rust", langs)
        self.assertIn("go", langs)
        self.assertIn("c-family", langs)

    def test_rust_scope_extraction(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "lib.rs"
            path.write_text("pub fn calculate_total(items: i32) -> i32 { items + 1 }\n")
            scopes = CodeIndex(path).build()
            self.assertEqual(scopes[0].name, "calculate_total")
            self.assertEqual(scopes[0].language, "rust")

    def test_go_scope_extraction(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "main.go"
            path.write_text("package main\nfunc CalculateTotal(items int) int { return items + 1 }\n")
            scopes = CodeIndex(path).build()
            self.assertEqual(scopes[0].name, "CalculateTotal")
            self.assertEqual(scopes[0].language, "go")

    def test_typescript_scope_extraction(self):
        with tempfile.TemporaryDirectory() as td:
            path = Path(td) / "x.ts"
            path.write_text("export function calculateTotal(items: number): number { return items + 1 }\n")
            scopes = CodeIndex(path).build()
            self.assertEqual(scopes[0].name, "calculateTotal")
            self.assertEqual(scopes[0].language, "javascript-typescript")
