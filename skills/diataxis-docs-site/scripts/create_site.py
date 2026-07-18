#!/usr/bin/env python3
"""Instantiate the fixed AOJ Starlight docs starter without overwriting files."""

from __future__ import annotations

import argparse
import json
import re
import shutil
from pathlib import Path


SKILL_ROOT = Path(__file__).resolve().parent.parent
STARTER = SKILL_ROOT / "assets" / "aoj-starlight"
WORKFLOW = SKILL_ROOT / "assets" / "github-pages.yml"
TOKEN_PATTERN = re.compile(r"__[A-Z0-9_]+__")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--docs-dir", default="docs-site")
    parser.add_argument("--project-name", required=True)
    parser.add_argument("--description", required=True)
    parser.add_argument("--repository-url", required=True)
    parser.add_argument("--site-url", required=True)
    parser.add_argument("--base-path", default="/")
    parser.add_argument("--default-branch", default="main")
    args = parser.parse_args()

    for label, value in {
        "docs directory": args.docs_dir,
        "default branch": args.default_branch,
    }.items():
        if not re.fullmatch(r"[A-Za-z0-9._/-]+", value) or ".." in value:
            raise ValueError(f"invalid {label}: {value!r}")

    text_values = {
        "__PROJECT_NAME_TEXT__": args.project_name,
        "__DOCS_DIR__": args.docs_dir.strip("/"),
        "__DEFAULT_BRANCH__": args.default_branch,
    }
    if any("\n" in value or "\r" in value for value in text_values.values()):
        raise ValueError("template values must be single-line")
    if Path(args.docs_dir).is_absolute():
        raise ValueError("docs directory must be relative to the repository")
    if not args.base_path.startswith("/") or ".." in args.base_path:
        raise ValueError("base path must be an absolute URL path without '..'")

    repo_root = args.repo_root.resolve()
    target = repo_root / args.docs_dir
    workflow_target = repo_root / ".github" / "workflows" / "docs.yml"
    occupied = [path for path in (target, workflow_target) if path.exists()]
    if occupied:
        raise FileExistsError("refusing to overwrite: " + ", ".join(map(str, occupied)))

    shutil.copytree(STARTER, target)
    (target / "gitignore.template").replace(target / ".gitignore")
    workflow_target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(WORKFLOW, workflow_target)

    replacements = {
        **text_values,
        "__PROJECT_NAME_JSON__": json.dumps(args.project_name),
        "__DESCRIPTION_JSON__": json.dumps(args.description),
        "__REPOSITORY_URL_JSON__": json.dumps(args.repository_url.rstrip("/")),
        "__SITE_URL_JSON__": json.dumps(args.site_url.rstrip("/")),
        "__BASE_PATH_JSON__": json.dumps(args.base_path),
        "__TUTORIAL_DESCRIPTION_JSON__": json.dumps(
            f"Learn {args.project_name} through guided lessons."
        ),
        "__HOW_TO_DESCRIPTION_JSON__": json.dumps(
            f"Complete specific tasks with {args.project_name}."
        ),
        "__REFERENCE_DESCRIPTION_JSON__": json.dumps(
            f"Look up exact {args.project_name} behavior and configuration."
        ),
        "__EXPLANATION_DESCRIPTION_JSON__": json.dumps(
            f"Understand how and why {args.project_name} works."
        ),
    }

    files = [path for path in target.rglob("*") if path.is_file()] + [workflow_target]
    for path in files:
        content = path.read_text(encoding="utf-8")
        for token, value in replacements.items():
            content = content.replace(token, value)
        unresolved = TOKEN_PATTERN.findall(content)
        if unresolved:
            raise RuntimeError(f"unresolved tokens in {path}: {sorted(set(unresolved))}")
        path.write_text(content, encoding="utf-8")

    print(target)
    print(workflow_target)


if __name__ == "__main__":
    main()
