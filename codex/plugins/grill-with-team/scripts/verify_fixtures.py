#!/usr/bin/env python3
"""Run grill-with-team verifier fixtures."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "references" / "fixtures" / "migration"


def main() -> int:
    command = [
        sys.executable,
        str(ROOT / "scripts" / "verify_html_migration.py"),
        "--context-md",
        str(FIXTURE / "CONTEXT.md"),
        "--context-html",
        str(FIXTURE / "CONTEXT.html"),
        "--adr-md-dir",
        str(FIXTURE / "docs" / "adr"),
        "--adr-html-dir",
        str(FIXTURE / "docs" / "adr"),
    ]
    result = subprocess.run(command, check=False)
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
