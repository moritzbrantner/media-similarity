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

IMPORT_RE = re.compile(r"\b(?:pub\s+)?use\s+([A-Za-z0-9_]+)")


def main() -> int:
    violations: list[str] = []
    for path in sorted(DOMAIN_ROOT.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for line_number, line in enumerate(text.splitlines(), start=1):
            match = IMPORT_RE.search(line)
            if not match:
                continue
            crate = match.group(1)
            if crate.startswith(FORBIDDEN_IMPORTS):
                relative = path.relative_to(ROOT)
                violations.append(f"{relative}:{line_number}: implementation import `{crate}`")

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
