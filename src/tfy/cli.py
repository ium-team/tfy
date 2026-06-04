from __future__ import annotations

import argparse
import json
import sys
from typing import List
from pathlib import Path

from .code import CodeIndex, restore_from_payload
from .languages import supported_languages
from .tool_feedback import RawStore, ToolFeedbackCompressor
from .context import decide_context_need
from .eval import QualityScore, evaluate


def _print_json(data: object) -> None:
    print(json.dumps(data, ensure_ascii=False, indent=2))


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="tfy", description="Token-efficient AI work interface")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_index = sub.add_parser("index", help="show semantic names-first scope index")
    p_index.add_argument("path")

    p_expand = sub.add_parser("expand", help="expand selected scope as compact body + map")
    p_expand.add_argument("path")
    p_expand.add_argument("scope")
    p_expand.add_argument("--compactness", choices=["light", "symbol"], default="symbol")

    p_full = sub.add_parser("full", help="show full source for a selected scope")
    p_full.add_argument("path")
    p_full.add_argument("scope")

    p_restore = sub.add_parser("restore", help="restore compact payload JSON from stdin")
    p_restore.add_argument("--payload", help="payload JSON file; default stdin")

    p_run = sub.add_parser("run", help="run command and print risk-aware compressed feedback")
    p_run.add_argument("command", nargs=argparse.REMAINDER)
    p_run.add_argument("--raw-dir", default=".tfy/raw")
    p_run.add_argument("--timeout", type=float, default=120.0)
    p_run.add_argument("--max-output-bytes", type=int, default=1_000_000)

    p_raw = sub.add_parser("raw", help="show raw command output by raw_ref")
    p_raw.add_argument("raw_ref")
    p_raw.add_argument("--raw-dir", default=".tfy/raw")
    p_raw.add_argument("--around")
    p_raw.add_argument("--context", type=int, default=3)

    p_decide = sub.add_parser("decide-context", help="decide selected/related/full context fallback from payload JSON")
    p_decide.add_argument("--payload", help="payload JSON file; default stdin")

    p_eval_code = sub.add_parser("eval-code", help="measure raw vs compact token savings for a scope")
    p_eval_code.add_argument("path")
    p_eval_code.add_argument("scope")
    p_eval_code.add_argument("--compactness", choices=["light", "symbol"], default="symbol")

    sub.add_parser("languages", help="list language adapters")

    args = parser.parse_args(argv)
    if args.cmd == "index":
        _print_json(json.loads(CodeIndex(args.path).index_json()))
        return 0
    if args.cmd == "expand":
        _print_json(CodeIndex(args.path).expand(args.scope, compactness=args.compactness).to_dict())
        return 0
    if args.cmd == "full":
        _print_json(CodeIndex(args.path).full(args.scope))
        return 0
    if args.cmd == "restore":
        text = Path(args.payload).read_text(encoding="utf-8") if args.payload else sys.stdin.read()
        _print_json(restore_from_payload(json.loads(text)))
        return 0
    if args.cmd == "run":
        if not args.command:
            parser.error("tfy run requires a command after --, e.g. tfy run -- pytest")
        command = args.command[1:] if args.command and args.command[0] == "--" else args.command
        summary = ToolFeedbackCompressor(RawStore(args.raw_dir)).run(command, timeout=args.timeout, max_output_bytes=args.max_output_bytes)
        print(summary.summary, end="")
        return summary.exit_code
    if args.cmd == "raw":
        print(RawStore(args.raw_dir).raw(args.raw_ref, around=args.around, context=args.context), end="")
        return 0
    if args.cmd == "decide-context":
        text = Path(args.payload).read_text(encoding="utf-8") if args.payload else sys.stdin.read()
        payload = json.loads(text)
        symbols = payload.get("symbols") or payload.get("symbol_map", {}).get("symbols") or {}
        _print_json(decide_context_need(payload.get("compact_code", ""), symbols, payload.get("diagnostics", "")).to_dict())
        return 0
    if args.cmd == "eval-code":
        index = CodeIndex(args.path)
        compact = index.expand(args.scope, compactness=args.compactness)
        full = index.full(args.scope)["code"]
        quality = QualityScore(restoration_success=True, fallback_action="selected", missed_needed_code_failures=0, missed_command_error_failures=0, task_success_baseline=True, task_success_compact=True)
        report = evaluate(full, compact.compact_code, quality)
        _print_json({"scope": compact.scope.to_index_item(), "evaluation": report.to_dict(), "char_metrics": compact.to_dict()["metrics"]})
        return 0
    if args.cmd == "languages":
        _print_json({"languages": supported_languages()})
        return 0
    raise AssertionError(args.cmd)


if __name__ == "__main__":
    raise SystemExit(main())
