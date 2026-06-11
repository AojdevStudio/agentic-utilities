#!/usr/bin/env python3
"""Run html-docs fixture conversions and assert core output properties."""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


PLUGIN_ROOT = Path(__file__).resolve().parents[1]
CONVERTER = PLUGIN_ROOT / "scripts" / "markdown_to_html.py"
FIXTURES = PLUGIN_ROOT / "skills" / "html-docs" / "references" / "fixtures"


def main() -> int:
    failures: list[str] = []
    with tempfile.TemporaryDirectory(prefix="html-docs-fixtures-") as tmpdir:
        temp_root = Path(tmpdir)
        for fixture in sorted(FIXTURES.glob("*.md")):
            target = temp_root / fixture.name
            shutil.copy2(fixture, target)
            result = subprocess.run(
                [sys.executable, str(CONVERTER), str(target), "--force", "--quiet"],
                text=True,
                capture_output=True,
                check=False,
            )
            if result.returncode != 0:
                failures.append(f"{fixture.name}: converter exited {result.returncode}: {result.stderr.strip()}")
                continue
            output = target.with_suffix(".html")
            if not output.is_file():
                failures.append(f"{fixture.name}: adjacent HTML was not created")
                continue
            html_text = output.read_text(encoding="utf-8")
            required = [
                "<!doctype html>",
                "Copy markdown",
                "Generated from",
                "source-markdown",
                "class=\"content doc-type-",
            ]
            for marker in required:
                if marker not in html_text:
                    failures.append(f"{fixture.name}: missing marker {marker!r}")
            forbidden = ["<script src=", "<link rel=\"stylesheet\"", "https://cdn.", "http://cdn."]
            lower = html_text.lower()
            for marker in forbidden:
                if marker in lower:
                    failures.append(f"{fixture.name}: contains external dependency marker {marker!r}")
    if failures:
        print("html-docs fixture verification failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print("html-docs fixture verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
