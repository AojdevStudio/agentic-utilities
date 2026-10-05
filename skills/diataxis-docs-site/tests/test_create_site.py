#!/usr/bin/env python3
"""Regression tests for the bundled Diátaxis site generator."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SKILL_ROOT = Path(__file__).resolve().parent.parent
CREATE_SITE = SKILL_ROOT / "scripts" / "create_site.py"
STARTER = SKILL_ROOT / "assets" / "aoj-starlight"


class CreateSiteTests(unittest.TestCase):
    def test_generated_site_covers_locked_undici_node_minimum(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            subprocess.run(
                [
                    sys.executable,
                    str(CREATE_SITE),
                    "--repo-root", temp_dir,
                    "--project-name", "Engine contract",
                    "--description", "Example documentation",
                    "--repository-url", "https://github.com/example/project",
                    "--site-url", "https://example.github.io/project",
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            site = Path(temp_dir) / "docs-site"
            manifest = json.loads((site / "package.json").read_text(encoding="utf-8"))
            lockfile = json.loads((site / "package-lock.json").read_text(encoding="utf-8"))
            declared = manifest["engines"]["node"]
            required = lockfile["packages"]["node_modules/undici"]["engines"]["node"]
            self.assertEqual(declared, lockfile["packages"][""]["engines"]["node"])
            declared_minimum = tuple(map(int, declared.removeprefix(">=").split(".")))
            required_minimum = tuple(map(int, required.removeprefix(">=").split(".")))
            self.assertGreaterEqual(declared_minimum, required_minimum)

    def test_yaml_safe_project_name_and_generated_gitignore(self) -> None:
        project_name = "Example: Project"
        expected_descriptions = {
            "tutorials": f"Learn {project_name} through guided lessons.",
            "how-to": f"Complete specific tasks with {project_name}.",
            "reference": f"Look up exact {project_name} behavior and configuration.",
            "explanation": f"Understand how and why {project_name} works.",
        }

        with tempfile.TemporaryDirectory() as temp_dir:
            repo_root = Path(temp_dir)
            subprocess.run(
                [
                    sys.executable,
                    str(CREATE_SITE),
                    "--repo-root",
                    str(repo_root),
                    "--project-name",
                    project_name,
                    "--description",
                    "Example documentation",
                    "--repository-url",
                    "https://github.com/example/project",
                    "--site-url",
                    "https://example.github.io/project",
                    "--base-path",
                    "/project/",
                ],
                check=True,
                capture_output=True,
                text=True,
            )

            docs_root = repo_root / "docs-site" / "src" / "content" / "docs"
            for section, expected in expected_descriptions.items():
                content = (docs_root / section / "index.md").read_text(encoding="utf-8")
                frontmatter = content.split("---", 2)[1]
                description = next(
                    line.removeprefix("description:").strip()
                    for line in frontmatter.splitlines()
                    if line.startswith("description:")
                )
                self.assertEqual(json.loads(description), expected)

            self.assertTrue((repo_root / "docs-site" / ".gitignore").is_file())
            self.assertFalse((repo_root / "docs-site" / "gitignore.template").exists())

        self.assertTrue((STARTER / "gitignore.template").is_file())
        self.assertFalse((STARTER / ".gitignore").exists())


if __name__ == "__main__":
    unittest.main()
