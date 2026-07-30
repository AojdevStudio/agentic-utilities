"""Regression tests for the reviewed findings.

Each test encodes behavior that was BROKEN before the corresponding fix, so the
suite fails against the pre-fix implementation and passes after it.
"""

import git
import pytest
from click.testing import CliRunner

from conftest import commit, initialize_repo, run_git

from changelog_tool import utils
from changelog_tool.cli import cli, get_commit_changelog_entry


# --------------------------------------------------------------------------
# F1: conventional-commit "!" breaking marker was parsed and then discarded
# --------------------------------------------------------------------------

@pytest.mark.parametrize(
    ("subject", "body", "expected"),
    [
        ("fix!: drop legacy path", "", True),
        ("feat(api)!: replace response shape", "", True),
        ("fix(scope)!: drop legacy path", "", True),
        ("fix: keep behavior", "BREAKING CHANGE: response changed", True),
        ("feat: add endpoint", "", False),
        ("fix: correct rounding", "", False),
    ],
)
def test_f1_breaking_marker_is_recorded(subject, body, expected):
    parsed = utils.parse_commit_message(
        {"hash": "abcdef1234", "subject": subject, "body": body, "author": "T"}
    )
    assert parsed["breaking"] is expected


def test_f1_bang_marker_drives_major_bump(repo):
    commit(repo, "fix!: drop legacy path")
    # Pre-fix this returned 1.2.4 (patch), because "!" was discarded.
    assert utils.get_next_version("1.2.3", auto_mode=True) == "2.0.0"


def test_f1_scoped_bang_marker_drives_major_bump(repo):
    commit(repo, "feat(scope)!: replace response shape")
    # Pre-fix this returned 1.3.0 (minor).
    assert utils.get_next_version("1.2.3", auto_mode=True) == "2.0.0"


def test_f1_breaking_change_footer_drives_major_bump(repo):
    commit(repo, "fix: keep behavior", body="BREAKING CHANGE: response changed")
    assert utils.get_next_version("1.2.3", auto_mode=True) == "2.0.0"


# --------------------------------------------------------------------------
# F2: --unreleased deleted a following unbracketed H2 (e.g. "## Links")
# --------------------------------------------------------------------------

def test_f2_trailing_unbracketed_section_survives(repo):
    commit(repo, "feat: add endpoint")
    changelog = repo / "CHANGELOG.md"
    changelog.write_text(
        "# Changelog\n\n## [Unreleased]\n\n- stale item\n\n## Links\n\n[unreleased]: https://example.com\n",
        encoding="utf-8",
    )

    result = CliRunner().invoke(cli, ["--unreleased"])
    assert result.exit_code == 0, result.output

    content = changelog.read_text(encoding="utf-8")
    # Pre-fix the terminator was "^## \[", so "## Links" was swallowed.
    assert "## Links" in content
    assert "[unreleased]: https://example.com" in content
    assert "add endpoint" in content


def test_f2_body_still_ends_at_a_bracketed_release_heading(repo):
    commit(repo, "feat: add endpoint")
    changelog = repo / "CHANGELOG.md"
    changelog.write_text(
        "# Changelog\n\n## [Unreleased]\n\n- stale\n\n## [1.0.0] - 2020-01-01\n\n- old release\n",
        encoding="utf-8",
    )

    result = CliRunner().invoke(cli, ["--unreleased"])
    assert result.exit_code == 0, result.output

    content = changelog.read_text(encoding="utf-8")
    assert "- old release" in content
    assert "stale" not in content


def test_f2_unreleased_rewrite_is_idempotent(repo):
    commit(repo, "feat: add endpoint")
    runner = CliRunner()
    assert runner.invoke(cli, ["--unreleased"]).exit_code == 0
    first = (repo / "CHANGELOG.md").read_text(encoding="utf-8")
    assert runner.invoke(cli, ["--unreleased"]).exit_code == 0
    second = (repo / "CHANGELOG.md").read_text(encoding="utf-8")
    assert first == second


# --------------------------------------------------------------------------
# F3: release insertion split the Unreleased heading from its body,
#     duplicating every item when --unreleased had run first
# --------------------------------------------------------------------------

def test_f3_unreleased_then_release_emits_each_item_once(repo_with_remote):
    repo = repo_with_remote
    commit(repo, "feat: add token refresh endpoint")
    commit(repo, "fix: correct pagination")

    runner = CliRunner()
    assert runner.invoke(cli, ["--unreleased"]).exit_code == 0
    result = runner.invoke(cli, ["1.1.0", "--auto", "--force"])
    assert result.exit_code == 0, result.output

    content = (repo / "CHANGELOG.md").read_text(encoding="utf-8")

    # Pre-fix each bullet appeared twice: once under the new release heading
    # (carried over as the orphaned old Unreleased body) and once regenerated.
    assert content.count("- add token refresh endpoint") == 1
    assert content.count("- correct pagination") == 1

    # And the Unreleased section must be left empty.
    span = utils.find_unreleased_span(content)
    assert span is not None
    _, body_start, body_end = span
    assert content[body_start:body_end].strip() == ""


def test_f3_release_preserves_earlier_releases(repo_with_remote):
    repo = repo_with_remote
    commit(repo, "feat: add endpoint")
    (repo / "CHANGELOG.md").write_text(
        "# Changelog\n\n## [Unreleased]\n\n## [0.9.0] - 2020-01-01\n\n### Added\n\n- older thing\n",
        encoding="utf-8",
    )

    result = CliRunner().invoke(cli, ["1.0.0", "--auto", "--force"])
    assert result.exit_code == 0, result.output

    content = (repo / "CHANGELOG.md").read_text(encoding="utf-8")
    assert "- older thing" in content
    assert "## [0.9.0]" in content
    assert content.index("## [1.0.0]") < content.index("## [0.9.0]")


# --------------------------------------------------------------------------
# F4: git failures collapsed to [] and exited 0
# --------------------------------------------------------------------------

def test_f4_outside_a_git_repo_fails_loudly(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    with pytest.raises(git.InvalidGitRepositoryError):
        utils.parse_commits()


def test_f4_cli_exits_nonzero_outside_a_git_repo(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    result = CliRunner().invoke(cli, ["--unreleased"])
    # Pre-fix this printed "No commits found" and exited 0.
    assert result.exit_code != 0
    assert not (tmp_path / "CHANGELOG.md").exists()


def test_f4_works_from_a_subdirectory(repo, monkeypatch):
    commit(repo, "feat: add endpoint")
    nested = repo / "packages" / "inner"
    nested.mkdir(parents=True)

    # monkeypatch.chdir, not os.chdir: a bare chdir leaks the working directory
    # into later tests, which then write CHANGELOG.md into the package itself.
    monkeypatch.chdir(nested)

    commits = utils.parse_commits()
    # Pre-fix git.Repo(".") from a subdirectory raised, was swallowed, and
    # returned [].
    assert [c["subject"] for c in commits] == ["add endpoint"]


def test_f4_empty_repo_is_an_empty_range_not_an_error(repo):
    assert utils.parse_commits() == []


# --------------------------------------------------------------------------
# F5: re-releasing the same version duplicated headings and links
# --------------------------------------------------------------------------

def test_f5_duplicate_version_is_refused(repo_with_remote):
    repo = repo_with_remote
    commit(repo, "feat: add endpoint")

    runner = CliRunner()
    assert runner.invoke(cli, ["1.1.0", "--auto", "--force"]).exit_code == 0
    before = (repo / "CHANGELOG.md").read_text(encoding="utf-8")

    result = runner.invoke(cli, ["1.1.0", "--auto", "--force"])
    assert result.exit_code != 0
    assert (repo / "CHANGELOG.md").read_text(encoding="utf-8") == before
    assert before.count("## [1.1.0]") == 1


# --------------------------------------------------------------------------
# F6: a fabricated placeholder repo URL was written permanently
# --------------------------------------------------------------------------

def test_f6_no_repo_metadata_yields_no_url(repo):
    assert utils.get_repository_url() is None


def test_f6_links_are_skipped_rather_than_fabricated(repo, capsys):
    commit(repo, "feat: add endpoint")
    result = CliRunner().invoke(cli, ["1.1.0", "--auto", "--force"])
    assert result.exit_code == 0, result.output

    content = (repo / "CHANGELOG.md").read_text(encoding="utf-8")
    # Pre-fix this wrote https://github.com/org/repo into the file forever.
    assert "org/repo" not in content
    assert "[Unreleased]:" not in content


def test_f6_real_remote_still_produces_links(repo_with_remote):
    repo = repo_with_remote
    commit(repo, "feat: add endpoint")
    assert CliRunner().invoke(cli, ["1.1.0", "--auto", "--force"]).exit_code == 0

    content = (repo / "CHANGELOG.md").read_text(encoding="utf-8")
    assert "[Unreleased]: https://github.com/example/demo/compare/v1.1.0...HEAD" in content
    assert "[1.1.0]: https://github.com/example/demo/releases/tag/v1.1.0" in content


# --------------------------------------------------------------------------
# F7: reference definitions were case-sensitive and newline-dependent
# --------------------------------------------------------------------------

def test_f7_lowercase_reference_is_updated_in_place(repo_with_remote):
    repo = repo_with_remote
    commit(repo, "feat: add endpoint")
    (repo / "CHANGELOG.md").write_text(
        "# Changelog\n\n## [Unreleased]\n\n[unreleased]: https://github.com/example/demo/compare/v1.0.0...HEAD\n",
        encoding="utf-8",
    )

    assert CliRunner().invoke(cli, ["1.1.0", "--auto", "--force"]).exit_code == 0
    content = (repo / "CHANGELOG.md").read_text(encoding="utf-8")

    # Pre-fix the lowercase definition stayed stale AND a duplicate "## Links"
    # section was appended.
    assert "v1.0.0...HEAD" not in content
    assert content.lower().count("[unreleased]:") == 1
    assert "## Links" not in content


def test_f7_reference_terminated_by_eof_is_handled(repo_with_remote):
    repo = repo_with_remote
    commit(repo, "feat: add endpoint")
    # No trailing newline after the definition.
    (repo / "CHANGELOG.md").write_text(
        "# Changelog\n\n## [Unreleased]\n\n[Unreleased]: https://github.com/example/demo/compare/v1.0.0...HEAD",
        encoding="utf-8",
    )

    assert CliRunner().invoke(cli, ["1.1.0", "--auto", "--force"]).exit_code == 0
    content = (repo / "CHANGELOG.md").read_text(encoding="utf-8")

    assert "[1.1.0]: https://github.com/example/demo/releases/tag/v1.1.0" in content
    assert content.lower().count("[unreleased]:") == 1


# --------------------------------------------------------------------------
# F8: an Unreleased heading at EOF was treated as absent
# --------------------------------------------------------------------------

def test_f8_heading_at_eof_without_newline_is_found():
    content = "# Changelog\n\n## [Unreleased]"
    span = utils.find_unreleased_span(content)
    assert span is not None
    _, body_start, body_end = span
    assert content[body_start:body_end] == ""


def test_f8_no_duplicate_heading_appended(repo):
    commit(repo, "feat: add endpoint")
    (repo / "CHANGELOG.md").write_text("# Changelog\n\n## [Unreleased]", encoding="utf-8")

    assert CliRunner().invoke(cli, ["--unreleased"]).exit_code == 0
    content = (repo / "CHANGELOG.md").read_text(encoding="utf-8")

    # Pre-fix a second "## [Unreleased]" was appended.
    assert content.count("## [Unreleased]") == 1
    assert "- add endpoint" in content


# --------------------------------------------------------------------------
# F10: the KeyboardInterrupt guard was unreachable; the generic one is not
# --------------------------------------------------------------------------

def test_f10_generic_error_handler_is_reachable(monkeypatch, capsys):
    from changelog_tool import cli as cli_module

    def boom(*args, **kwargs):
        raise RuntimeError("boom")

    monkeypatch.setattr(cli_module, "cli", boom)
    with pytest.raises(SystemExit) as excinfo:
        cli_module.main()

    assert excinfo.value.code == 1
    assert "Unexpected error: boom" in capsys.readouterr().out


def test_f10_no_keyboardinterrupt_branch_remains():
    import inspect
    from changelog_tool import cli as cli_module

    # The branch was dead code: Click consumes KeyboardInterrupt itself.
    # Matched as an except clause so the explanatory comment does not count.
    assert "except KeyboardInterrupt" not in inspect.getsource(cli_module.main)


# --------------------------------------------------------------------------
# F11: squash subjects already carrying (#42) got a duplicate [#42] suffix
# --------------------------------------------------------------------------

def test_f11_existing_pr_reference_is_not_duplicated():
    entry = get_commit_changelog_entry(
        {"subject": "correct off-by-one in pagination (#123)", "pr": "123"}
    )
    # Pre-fix: "... (#123) [#123]".
    assert entry == "correct off-by-one in pagination (#123)"


def test_f11_pr_suffix_still_added_when_absent():
    entry = get_commit_changelog_entry({"subject": "add endpoint", "pr": "123"})
    assert entry == "add endpoint [#123]"


def test_f11_partial_number_does_not_suppress_suffix():
    # "#4" must not be considered present in a subject mentioning "#42".
    entry = get_commit_changelog_entry({"subject": "touch up thing (#42)", "pr": "4"})
    assert entry == "touch up thing (#42) [#4]"
