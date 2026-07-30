"""Shared test fixtures.

Every git repository used by the suite is built inside pytest's tmp_path with
explicit user identity, so the suite never depends on the developer's global git
config and never touches a real repository.
"""

import subprocess
from pathlib import Path
from typing import Optional

import pytest


def run_git(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", *args],
        cwd=str(repo),
        check=True,
        capture_output=True,
        text=True,
    )


def initialize_repo(repo: Path) -> Path:
    repo.mkdir(parents=True, exist_ok=True)
    run_git(repo, "init", "-q")
    run_git(repo, "config", "user.email", "test@example.com")
    run_git(repo, "config", "user.name", "Changelog Test")
    run_git(repo, "config", "commit.gpgsign", "false")
    return repo


def commit(repo: Path, message: str, body: Optional[str] = None) -> None:
    tracked_file = repo / "tracked.txt"
    existing = tracked_file.read_text(encoding="utf-8") if tracked_file.exists() else ""
    tracked_file.write_text(existing + message + "\n", encoding="utf-8")
    run_git(repo, "add", "tracked.txt")

    command = ["commit", "-m", message]
    if body is not None:
        command.extend(["-m", body])
    run_git(repo, *command)


@pytest.fixture
def repo(tmp_path, monkeypatch):
    """An initialized git repo, with cwd moved into it."""
    path = initialize_repo(tmp_path / "repo")
    monkeypatch.chdir(path)
    return path


@pytest.fixture
def repo_with_remote(repo):
    run_git(repo, "remote", "add", "origin", "https://github.com/example/demo.git")
    return repo
