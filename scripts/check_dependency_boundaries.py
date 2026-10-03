#!/usr/bin/env python3
"""Keep media-similarity domain code independent from capability implementations."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOMAIN_ROOT = ROOT / "backend" / "src" / "domain"

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
STRIP_RE = re.compile(
    r"""
    //[^\n]*                          # line comment
    | /\*.*?\*/                       # block comment
    | r(?P<hashes>\#*)".*?"(?P=hashes) # raw string
    | b?"(?:\\.|[^"\\])*"             # string / byte string
    | b?'(?:\\.|[^'\\])'              # char / byte literal
    """,
    re.DOTALL | re.VERBOSE,
)

IDENT_RE = re.compile(r"(?<![A-Za-z0-9_])(?:r#)?([A-Za-z_][A-Za-z0-9_]*)")
USE_RE = re.compile(r"\b(?:use|extern\s+crate)\b[^;]*;", re.DOTALL)
PATH_ROOT_RE = re.compile(r"(?<![A-Za-z0-9_])(?:r#)?([A-Za-z_][A-Za-z0-9_]*)\s*::")
# Keywords that may directly precede a root-qualified `::crate::Item` path.
PATH_KEYWORDS = frozenset(
    {"use", "pub", "in", "as", "mut", "dyn", "impl", "where", "for", "let", "return", "type", "const", "static"}
)


def blank(match: re.Match[str]) -> str:
    return re.sub(r"[^\n]", " ", match.group(0))


def is_forbidden(name: str) -> bool:
    return name.startswith(FORBIDDEN_IMPORTS)


def preceded_by_relative_path(text: str, start: int) -> bool:
    """True when the identifier is a later segment of a crate/self/super path."""
    prefix = text[:start].rstrip()
    if not prefix.endswith("::"):
        return False
    before = prefix[:-2].rstrip()
    segment = re.search(r"([A-Za-z_][A-Za-z0-9_]*)$", before)
    # Leading `::name` (after punctuation or a keyword) is a root-qualified external crate path.
    return segment is not None and segment.group(1) not in PATH_KEYWORDS


def violations_in(text: str) -> list[tuple[int, str]]:
    code = STRIP_RE.sub(blank, text)
    found: dict[int, tuple[int, str]] = {}

    def record(offset: int, name: str) -> None:
        line = code.count("\n", 0, offset) + 1
        found.setdefault(offset, (line, name))

    # Any path whose first segment is a capability crate: fully qualified uses,
    # root-qualified `::crate::Item`, and nested/grouped import trees.
    for match in PATH_ROOT_RE.finditer(code):
        name = match.group(1)
        if is_forbidden(name) and not preceded_by_relative_path(code, match.start(1)):
            record(match.start(1), name)

    # Bare crate imports such as `use model_runtime;`, `use {runtime_onnx as rt};`,
    # or `extern crate image_analysis_core;`.
    for statement in USE_RE.finditer(code):
        body = statement.group(0)
        for ident in IDENT_RE.finditer(body):
            name = ident.group(1)
            offset = statement.start() + ident.start(1)
            if is_forbidden(name) and not preceded_by_relative_path(code, offset):
                record(offset, name)

    return sorted(found.values())


def main() -> int:
    violations: list[str] = []
    for path in sorted(DOMAIN_ROOT.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT)
        for line_number, crate in violations_in(text):
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
