import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

from tfy.cli import main


class CliTests(unittest.TestCase):
    def test_cli_index_outputs_scopes(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "x.py"
            source.write_text("def hello_world(name):\n    return name\n")
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                self.assertEqual(main(["index", str(source)]), 0)
            out = json.loads(buf.getvalue())
            self.assertEqual(out["scopes"][0]["name"], "hello_world")

    def test_cli_languages(self):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            self.assertEqual(main(["languages"]), 0)
        out = json.loads(buf.getvalue())
        self.assertIn("python", out["languages"])
        self.assertIn("rust", out["languages"])


if __name__ == "__main__":
    unittest.main()

class AgentNeutralCliTests(unittest.TestCase):
    def test_cli_exposes_protocol_commands(self):
        import argparse
        commands = ["index", "expand", "full", "restore", "run", "raw", "languages", "decide-context"]
        for command in commands:
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                with self.assertRaises(SystemExit) as cm:
                    main([command, "--help"])
            self.assertEqual(cm.exception.code, 0)

class EvalCliTests(unittest.TestCase):
    def test_eval_code_reports_savings(self):
        with tempfile.TemporaryDirectory() as td:
            source = Path(td) / "x.py"
            source.write_text("def hello_world(long_name):\n    return long_name + long_name\n")
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                self.assertEqual(main(["eval-code", str(source), "hello_world"]), 0)
            out = json.loads(buf.getvalue())
            self.assertGreater(out["evaluation"]["savings"]["saved_tokens"], 0)
