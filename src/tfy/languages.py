from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import re
from typing import List, Tuple, Union


@dataclass(frozen=True)
class LanguageAdapter:
    name: str
    extensions: Tuple[str, ...]
    scope_patterns: tuple[re.Pattern[str], ...]
    identifier_pattern: re.Pattern[str]
    keywords: frozenset[str]

    def matches(self, path: Union[str, Path]) -> bool:
        suffix = Path(path).suffix.lower()
        return suffix in self.extensions


COMMON_KEYWORDS = frozenset(
    "if else for while return break continue switch case default try catch finally throw new class struct enum interface "
    "import from export const let var def fn func function pub private protected static async await yield true false null none nil "
    "self this super in is and or not as with match where impl trait use mod crate type sizeof typeof do go package public void int float double char bool string".split()
)

IDENT = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")

PYTHON = LanguageAdapter(
    name="python",
    extensions=(".py",),
    scope_patterns=(re.compile(r"^(?P<indent>[ \t]*)(?:async\s+)?def\s+(?P<name>[A-Za-z_][\w]*)\s*\([^\n]*\)\s*:", re.M),
                    re.compile(r"^(?P<indent>[ \t]*)class\s+(?P<name>[A-Za-z_][\w]*)\b[^\n]*:", re.M)),
    identifier_pattern=IDENT,
    keywords=COMMON_KEYWORDS | frozenset("elif except lambda global nonlocal pass raise assert del yield".split()),
)

JS_TS = LanguageAdapter(
    name="javascript-typescript",
    extensions=(".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs"),
    scope_patterns=(
        re.compile(r"^(?P<indent>[ \t]*)(?:export\s+)?(?:async\s+)?function\s+(?P<name>[A-Za-z_$][\w$]*)\s*\(", re.M),
        re.compile(r"^(?P<indent>[ \t]*)(?:export\s+)?(?:const|let|var)\s+(?P<name>[A-Za-z_$][\w$]*)\s*=\s*(?:async\s*)?\([^\n]*\)\s*=>", re.M),
        re.compile(r"^(?P<indent>[ \t]*)(?:export\s+)?class\s+(?P<name>[A-Za-z_$][\w$]*)\b", re.M),
    ),
    identifier_pattern=re.compile(r"\b[A-Za-z_$][A-Za-z0-9_$]*\b"),
    keywords=COMMON_KEYWORDS | frozenset("constructor extends implements get set of require module".split()),
)

RUST = LanguageAdapter(
    name="rust",
    extensions=(".rs",),
    scope_patterns=(
        re.compile(r"^(?P<indent>[ \t]*)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(?P<name>[A-Za-z_][\w]*)\s*[<(]", re.M),
        re.compile(r"^(?P<indent>[ \t]*)(?:pub\s+)?(?:struct|enum|trait|impl)\s+(?P<name>[A-Za-z_][\w]*)\b", re.M),
    ),
    identifier_pattern=IDENT,
    keywords=COMMON_KEYWORDS | frozenset("mut let ref move unsafe extern lifetime dyn async await macro_rules".split()),
)

GO = LanguageAdapter(
    name="go",
    extensions=(".go",),
    scope_patterns=(re.compile(r"^(?P<indent>[ \t]*)func\s+(?:\([^)]*\)\s*)?(?P<name>[A-Za-z_][\w]*)\s*\(", re.M),),
    identifier_pattern=IDENT,
    keywords=COMMON_KEYWORDS | frozenset("defer select chan range interface map recover panic".split()),
)

C_FAMILY = LanguageAdapter(
    name="c-family",
    extensions=(".c", ".h", ".cc", ".cpp", ".hpp", ".java", ".cs"),
    scope_patterns=(
        re.compile(r"^(?P<indent>[ \t]*)(?:public|private|protected|static|final|virtual|override|async|inline|extern|template|const|\s)*[A-Za-z_][\w:<>,\[\]\s*&]*\s+(?P<name>[A-Za-z_][\w]*)\s*\([^;{}]*\)\s*(?:const\s*)?\{", re.M),
        re.compile(r"^(?P<indent>[ \t]*)(?:public\s+)?(?:class|struct|interface|enum)\s+(?P<name>[A-Za-z_][\w]*)\b", re.M),
    ),
    identifier_pattern=IDENT,
    keywords=COMMON_KEYWORDS | frozenset("namespace using template operator constexpr volatile synchronized throws".split()),
)

ADAPTERS = (PYTHON, JS_TS, RUST, GO, C_FAMILY)


def adapter_for_path(path: Union[str, Path]) -> LanguageAdapter:
    for adapter in ADAPTERS:
        if adapter.matches(path):
            return adapter
    return LanguageAdapter("generic", tuple(), (re.compile(r"^(?P<indent>[ \t]*)(?P<name>[A-Za-z_][\w]*)\s*\([^\n]*\)", re.M),), IDENT, COMMON_KEYWORDS)


def supported_languages() -> List[str]:
    return [a.name for a in ADAPTERS]
