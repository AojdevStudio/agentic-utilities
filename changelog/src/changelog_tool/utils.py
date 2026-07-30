"""
Changelog Utilities

Helper functions for changelog automation
Cross-platform compatible utility functions for git analysis and file manipulation
"""

import json
import os
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any

import git
import semver
from git.exc import GitCommandError
from colorama import Fore, Style, init

# Initialize colorama for cross-platform color support
init(autoreset=True)

# The Unreleased heading is matched line-anchored and may be terminated by a
# newline OR end-of-file. A file ending exactly at "## [Unreleased]" still has a
# heading; requiring a trailing newline made it look absent and appended a
# duplicate.
UNRELEASED_HEADING_PATTERN = r"(?mi)^## \[Unreleased\][^\n]*(?:\r?\n|$)"

# The Unreleased body ends at ANY following level-2 heading, not only a
# bracketed one. Anchoring on "^## \[" swallowed a trailing "## Links" section
# (or any unbracketed H2) into the body and deleted it on rewrite.
NEXT_H2_PATTERN = r"(?m)^## "

# Reference definitions are matched line-anchored and case-insensitively, and
# may be terminated by a newline OR end-of-file. Markdown reference labels are
# case-insensitive, so a lowercase "[unreleased]:" is the same definition.
UNRELEASED_LINK_PATTERN = r"(?mi)^\[unreleased\]:[^\n]*(?:\r?\n|$)"

# A link reference definition: "[label]: target". These collect in a footer
# block at the bottom of the file.
REFERENCE_DEFINITION_PATTERN = re.compile(r"^\[[^\]]+\]:")


def _trim_trailing_reference_block(content: str, body_start: int, body_end: int) -> int:
    """
    Pull a terminal reference-definition footer out of the Unreleased body.

    In a first-release changelog the reference definitions sit directly under
    "## [Unreleased]" with no following H2, so a naive body span ran to EOF and
    swallowed them. Releasing then DELETED unrelated definitions such as
    "[guide]:", and with no repository metadata it deleted the "[unreleased]:"
    definition too, because nothing regenerated it.

    Returns:
        The body end offset with any trailing reference-definition block (and
        the blank lines separating it from the prose) excluded. Unchanged when
        the body has no such block.
    """
    lines = content[body_start:body_end].splitlines(keepends=True)

    index = len(lines)
    saw_reference = False

    while index > 0:
        stripped = lines[index - 1].strip()
        if not stripped:
            index -= 1
            continue
        if REFERENCE_DEFINITION_PATTERN.match(stripped):
            saw_reference = True
            index -= 1
            continue
        break

    if not saw_reference:
        return body_end

    return body_start + sum(len(line) for line in lines[:index])


def changelog_path() -> Path:
    """
    Resolve CHANGELOG.md against the current working directory.

    Deliberately a function rather than an import-time constant so the path
    follows chdir instead of freezing at import.
    """
    return Path.cwd() / "CHANGELOG.md"


def package_json_path() -> Path:
    """
    Resolve package.json against the current working directory.
    """
    return Path.cwd() / "package.json"


def find_unreleased_span(content: str) -> Optional[Tuple[int, int, int]]:
    """
    Locate the Unreleased section.

    Returns:
        (heading_start, body_start, body_end) offsets, or None if there is no
        Unreleased heading. body_end is the offset of the next level-2 heading,
        or end-of-file, with any trailing reference-definition footer excluded.
    """
    match = re.search(UNRELEASED_HEADING_PATTERN, content)
    if not match:
        return None

    body_start = match.end()
    next_section_match = re.search(NEXT_H2_PATTERN, content[body_start:])
    body_end = body_start + next_section_match.start() if next_section_match else len(content)
    body_end = _trim_trailing_reference_block(content, body_start, body_end)
    return (match.start(), body_start, body_end)


def read_unreleased_body(content: str) -> str:
    """
    Return the current Unreleased section body, stripped. Empty when absent.
    """
    span = find_unreleased_span(content)
    if span is None:
        return ""

    _, body_start, body_end = span
    return content[body_start:body_end].strip()


def ensure_unreleased_heading(content: str) -> str:
    """
    Return content guaranteed to contain an Unreleased heading.
    """
    if find_unreleased_span(content) is None:
        return content.rstrip() + "\n\n## [Unreleased]\n"
    return content


def require_unreleased_span(content: str) -> Tuple[int, int, int]:
    """
    Locate the Unreleased section, which the caller has guaranteed exists.

    Callers run ensure_unreleased_heading first, so a miss here is a bug rather
    than a user-input problem. Raising beats unpacking None with a
    "cannot unpack non-sequence" traceback.
    """
    span = find_unreleased_span(content)
    if span is None:
        raise ValueError("CHANGELOG.md has no [Unreleased] section to update.")
    return span


def has_version_heading(content: str, version: str) -> bool:
    """
    Report whether an entry for this exact version already exists.
    """
    return re.search(rf"(?m)^## \[{re.escape(version)}\]", content) is not None


def validate_version(version: str) -> bool:
    """
    Validate semantic version format
    
    Args:
        version: Version string to validate
        
    Returns:
        True if valid semver format
    """
    try:
        semver.VersionInfo.parse(version)
        return True
    except ValueError:
        return False


def get_next_version(current_version: str, auto_mode: bool = False, force_mode: bool = False) -> str:
    """
    Get the next semantic version based on current version
    
    Args:
        current_version: Current version from package.json
        auto_mode: Whether to automatically determine version bump
        force_mode: Skip interactive prompts for autonomous execution
        
    Returns:
        Next version string
    """
    # Force mode: automatically determine version without user interaction
    if force_mode and not auto_mode:
        print(f"{Fore.YELLOW}🤖 Force mode: Auto-determining version bump from commits...")
        auto_mode = True
    
    if not auto_mode and not force_mode:
        print(f"{Fore.BLUE}What type of version bump?")
        print("1. Patch (bug fixes)")
        print("2. Minor (new features)")  
        print("3. Major (breaking changes)")
        print("4. Custom version")
        
        choice = input("Enter choice (1-4): ").strip()
        
        if choice == "4":
            custom_version = input("Enter custom version (e.g., 1.5.0): ").strip()
            if not validate_version(custom_version):
                raise ValueError("Please enter a valid semantic version")
            return custom_version
        elif choice == "1":
            return str(semver.VersionInfo.parse(current_version).bump_patch())
        elif choice == "2":
            return str(semver.VersionInfo.parse(current_version).bump_minor())
        elif choice == "3":
            return str(semver.VersionInfo.parse(current_version).bump_major())
        else:
            raise ValueError("Invalid choice")
    
    # Auto-determine version bump based on commit types.
    # Git failures are NOT swallowed here. Falling back to a patch bump on a
    # broken repository silently produces a wrong version instead of failing.
    commits = parse_commits()

    has_breaking = any(commit.get("breaking") for commit in commits)
    has_features = any(commit.get("type") in ["feat", "add"] for commit in commits)

    current_ver = semver.VersionInfo.parse(current_version)
    if has_breaking:
        return str(current_ver.bump_major())
    elif has_features:
        return str(current_ver.bump_minor())
    else:
        return str(current_ver.bump_patch())


def parse_commits() -> List[Dict[str, Any]]:
    """
    Parse git commits since last tag/release
    
    Returns:
        Array of parsed commit objects
    """
    # Git failures propagate to the CLI boundary, which reports them and exits
    # nonzero. Collapsing them to [] made "not a git repository" indistinguish-
    # able from "no new commits", so the tool silently no-opped and exited 0.
    # search_parent_directories lets the tool run from a subdirectory.
    repo = git.Repo(".", search_parent_directories=True)

    # An unborn HEAD (a repository with no commits yet) is a genuinely empty
    # range, not a failure.
    if not repo.head.is_valid():
        return []

    # Get the last tag
    try:
        last_tag = str(repo.git.describe("--tags", "--abbrev=0"))
    except GitCommandError:
        # No tags found, get all commits
        last_tag = None

    # Get commits since last tag
    if last_tag:
        commits = list(repo.iter_commits(f"{last_tag}..HEAD"))
    else:
        commits = list(repo.iter_commits())

    if not commits:
        return []

    parsed_commits = []
    for commit in commits:
        # GitPython types commit.message as str OR bytes, and it really can be
        # bytes for a message that is not valid UTF-8. Splitting bytes on a str
        # separator raises, so it is normalized to str once here.
        raw_message = commit.message
        message = (
            raw_message.decode("utf-8", errors="replace")
            if isinstance(raw_message, bytes)
            else raw_message
        )
        message_lines = message.split("\n") if message else [""]

        parsed_commit = parse_commit_message({
            "hash": str(commit.hexsha),
            "subject": message_lines[0],
            "body": "\n".join(message_lines[1:]),
            "author": str(commit.author.name)
        })
        if parsed_commit and parsed_commit["type"] != "ignore":
            parsed_commits.append(parsed_commit)

    return parsed_commits


def parse_commit_message(commit: Dict[str, str]) -> Dict[str, Any]:
    """
    Parse individual commit message using conventional commit format
    
    Args:
        commit: Raw commit object
        
    Returns:
        Parsed commit with type, scope, subject
    """
    subject = commit.get("subject", "")
    body = commit.get("body", "")

    # Handle undefined or empty subjects
    if not subject:
        return {"type": "ignore", **commit}

    # Handle merge commits
    if subject.startswith("Merge "):
        return {"type": "ignore", **commit}

    # A "BREAKING CHANGE" footer marks a breaking change regardless of the
    # conventional "!" marker, so both signals are checked for every commit.
    has_breaking_footer = "BREAKING CHANGE" in subject or "BREAKING CHANGE" in (body or "")

    # Parse conventional commit format: type(scope): subject
    # Tolerate an optional leading gitmoji/emoji prefix ("✨ feat: x") and a "!" breaking marker
    # so emoji-prefixed commits yield a clean description bullet instead of "✨ feat: x".
    # The "!" is CAPTURED, not just tolerated: it is the conventional-commits
    # breaking marker, and discarding it downgraded "fix!:" to a patch bump.
    conventional_pattern = r"^(?:[^\w\s]+\s*)?(\w+)(\([^)]+\))?(!)?: (.+)$"
    match = re.match(conventional_pattern, subject)

    if match:
        type_name, scope, breaking_marker, description = match.groups()
        result = {
            "hash": commit["hash"][:8] if commit["hash"] else "",
            "type": type_name.lower(),
            "scope": scope[1:-1] if scope else None,
            "subject": description,
            "body": body,
            "author": commit.get("author", ""),
            "pr": extract_pr_number(subject, body),
            "breaking": bool(breaking_marker) or has_breaking_footer,
        }
        return result

    # Handle non-conventional commits
    commit_type = infer_commit_type(subject)
    return {
        "hash": commit["hash"][:8] if commit["hash"] else "",
        "type": commit_type,
        "scope": None,
        "subject": subject,
        "body": body,
        "author": commit.get("author", ""),
        "pr": extract_pr_number(subject, body),
        "breaking": has_breaking_footer,
    }


def infer_commit_type(subject: str) -> str:
    """
    Infer commit type from subject line for non-conventional commits
    
    Args:
        subject: Commit subject line
        
    Returns:
        Inferred commit type
    """
    lower = subject.lower()
    
    if any(word in lower for word in ["fix", "bug", "patch"]):
        return "fix"
    if any(word in lower for word in ["add", "new", "feat"]):
        return "feat"
    if any(word in lower for word in ["update", "change", "modify"]):
        return "change"
    if any(word in lower for word in ["remove", "delete"]):
        return "remove"
    if any(word in lower for word in ["security", "vuln"]):
        return "security"
    if "deprecate" in lower:
        return "deprecate"
    if any(word in lower for word in ["doc", "readme"]):
        return "docs"
    if "test" in lower:
        return "test"
    if any(word in lower for word in ["chore", "build", "ci"]):
        return "chore"
    
    return "change"


def extract_pr_number(subject: str, body: str) -> Optional[str]:
    """
    Extract PR number from commit subject or body
    
    Args:
        subject: Commit subject
        body: Commit body
        
    Returns:
        PR number or None
    """
    pr_pattern = r"#(\d+)"
    
    subject_match = re.search(pr_pattern, subject)
    if subject_match:
        return subject_match.group(1)
    
    if body:
        body_match = re.search(pr_pattern, body)
        if body_match:
            return body_match.group(1)
    
    return None


def format_changelog(changes: Dict[str, List[str]], version: str) -> str:
    """
    Format changelog entry according to conventions
    
    Args:
        changes: Grouped changes by category
        version: Version number
        
    Returns:
        Formatted changelog entry
    """
    today = datetime.now().date().isoformat()
    entry = f"## [{version}] - {today}\n"
    
    # Order categories according to Keep a Changelog
    ordered_categories = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"]
    
    for category in ordered_categories:
        if changes.get(category) and len(changes[category]) > 0:
            entry += f"\n### {category}\n\n"
            for item in changes[category]:
                entry += f"- {item}\n"
    
    return entry


def update_changelog_file(changelog_entry: str, version: str) -> None:
    """
    Update CHANGELOG.md file with new entry
    
    Args:
        changelog_entry: Formatted changelog entry
        version: Version number
    """
    path = changelog_path()

    if not path.exists():
        initialize_changelog_file(path)

    current_content = path.read_text(encoding="utf-8")

    # Releasing the same version twice previously appended a second identical
    # heading and a second set of reference definitions. Refuse instead.
    if has_version_heading(current_content, version):
        raise ValueError(
            f"CHANGELOG.md already contains an entry for version {version}. "
            "Refusing to add a duplicate entry."
        )

    current_content = ensure_unreleased_heading(current_content)
    heading_start, body_start, body_end = require_unreleased_span(current_content)

    # Consume the ENTIRE Unreleased span, not just its heading. Inserting after
    # the heading alone left the previous Unreleased body sitting underneath the
    # new release heading, so `--unreleased` followed by a release emitted every
    # item twice. The released items are carried by changelog_entry, so the
    # Unreleased section is re-emitted empty.
    unreleased_heading = current_content[heading_start:body_start]
    if not unreleased_heading.endswith("\n"):
        unreleased_heading += "\n"

    new_content = (
        current_content[:heading_start]
        + unreleased_heading
        + "\n"
        + changelog_entry
        + "\n"
        + current_content[body_end:]
    )

    # Update version comparison links at the bottom
    updated_content = update_version_links(new_content, version)

    path.write_text(updated_content, encoding="utf-8")


def initialize_changelog_file(changelog_path: Path) -> None:
    """
    Create a minimal Keep a Changelog-compatible file for first releases.
    
    Args:
        changelog_path: Path to CHANGELOG.md
    """
    changelog_path.write_text(
        "# Changelog\n\n"
        "All notable changes to this project will be documented in this file.\n\n"
        "The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), "
        "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n\n"
        "## [Unreleased]\n",
        encoding="utf-8",
    )


def update_version_links(content: str, version: str) -> str:
    """
    Update version comparison links in changelog footer
    
    Args:
        content: Changelog content
        version: New version
        
    Returns:
        Updated content with version links
    """
    repo_url = get_repository_url()

    # Without real repository metadata the previous code wrote a fabricated
    # "https://github.com/org/repo" placeholder permanently into the changelog.
    # Skipping is honest; a fake link is worse than no link.
    if not repo_url:
        print(
            f"{Fore.YELLOW}Warning: Could not determine the repository URL. "
            "Skipping changelog comparison links."
        )
        return content

    new_unreleased_link = f"[Unreleased]: {repo_url}/compare/v{version}...HEAD"
    version_link = f"[{version}]: {repo_url}/releases/tag/v{version}"

    # Markdown reference labels are case-insensitive, so a lowercase
    # "[unreleased]:" is the SAME definition. Matching "[Unreleased]:" literally
    # left it stale and appended a duplicate links section alongside it.
    match = re.search(UNRELEASED_LINK_PATTERN, content)

    if match:
        replacement = f"{new_unreleased_link}\n{version_link}\n"
        return content[:match.start()] + replacement + content[match.end():]

    return content.rstrip() + "\n\n" + new_unreleased_link + "\n" + version_link + "\n"


def get_repository_url() -> Optional[str]:
    """
    Resolve the repository URL from package.json or git remote metadata.

    Returns:
        Normalized repository URL, or None when it cannot be determined. The
        caller is expected to skip link generation rather than invent one.
    """
    path = package_json_path()
    if path.exists():
        try:
            package_json = json.loads(path.read_text())
        except (ValueError, OSError):
            package_json = {}
        repo_url = package_json.get("repository", {})
        if isinstance(repo_url, dict):
            repo_url = repo_url.get("url")
        if isinstance(repo_url, str) and repo_url:
            return normalize_repository_url(repo_url)

    try:
        repo = git.Repo(".", search_parent_directories=True)
        remote_url = repo.git.config("--get", "remote.origin.url").strip()
        if remote_url:
            return normalize_repository_url(remote_url)
    except Exception:
        pass

    return None


def normalize_repository_url(repo_url: str) -> str:
    """
    Normalize common git remote URL formats to an HTTPS repository URL.
    
    Args:
        repo_url: Raw repository URL
    
    Returns:
        Normalized HTTPS URL without a .git suffix
    """
    repo_url = repo_url.replace("git+", "")
    if repo_url.endswith(".git"):
        repo_url = repo_url[:-4]

    ssh_match = re.match(r"^git@([^:]+):(.+)$", repo_url)
    if ssh_match:
        host, path = ssh_match.groups()
        return f"https://{host}/{path}"

    ssh_url_match = re.match(r"^ssh://git@([^/]+)/(.+)$", repo_url)
    if ssh_url_match:
        host, path = ssh_url_match.groups()
        return f"https://{host}/{path}"

    return repo_url


def get_repository_info() -> Optional[Dict[str, str]]:
    """
    Get git repository information
    
    Returns:
        Repository info including remote URL
    """
    try:
        repo = git.Repo(".")
        remote_url = repo.remotes.origin.url
        
        repo_match = re.search(r"github\.com[:/](.+?)/(.+?)(?:\.git)?$", remote_url)
        
        if repo_match:
            owner, repo_name = repo_match.groups()
            return {
                "owner": owner,
                "repo": repo_name,
                "url": f"https://github.com/{owner}/{repo_name}"
            }
        
        return None
    except Exception:
        return None


def validate_changelog_structure(file_path: Path) -> bool:
    """
    Validate changelog file structure
    
    Args:
        file_path: Path to changelog file
        
    Returns:
        True if valid structure
    """
    try:
        content = file_path.read_text(encoding="utf-8")
        
        # Check for required sections
        required_patterns = [
            r"# Changelog",
            r"## \[Unreleased\]",
            r"The format is based on \[Keep a Changelog\]"
        ]
        
        return all(re.search(pattern, content) for pattern in required_patterns)
    except Exception:
        return False


def create_backup(file_path: Path) -> Path:
    """
    Create a backup of the changelog file
    
    Args:
        file_path: Path to changelog file
        
    Returns:
        Path to backup file
    """
    timestamp = datetime.now().isoformat().replace(":", "-").replace(".", "-")
    backup_path = file_path.with_suffix(f".backup.{timestamp}")
    backup_path.write_text(file_path.read_text(encoding="utf-8"))
    return backup_path


if __name__ == "__main__":
    # Example usage
    print(f"{Fore.GREEN}Changelog Utilities - Python Version")
    print(f"{Fore.BLUE}Available functions:")
    print("- validate_version()")
    print("- get_next_version()")
    print("- parse_commits()")
    print("- format_changelog()")
    print("- update_changelog_file()")
