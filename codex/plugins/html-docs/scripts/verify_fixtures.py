#!/usr/bin/env python3
"""Run html-docs fixture conversions and assert core output properties."""

from __future__ import annotations

import base64
import importlib.util
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


PLUGIN_ROOT = Path(__file__).resolve().parents[1]
CONVERTER = PLUGIN_ROOT / "scripts" / "markdown_to_html.py"
FIXTURES = PLUGIN_ROOT / "skills" / "html-docs" / "references" / "fixtures"

# Smallest valid 1x1 transparent PNG, used as a real local image fixture.
_MIN_PNG_B64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="


def _load_converter():
    spec = importlib.util.spec_from_file_location("markdown_to_html", CONVERTER)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def check_render_image_ref_security(failures: list[str]) -> None:
    converter = _load_converter()
    with tempfile.TemporaryDirectory(prefix="html-docs-imgref-") as tmpdir:
        base_dir = Path(tmpdir) / "docs"
        base_dir.mkdir()
        outside_dir = Path(tmpdir) / "outside"
        outside_dir.mkdir()

        valid_png = base_dir / "photo.png"
        valid_png.write_bytes(base64.b64decode(_MIN_PNG_B64))

        disallowed_mime = base_dir / "notes.txt"
        disallowed_mime.write_text("not an image", encoding="utf-8")

        secret_bytes = b"top secret"
        secret_outside = outside_dir / "secret.txt"
        secret_outside.write_bytes(secret_bytes)
        secret_b64 = base64.b64encode(secret_bytes).decode("ascii")

        cases = {
            "valid local image": (valid_png.name, True),
            "relative traversal": ("../outside/secret.txt", False),
            "absolute path read": (str(secret_outside), False),
            "disallowed MIME": (disallowed_mime.name, False),
        }
        for label, (target, should_embed) in cases.items():
            result = converter.render_image_ref("alt", target, base_dir)
            embedded = ";base64," in result and "<img " in result
            if embedded != should_embed:
                failures.append(
                    f"render_image_ref[{label}]: expected embed={should_embed}, got embed={embedded} ({result[:160]!r})"
                )
            if secret_b64 in result:
                failures.append(f"render_image_ref[{label}]: leaked outside-base_dir file contents into output")


def check_sanitize_href_security(failures: list[str]) -> None:
    converter = _load_converter()
    nul_bypass = converter.sanitize_href("java\x00script:alert(1)")
    if nul_bypass is not None:
        failures.append(f"sanitize_href[NUL scheme bypass]: expected None, got {nul_bypass!r}")

    valid_https = converter.sanitize_href("https://example.com/page")
    if valid_https != "https://example.com/page":
        failures.append(f"sanitize_href[valid https]: expected passthrough, got {valid_https!r}")

    valid_relative = converter.sanitize_href("./page.html")
    if valid_relative != "./page.html":
        failures.append(f"sanitize_href[valid relative link]: expected passthrough, got {valid_relative!r}")


def main() -> int:
    failures: list[str] = []
    check_render_image_ref_security(failures)
    check_sanitize_href_security(failures)
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
