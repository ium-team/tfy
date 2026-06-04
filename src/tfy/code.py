from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import io
import json
import keyword
import re
import tokenize
from typing import Dict, Iterable, List, Tuple, Union

from .languages import LanguageAdapter, adapter_for_path


@dataclass(frozen=True)
class Scope:
    id: str
    name: str
    path: str
    language: str
    start_line: int
    end_line: int
    start_offset: int
    end_offset: int
    kind: str = "scope"

    def to_index_item(self) -> dict:
        return {k: getattr(self, k) for k in ("id", "name", "path", "language", "start_line", "end_line", "kind")}


@dataclass(frozen=True)
class SymbolMap:
    scope_id: str
    symbols: Dict[str, str]

    @property
    def reverse(self) -> Dict[str, str]:
        return {short: original for original, short in self.symbols.items()}

    def to_dict(self) -> dict:
        return {"scope_id": self.scope_id, "symbols": self.symbols, "reverse": self.reverse}


@dataclass(frozen=True)
class CompactScope:
    scope: Scope
    compact_code: str
    symbol_map: SymbolMap
    raw_chars: int
    compact_chars: int
    savings_pct: float
    compactness: str = "symbol"

    def to_dict(self) -> dict:
        return {
            "scope": self.scope.to_index_item(),
            "compactness": self.compactness,
            "compact_code": self.compact_code,
            "symbol_map": self.symbol_map.to_dict(),
            "metrics": {"raw_chars": self.raw_chars, "compact_chars": self.compact_chars, "savings_pct": self.savings_pct},
        }


class CodeIndex:
    def __init__(self, root: Union[str, Path]):
        self.root = Path(root)

    def iter_files(self) -> Iterable[Path]:
        ignored = {".git", ".omx", "__pycache__", "node_modules", "target", "dist", "build", ".venv"}
        if self.root.is_file():
            yield self.root
            return
        for path in sorted(self.root.rglob("*")):
            if not path.is_file():
                continue
            if any(part in ignored for part in path.parts):
                continue
            if adapter_for_path(path).name != "generic" or path.suffix in {".txt", ".md"}:
                yield path

    def _display_path(self, path: Path) -> str:
        if self.root.is_dir():
            try:
                return str(path.relative_to(self.root))
            except ValueError:
                return str(path)
        return path.name

    def build(self) -> List[Scope]:
        scopes: List[Scope] = []
        for path in self.iter_files():
            adapter = adapter_for_path(path)
            if adapter.name == "generic" and path.suffix in {".txt", ".md"}:
                continue
            try:
                text = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                continue
            scopes.extend(_extract_scopes(path, self._display_path(path), text, adapter))
        return scopes

    def index_json(self) -> str:
        return json.dumps({"root": str(self.root), "scopes": [s.to_index_item() for s in self.build()]}, ensure_ascii=False, indent=2)

    def find_scope(self, scope_query: str) -> Scope:
        scopes = self.build()
        for scope in scopes:
            if scope.id == scope_query:
                return scope
        matches = [scope for scope in scopes if scope.name == scope_query]
        if len(matches) == 1:
            return matches[0]
        if len(matches) > 1:
            ids = ", ".join(scope.id for scope in matches)
            raise ValueError(f"ambiguous scope name '{scope_query}'; use one of: {ids}")
        raise KeyError(f"scope not found: {scope_query}")

    def expand(self, scope_id: str, compactness: str = "symbol") -> CompactScope:
        scope = self.find_scope(scope_id)
        source = Path(scope.path).read_text(encoding="utf-8")[scope.start_offset:scope.end_offset]
        return compact_scope(scope, source, adapter_for_path(scope.path), compactness=compactness)

    def full(self, scope_id: str) -> dict:
        scope = self.find_scope(scope_id)
        source = Path(scope.path).read_text(encoding="utf-8")[scope.start_offset:scope.end_offset]
        return {"scope": scope.to_index_item(), "code": source}


def _line_for_offset(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def _line_start_offsets(text: str) -> List[int]:
    offsets = [0]
    for m in re.finditer("\n", text):
        offsets.append(m.end())
    return offsets


def _python_scope_end(text: str, start: int, indent: int) -> int:
    line_starts = _line_start_offsets(text)
    start_line_idx = max(i for i, off in enumerate(line_starts) if off <= start)
    for off in line_starts[start_line_idx + 1:]:
        line_end = text.find("\n", off)
        if line_end == -1:
            line_end = len(text)
        line = text[off:line_end]
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        current_indent = len(line) - len(line.lstrip(" \t"))
        if current_indent <= indent:
            return off
    return len(text)


def _scope_end(text: str, start: int, indent: int, next_start: Union[int, None], adapter: LanguageAdapter) -> int:
    if adapter.name == "python":
        return _python_scope_end(text, start, indent)
    candidate = _balanced_brace_end(text, start)
    if candidate > start:
        return candidate
    return next_start if next_start is not None else len(text)


def _balanced_brace_end(text: str, start: int) -> int:
    open_at = text.find("{", start)
    if open_at == -1:
        return -1
    depth = 0
    in_string: Union[str, None] = None
    escape = False
    i = open_at
    while i < len(text):
        ch = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if in_string:
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == in_string:
                in_string = None
            i += 1
            continue
        if ch == "/" and nxt == "/":
            i += 2
            while i < len(text) and text[i] != "\n":
                i += 1
            continue
        if ch == "/" and nxt == "*":
            i += 2
            while i + 1 < len(text) and not (text[i] == "*" and text[i + 1] == "/"):
                i += 1
            i = min(len(text), i + 2)
            continue
        if ch in {'"', "'", "`"}:
            in_string = ch
        elif ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return len(text)


def _safe_scope_path(display_path: str) -> str:
    return display_path.replace("\\", "/")


def _extract_scopes(path: Path, display_path: str, text: str, adapter: LanguageAdapter) -> List[Scope]:
    matches: List[Tuple[int, int, str]] = []
    for pattern in adapter.scope_patterns:
        for m in pattern.finditer(text):
            indent_text = m.groupdict().get("indent") or ""
            matches.append((m.start(), len(indent_text), m.group("name")))
    matches = sorted(set(matches), key=lambda x: x[0])
    scopes: List[Scope] = []
    safe_path = _safe_scope_path(display_path)
    for idx, (start, indent, name) in enumerate(matches):
        next_start = matches[idx + 1][0] if idx + 1 < len(matches) else None
        end = _scope_end(text, start, indent, next_start, adapter)
        line = _line_for_offset(text, start)
        end_line = _line_for_offset(text, end)
        scope_id = f"{safe_path}:{name}:{line}"
        scopes.append(Scope(scope_id, name, str(path), adapter.name, line, end_line, start, end))
    if not scopes and text.strip():
        scopes.append(Scope(f"{safe_path}:file:1", Path(display_path).name, str(path), adapter.name, 1, _line_for_offset(text, len(text)), 0, len(text), "file"))
    return scopes


def compact_scope(scope: Scope, source: str, adapter: LanguageAdapter, compactness: str = "symbol") -> CompactScope:
    compactness = compactness.lower()
    if compactness not in {"light", "symbol"}:
        raise ValueError("compactness must be light or symbol")
    if compactness == "light":
        compact = _minify_python_layout(source) if adapter.name == "python" else _minify_non_python_whitespace(source)
        symbol_map = SymbolMap(scope.id, {})
    else:
        symbol_map = build_symbol_map(scope, source, adapter)
        compact = apply_symbol_map(source, symbol_map, adapter)
        compact = _minify_python_layout(compact) if adapter.name == "python" else _minify_non_python_whitespace(compact)
    raw = len(source)
    clen = len(compact)
    savings = 0.0 if raw == 0 else round((raw - clen) / raw * 100, 2)
    return CompactScope(scope, compact, symbol_map, raw, clen, savings, compactness)


def build_symbol_map(scope: Scope, source: str, adapter: LanguageAdapter) -> SymbolMap:
    names: List[str] = []
    if scope.name and _renamable_identifier(scope.name, adapter) and scope.kind != "file":
        names.append(scope.name)
    for token in _identifier_tokens_for_mapping(source, adapter):
        if token not in names:
            names.append(token)
    mapping: Dict[str, str] = {}
    var_index = 0
    for name in names:
        if name == scope.name and scope.kind != "file":
            mapping[name] = "f1"
        else:
            mapping[name] = _short_symbol(var_index)
            var_index += 1
    return SymbolMap(scope.id, mapping)


def _renamable_identifier(token: str, adapter: LanguageAdapter) -> bool:
    if token in adapter.keywords or keyword.iskeyword(token) or token in _BUILTIN_NAMES:
        return False
    return bool(adapter.identifier_pattern.fullmatch(token))


def _identifier_tokens_for_mapping(source: str, adapter: LanguageAdapter) -> List[str]:
    if adapter.name == "python":
        return _python_identifier_tokens(source, adapter)
    return _generic_identifier_tokens(source, adapter)


def _python_identifier_tokens(source: str, adapter: LanguageAdapter) -> List[str]:
    names: List[str] = []
    previous_significant = None
    try:
        stream = io.StringIO(source).readline
        for tok in tokenize.generate_tokens(stream):
            token_type, token_text = tok.type, tok.string
            if token_type == tokenize.NAME:
                if previous_significant == ".":
                    previous_significant = token_text
                    continue
                if _renamable_identifier(token_text, adapter) and token_text not in names:
                    names.append(token_text)
                previous_significant = token_text
            elif token_type not in {tokenize.NL, tokenize.NEWLINE, tokenize.INDENT, tokenize.DEDENT, tokenize.COMMENT, tokenize.ENCODING}:
                previous_significant = token_text
    except tokenize.TokenError:
        return _generic_identifier_tokens(source, adapter)
    return names


def _generic_identifier_tokens(source: str, adapter: LanguageAdapter) -> List[str]:
    names: List[str] = []
    for match in adapter.identifier_pattern.finditer(_mask_strings_and_comments(source)):
        token = match.group(0)
        if match.start() > 0 and source[match.start() - 1] == ".":
            continue
        if _renamable_identifier(token, adapter) and token not in names:
            names.append(token)
    return names


def _short_symbol(i: int) -> str:
    alphabet = "abcdefghijklmnopqrstuvwxyz"
    if i < len(alphabet):
        return alphabet[i]
    return f"v{i - len(alphabet) + 1}"


def apply_symbol_map(source: str, symbol_map: SymbolMap, adapter: Union[LanguageAdapter, None] = None) -> str:
    adapter = adapter or adapter_for_path("x.py")
    if adapter.name == "python":
        return _replace_python_names(source, symbol_map.symbols)
    return _replace_generic_names(source, symbol_map.symbols)


def restore_symbols(compact_code: str, symbol_map: SymbolMap, adapter: Union[LanguageAdapter, None] = None, compactness: str = "symbol") -> str:
    adapter = adapter or adapter_for_path("x.py")
    compactness = compactness.lower()
    reverse = symbol_map.reverse
    if compactness == "light" and not reverse:
        return compact_code.strip() + "\n"
    _reject_unmapped_compact_symbols(compact_code, reverse, adapter)
    if adapter.name == "python":
        return _replace_python_names(compact_code, reverse).strip() + "\n"
    out = _replace_generic_names(compact_code, reverse)
    return out.strip() + "\n"


def restore_from_payload(payload: dict) -> dict:
    scope_id = payload.get("scope_id") or payload.get("scope", {}).get("id", "unknown")
    reverse = payload.get("reverse") or payload.get("symbol_map", {}).get("reverse") or {}
    symbols = {original: short for short, original in reverse.items()}
    if not symbols and "symbols" in payload:
        symbols = dict(payload["symbols"])
    compact_code = payload.get("compact_code") or payload.get("code") or payload.get("patch") or ""
    language = payload.get("language") or payload.get("scope", {}).get("language") or "python"
    compactness = payload.get("compactness") or ("light" if not symbols else "symbol")
    adapter = _adapter_for_language_name(language)
    restored = restore_symbols(compact_code, SymbolMap(scope_id, symbols), adapter, compactness=compactness)
    return {"scope_id": scope_id, "restored_code": restored}


def _adapter_for_language_name(language: str) -> LanguageAdapter:
    if language == "python":
        return adapter_for_path("x.py")
    if language == "rust":
        return adapter_for_path("x.rs")
    if language == "go":
        return adapter_for_path("x.go")
    if language == "javascript-typescript":
        return adapter_for_path("x.ts")
    if language == "c-family":
        return adapter_for_path("x.cpp")
    return adapter_for_path("x.py")


def _replace_python_names(source: str, mapping: Dict[str, str]) -> str:
    previous_significant = None
    out_tokens = []
    try:
        for tok in tokenize.generate_tokens(io.StringIO(source).readline):
            token_type, token_text, start, end, line = tok
            replacement = token_text
            if token_type == tokenize.NAME and previous_significant != "." and token_text in mapping:
                replacement = mapping[token_text]
            out_tokens.append((token_type, replacement, start, end, line))
            if token_type not in {tokenize.NL, tokenize.NEWLINE, tokenize.INDENT, tokenize.DEDENT, tokenize.COMMENT, tokenize.ENCODING}:
                previous_significant = token_text
        return tokenize.untokenize(out_tokens)
    except tokenize.TokenError:
        return _replace_generic_names(source, mapping)


def _replace_generic_names(source: str, mapping: Dict[str, str]) -> str:
    masked = _mask_strings_and_comments(source)
    if not mapping:
        return source
    pattern = re.compile(r"\b(" + "|".join(re.escape(k) for k in sorted(mapping, key=len, reverse=True)) + r")\b")
    def repl(match: re.Match[str]) -> str:
        start = match.start()
        if start > 0 and source[start - 1] == ".":
            return match.group(0)
        return mapping[match.group(0)]
    pieces: List[str] = []
    last = 0
    for match in pattern.finditer(masked):
        pieces.append(source[last:match.start()])
        pieces.append(repl(match))
        last = match.end()
    pieces.append(source[last:])
    return "".join(pieces)


def _reject_unmapped_compact_symbols(compact_code: str, reverse: Dict[str, str], adapter: LanguageAdapter) -> None:
    allowed = set(reverse) | {"def", "return", "for", "in", "if", "else", "while", "class", "async", "await"}
    tokens = _python_code_tokens(compact_code) if adapter.name == "python" else _generic_code_tokens(compact_code)
    for token in tokens:
        if _looks_compact_symbol(token) and token not in allowed:
            raise ValueError(f"unmapped compact symbol: {token}")


def _python_code_tokens(source: str) -> List[str]:
    tokens: List[str] = []
    previous_significant = None
    try:
        for tok in tokenize.generate_tokens(io.StringIO(source).readline):
            if tok.type == tokenize.NAME:
                if previous_significant != ".":
                    tokens.append(tok.string)
                previous_significant = tok.string
            elif tok.type not in {tokenize.NL, tokenize.NEWLINE, tokenize.INDENT, tokenize.DEDENT, tokenize.COMMENT, tokenize.ENCODING}:
                previous_significant = tok.string
    except tokenize.TokenError:
        return _generic_code_tokens(source)
    return tokens


def _generic_code_tokens(source: str) -> List[str]:
    masked = _mask_strings_and_comments(source)
    return [m.group(0) for m in re.finditer(r"\b[A-Za-z_][A-Za-z0-9_]*\b", masked)]


def _looks_compact_symbol(token: str) -> bool:
    return bool(re.fullmatch(r"(?:f\d+|[a-z]|v\d+)", token))


def _mask_strings_and_comments(source: str) -> str:
    chars = list(source)
    i = 0
    while i < len(chars):
        ch = chars[i]
        if ch == "#":
            while i < len(chars) and chars[i] != "\n":
                chars[i] = " "
                i += 1
            continue
        if ch in {'"', "'", "`"}:
            quote = ch
            triple = i + 2 < len(chars) and chars[i:i+3] == [quote, quote, quote]
            end_quote_len = 3 if triple else 1
            for _ in range(end_quote_len):
                chars[i] = " "
                i += 1
            while i < len(chars):
                if not triple and chars[i] == "\\":
                    chars[i] = " "
                    i += 1
                    if i < len(chars):
                        chars[i] = " "
                        i += 1
                    continue
                if triple and i + 2 < len(chars) and chars[i:i+3] == [quote, quote, quote]:
                    for _ in range(3):
                        chars[i] = " "
                        i += 1
                    break
                if not triple and chars[i] == quote:
                    chars[i] = " "
                    i += 1
                    break
                chars[i] = " "
                i += 1
            continue
        i += 1
    return "".join(chars)


def _minify_python_layout(source: str) -> str:
    # Python is indentation-sensitive and string-literal-sensitive.  Keep line
    # structure intact and let deterministic symbol shortening provide the
    # token savings; trimming only trailing/blank whitespace avoids semantic
    # corruption such as rewriting string contents or flattening blocks.
    lines = [line.rstrip() for line in source.splitlines() if line.strip()]
    return "\n".join(lines).strip()


def _minify_non_python_whitespace(source: str) -> str:
    out: List[str] = []
    i = 0
    in_string: Union[str, None] = None
    escape = False
    pending_space = False
    compact_ops = set("{}()[],;:+-*/=<>")
    while i < len(source):
        ch = source[i]
        nxt = source[i + 1] if i + 1 < len(source) else ""
        if in_string:
            out.append(ch)
            if escape:
                escape = False
            elif ch == "\\":
                escape = True
            elif ch == in_string:
                in_string = None
            i += 1
            continue
        if ch in {'"', "'", "`"}:
            if pending_space and _needs_space_before(out, ch):
                out.append(" ")
            pending_space = False
            in_string = ch
            out.append(ch)
            i += 1
            continue
        if ch == "/" and nxt == "/":
            while i < len(source) and source[i] != "\n":
                i += 1
            pending_space = True
            continue
        if ch == "/" and nxt == "*":
            i += 2
            while i + 1 < len(source) and not (source[i] == "*" and source[i + 1] == "/"):
                i += 1
            i = min(len(source), i + 2)
            pending_space = True
            continue
        if ch.isspace():
            pending_space = True
            i += 1
            continue
        if ch in compact_ops:
            while out and out[-1] == " ":
                out.pop()
            out.append(ch)
            pending_space = False
            i += 1
            continue
        if pending_space and _needs_space_before(out, ch):
            out.append(" ")
        pending_space = False
        out.append(ch)
        i += 1
    return "".join(out).strip()


def _needs_space_before(out: List[str], ch: str) -> bool:
    if not out:
        return False
    prev = out[-1]
    return (prev.isalnum() or prev in {"_", "$"}) and (ch.isalnum() or ch in {"_", "$"})


def _readable_format(code: str) -> str:
    code = re.sub(r"\s*([{};])\s*", r"\1\n", code.strip())
    code = re.sub(r"\s*,\s*", ", ", code)
    code = re.sub(r"\s*([=+\-*/<>])\s*", r" \1 ", code)
    lines = [line.strip() for line in code.splitlines() if line.strip()]
    indent = 0
    formatted: List[str] = []
    for line in lines:
        if line.startswith("}"):
            indent = max(0, indent - 1)
        formatted.append("  " * indent + line)
        if line.endswith("{"):
            indent += 1
    return "\n".join(formatted) + ("\n" if formatted else "")


_BUILTIN_NAMES = frozenset({
    "print", "len", "range", "str", "int", "float", "bool", "list", "dict", "set", "tuple", "sum", "min", "max", "open",
    "Exception", "ValueError", "KeyError", "True", "False", "None", "self", "cls",
})
