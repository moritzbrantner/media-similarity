#!/usr/bin/env python3
"""Keep media-similarity domain code independent from capability implementations."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOMAIN_ROOT = ROOT / "backend" / "src" / "domain"
CARGO_MANIFEST = ROOT / "backend" / "Cargo.toml"
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")

FORBIDDEN_IMPORTS = (
    "audio_analysis_",
    "image_analysis_",
    "video_analysis_",
    "text_transcripts",
    "text_analysis_core",
    "model_runtime",
    "runtime_onnx",
    "vector_analysis_core",
)

# Comments and literals are blanked out before scanning so prose and strings never
# count as dependencies. Line structure is preserved for accurate reporting.
RAW_STRING_RE = re.compile(r'[bc]?r(#*)"')
QUOTED_STRING_RE = re.compile(r'[bc]?"(?:\\.|[^"\\])*"', re.DOTALL)
CHAR_LITERAL_RE = re.compile(r"b?'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]+\}|.)|[^'\\\n])'")

IDENT_RE = re.compile(r"(?<![A-Za-z0-9_])(?:r#)?([A-Za-z_][A-Za-z0-9_]*)")
USE_RE = re.compile(r"\b(?:use|extern\s+crate)\b[^;]*;", re.DOTALL)
PATH_ROOT_RE = re.compile(r"(?<![A-Za-z0-9_])(?:r#)?([A-Za-z_][A-Za-z0-9_]*)\s*::")
# Rust keywords other than the path roots `crate`, `self`, `super` and `Self`. A `::`
# that follows one of these starts a root-qualified (external crate) path.
NON_PATH_KEYWORDS = frozenset(
    {
        "abstract", "as", "async", "await", "become", "box", "break", "const", "continue",
        "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if",
        "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override",
        "priv", "pub", "ref", "return", "static", "struct", "trait", "true", "try", "type",
        "typeof", "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
    }
)


def blank_span(chars: list[str], start: int, end: int) -> None:
    for index in range(start, end):
        if chars[index] != "\n":
            chars[index] = " "


def block_comment_end(text: str, start: int) -> int:
    """Return the index after a (possibly nested) block comment starting at `start`."""
    depth = 0
    index = start
    while index < len(text):
        if text.startswith("/*", index):
            depth += 1
            index += 2
        elif text.startswith("*/", index):
            depth -= 1
            index += 2
            if depth == 0:
                return index
        else:
            index += 1
    return len(text)


def strip_comments_and_literals(text: str) -> str:
    chars = list(text)
    index = 0
    while index < len(text):
        previous = text[index - 1] if index else ""
        identifier_char = previous.isalnum() or previous == "_"
        if text.startswith("//", index):
            end = text.find("\n", index)
            end = len(text) if end == -1 else end
        elif text.startswith("/*", index):
            end = block_comment_end(text, index)
        elif not identifier_char and (raw := RAW_STRING_RE.match(text, index)):
            closing = '"' + raw.group(1)
            found = text.find(closing, raw.end())
            end = len(text) if found == -1 else found + len(closing)
        elif not identifier_char and (quoted := QUOTED_STRING_RE.match(text, index)):
            end = quoted.end()
        elif not identifier_char and (char := CHAR_LITERAL_RE.match(text, index)):
            end = char.end()
        else:
            index += 1
            continue
        blank_span(chars, index, end)
        index = end
    return "".join(chars)


def capability_crate_aliases(manifest: Path = CARGO_MANIFEST) -> frozenset[str]:
    """Rust names under which Cargo exposes forbidden capability packages.

    A dependency key may rename a package (`rt = { package = "moenarch-runtime-onnx" }`),
    so the forbidden identifiers are derived from the declared package names.
    """
    if not manifest.exists():
        return frozenset()
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    tables = [data.get(name, {}) for name in DEPENDENCY_TABLES]
    for target in data.get("target", {}).values():
        tables.extend(target.get(name, {}) for name in DEPENDENCY_TABLES)

    aliases: set[str] = set()
    for table in tables:
        for key, spec in table.items():
            package = spec.get("package", key) if isinstance(spec, dict) else key
            normalized = package.removeprefix("moenarch-").replace("-", "_")
            if normalized.startswith(FORBIDDEN_IMPORTS):
                aliases.add(key.replace("-", "_"))
    return frozenset(aliases)


def is_forbidden(name: str, aliases: frozenset[str] = frozenset()) -> bool:
    return name.startswith(FORBIDDEN_IMPORTS) or name in aliases


def preceded_by_relative_path(text: str, start: int) -> bool:
    """True when the identifier is a later segment of a crate/self/super path."""
    prefix = text[:start].rstrip()
    if not prefix.endswith("::"):
        return False
    before = prefix[:-2].rstrip()
    segment = re.search(r"([A-Za-z_][A-Za-z0-9_]*)$", before)
    # Leading `::name` (after punctuation or a keyword) is a root-qualified external crate path.
    return segment is not None and segment.group(1) not in NON_PATH_KEYWORDS


def violations_in(text: str, aliases: frozenset[str] = frozenset()) -> list[tuple[int, str]]:
    code = strip_comments_and_literals(text)
    found: dict[int, tuple[int, str]] = {}

    def record(offset: int, name: str) -> None:
        line = code.count("\n", 0, offset) + 1
        found.setdefault(offset, (line, name))

    # Any path whose first segment is a capability crate: fully qualified uses,
    # root-qualified `::crate::Item`, and nested/grouped import trees.
    for match in PATH_ROOT_RE.finditer(code):
        name = match.group(1)
        if is_forbidden(name, aliases) and not preceded_by_relative_path(code, match.start(1)):
            record(match.start(1), name)

    # Bare crate imports such as `use model_runtime;`, `use {runtime_onnx as rt};`,
    # or `extern crate image_analysis_core;`.
    for statement in USE_RE.finditer(code):
        body = statement.group(0)
        for ident in IDENT_RE.finditer(body):
            name = ident.group(1)
            offset = statement.start() + ident.start(1)
            if is_forbidden(name, aliases) and not preceded_by_relative_path(code, offset):
                record(offset, name)

    return sorted(found.values())


def main() -> int:
    violations: list[str] = []
    aliases = capability_crate_aliases()
    for path in sorted(DOMAIN_ROOT.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT)
        for line_number, crate in violations_in(text, aliases):
            violations.append(f"{relative}:{line_number}: implementation dependency `{crate}`")

    if violations:
        print("media-similarity dependency boundary violations:")
        for violation in violations:
            print(f"- {violation}")
        print(
            "Domain code must use application-owned DTOs/traits. Put concrete audio, visual, "
            "text, model, and vector integrations under an adapter such as backend/src/workers/media/."
        )
        return 1

    print("media-similarity dependency boundaries: ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
