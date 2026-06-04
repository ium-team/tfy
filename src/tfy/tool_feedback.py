from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import hashlib
import json
import re
import subprocess
import tempfile
import time
from typing import List, Optional, Sequence, Union


@dataclass(frozen=True)
class CommandSummary:
    command: str
    exit_code: int
    risk: str
    summary: str
    raw_ref: str
    raw_chars: int
    summary_chars: int
    savings_pct: float
    evidence: List[str]

    def to_dict(self) -> dict:
        return self.__dict__.copy()


class RawStore:
    REF_RE = re.compile(r"^cmdout_[0-9a-f]{12}_[0-9a-f]{16}$")

    def __init__(self, root: Union[str, Path] = ".tfy/raw"):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)

    def put(self, command: str, text: str, exit_code: int) -> str:
        payload = {"command": command, "exit_code": exit_code, "raw": text, "created_ns": time.time_ns()}
        for attempt in range(16):
            nonce = (time.time_ns() + attempt).to_bytes(8, "big", signed=False).hex()
            digest = hashlib.sha256((command + "\0" + str(exit_code) + "\0" + text + "\0" + nonce).encode("utf-8", "replace")).hexdigest()[:12]
            ref = f"cmdout_{digest}_{nonce}"
            path = self._path_for_ref(ref)
            try:
                with path.open("x", encoding="utf-8") as fh:
                    json.dump(payload, fh, ensure_ascii=False, indent=2)
                return ref
            except FileExistsError:
                continue
        raise RuntimeError("could not allocate unique raw_ref")

    def _path_for_ref(self, raw_ref: str) -> Path:
        if not self.REF_RE.fullmatch(raw_ref):
            raise ValueError(f"invalid raw ref: {raw_ref}")
        root = self.root.resolve()
        path = (root / f"{raw_ref}.json").resolve()
        if root != path.parent:
            raise ValueError(f"raw ref escapes store: {raw_ref}")
        return path

    def get(self, raw_ref: str) -> dict:
        path = self._path_for_ref(raw_ref)
        if not path.exists():
            raise KeyError(f"raw ref not found: {raw_ref}")
        return json.loads(path.read_text(encoding="utf-8"))

    def raw(self, raw_ref: str, around: Optional[str] = None, context: int = 3) -> str:
        payload = self.get(raw_ref)
        raw = payload["raw"]
        if not around:
            return raw
        lines = raw.splitlines()
        hits = [i for i, line in enumerate(lines) if around in line]
        if not hits:
            return raw
        selected: List[str] = []
        for hit in hits[:5]:
            start = max(0, hit - context)
            end = min(len(lines), hit + context + 1)
            selected.extend(lines[start:end])
        return "\n".join(selected) + "\n"


def _read_head(handle, max_bytes: int) -> tuple[bytes, bool]:
    handle.seek(0, 2)
    size = handle.tell()
    handle.seek(0)
    return handle.read(max_bytes), size > max_bytes


def _read_tail(handle, max_bytes: int) -> tuple[bytes, bool]:
    handle.seek(0, 2)
    size = handle.tell()
    start = max(0, size - max_bytes)
    handle.seek(start)
    return handle.read(max_bytes), size > max_bytes


def _append_unique(items: List[str], item: str) -> None:
    if item and item not in items:
        items.append(item)


def _line_excerpt(line: str, match: re.Match[str]) -> str:
    return _line_excerpt_for_matches(line, (match,))


def _line_excerpt_for_matches(line: str, matches: tuple[re.Match[str], ...], limit: int = 240) -> str:
    start = min(match.start() for match in matches)
    end = max(match.end() for match in matches)
    if end - start > limit:
        parts = [_excerpt_around_span(line, match.start(), match.end(), limit=max(80, limit // len(matches))) for match in matches]
        excerpt = " … ".join(parts)
    else:
        excerpt = _excerpt_around_span(line, start, end, limit=limit)
    return re.sub(r"\s+", " ", excerpt).strip()


def _cap_line(line: str, limit: int = 240) -> str:
    return line if len(line) <= limit else line[:limit] + "…[tfy: line capped]"


def _excerpt_around_span(line: str, start: int, end: int, limit: int = 240) -> str:
    if len(line) <= limit:
        return line
    context = max(20, (limit - (end - start)) // 2)
    excerpt_start = max(0, start - context)
    excerpt_end = min(len(line), end + context)
    excerpt = line[excerpt_start:excerpt_end]
    if excerpt_start > 0:
        excerpt = "…" + excerpt
    if excerpt_end < len(line):
        excerpt += "…"
    return excerpt


def _bounded_combined_output(out_bytes: bytes, err_bytes: bytes, max_output_bytes: int) -> str:
    stdout = out_bytes.decode("utf-8", "replace")
    stderr = err_bytes.decode("utf-8", "replace")
    combined = stdout + stderr
    if len(combined.encode("utf-8", "replace")) <= max_output_bytes:
        return combined
    marker = f"\n[tfy: combined output truncated at {max_output_bytes} bytes; stdout head and stderr tail preserved]\n"
    budget = max(0, max_output_bytes - len(marker.encode("utf-8")))
    if stderr:
        err_budget = min(len(stderr.encode("utf-8", "replace")), max(128, budget // 2))
        out_budget = max(0, budget - err_budget)
        out_part = stdout.encode("utf-8", "replace")[:out_budget].decode("utf-8", "replace")
        err_part = stderr.encode("utf-8", "replace")[-err_budget:].decode("utf-8", "replace")
        return out_part + marker + err_part
    return stdout.encode("utf-8", "replace")[:budget].decode("utf-8", "replace") + marker


class ToolFeedbackCompressor:
    ERROR_PATTERNS = re.compile(r"(error|failed|failure|panic|traceback|exception|fatal|denied|not found|cannot|E\d{3,}|✗|FAIL)", re.I)
    SUCCESS_PATTERNS = re.compile(r"(success|passed|ok|done|clean|up.to.date|nothing to commit|no changes|0 failures?)", re.I)
    FILE_LINE = re.compile(r"(?:[\w./-]+\.(?:py|rs|js|ts|tsx|jsx|go|java|c|cpp|h|hpp|md))(?::\d+)?")

    def __init__(self, raw_store: Optional[RawStore] = None):
        self.raw_store = raw_store or RawStore()

    def run(self, command: Sequence[str], cwd: Optional[Union[str, Path]] = None, timeout: float = 120.0, max_output_bytes: int = 1_000_000) -> CommandSummary:
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            try:
                proc = subprocess.Popen(command, cwd=cwd, stdout=stdout, stderr=stderr)
            except OSError as exc:
                raw = f"[tfy: command launch failed] {type(exc).__name__}: {exc}\n"
                return self.compress(" ".join(command), raw, 127)
            timed_out = False
            try:
                proc.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                proc.kill()
                proc.wait()
            stdout.seek(0)
            stderr.seek(0)
            out_bytes = stdout.read()
            err_bytes = stderr.read()
        raw = (out_bytes + err_bytes).decode("utf-8", "replace")
        if len(out_bytes) + len(err_bytes) > max_output_bytes:
            raw += f"\n[tfy: summary output exceeded {max_output_bytes} bytes; raw_ref is not truncated and preserves full captured stdout/stderr before this notice]\n"
        if timed_out:
            raw += f"\n[tfy: command timed out after {timeout} seconds]\n"
        exit_code = proc.returncode if proc.returncode is not None else -1
        return self.compress(" ".join(command), raw, exit_code)

    def compress(self, command: str, raw: str, exit_code: int = 0) -> CommandSummary:
        raw_ref = self.raw_store.put(command, raw, exit_code)
        risk = self._risk(raw, exit_code)
        evidence = self._evidence(raw)
        if risk == "critical":
            summary = self._critical_summary(command, raw, exit_code, evidence, raw_ref)
        elif risk == "success":
            summary = self._success_summary(command, raw, exit_code, raw_ref)
        else:
            summary = self._conservative_summary(command, raw, exit_code, evidence, raw_ref)
        raw_len = len(raw)
        out_len = len(summary)
        savings = 0.0 if raw_len == 0 else round((raw_len - out_len) / raw_len * 100, 2)
        return CommandSummary(command, exit_code, risk, summary, raw_ref, raw_len, out_len, savings, evidence)

    def _risk(self, raw: str, exit_code: int) -> str:
        if exit_code != 0 or self.ERROR_PATTERNS.search(raw):
            return "critical"
        if self.SUCCESS_PATTERNS.search(raw) or not raw.strip():
            return "success"
        return "unknown"

    def _evidence(self, raw: str) -> List[str]:
        error_lines: List[str] = []
        file_lines: List[str] = []
        for line in raw.splitlines():
            error_match = self.ERROR_PATTERNS.search(line)
            file_match = self.FILE_LINE.search(line)
            if error_match and file_match:
                _append_unique(error_lines, _line_excerpt_for_matches(line, (file_match, error_match)))
            elif error_match:
                _append_unique(error_lines, _line_excerpt(line, error_match))
            elif file_match:
                _append_unique(file_lines, _line_excerpt(line, file_match))
        return (error_lines + file_lines)[:12]

    def _critical_summary(self, command: str, raw: str, exit_code: int, evidence: List[str], raw_ref: str) -> str:
        body = "\n".join(f"- {line}" for line in evidence) or "- critical output present; request raw for details"
        return f"TFY command summary: CRITICAL exit={exit_code} cmd={command}\n{body}\nraw_ref={raw_ref}\n"

    def _success_summary(self, command: str, raw: str, exit_code: int, raw_ref: str) -> str:
        lines = [line.strip() for line in raw.splitlines() if line.strip()]
        first = lines[0] if lines else "no output"
        return f"TFY command summary: SUCCESS exit={exit_code} cmd={command}; {first[:120]}; raw_ref={raw_ref}\n"

    def _conservative_summary(self, command: str, raw: str, exit_code: int, evidence: List[str], raw_ref: str) -> str:
        lines = [_cap_line(re.sub(r"\s+", " ", line).strip()) for line in raw.splitlines() if line.strip()]
        head = lines[:5]
        tail = lines[-3:] if len(lines) > 8 else []
        selected = head + (["..."] if tail else []) + tail
        if evidence:
            selected.extend([f"evidence: {x}" for x in evidence[:5]])
        body = "\n".join(selected) or "no output"
        return f"TFY command summary: UNKNOWN exit={exit_code} cmd={command}\n{body}\nraw_ref={raw_ref}\n"
