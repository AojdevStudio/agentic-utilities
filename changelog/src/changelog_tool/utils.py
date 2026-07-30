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
from colorama import Fore, Style, init

# Initialize colorama for cross-platform color support
init(autoreset=True)


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
    
    # Auto-determine version bump based on commit types
    try:
        commits = parse_commits()
        has_breaking = any(
            "BREAKING" in commit["subject"] or ("body" in commit and commit["body"] and "BREAKING CHANGE" in commit["body"])
            for commit in commits
        )
        has_features = any(
            commit["type"] in ["feat", "add"] for commit in commits
        )
        
        current_ver = semver.VersionInfo.parse(current_version)
        if has_breaking:
            return str(current_ver.bump_major())
        elif has_features:
            return str(current_ver.bump_minor())
        else:
            return str(current_ver.bump_patch())
    except Exception as error:
        print(f"{Fore.YELLOW}Warning: Could not auto-determine version, defaulting to patch")
        return str(semver.VersionInfo.parse(current_version).bump_patch())


def parse_commits() -> List[Dict[str, Any]]:
    """
    Parse git commits since last tag/release
    
    Returns:
        Array of parsed commit objects
    """
    try:
        repo = git.Repo(".")
        
        # Get the last tag
        try:
            last_tag = str(repo.git.describe("--tags", "--abbrev=0"))
        except git.exc.GitCommandError:
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
            parsed_commit = parse_commit_message({
                "hash": str(commit.hexsha),
                "subject": commit.message.split('\n')[0] if commit.message else "",
                "body": '\n'.join(commit.message.split('\n')[1:]) if commit.message and len(commit.message.split('\n')) > 1 else "",
                "author": str(commit.author.name)
            })
            if parsed_commit and parsed_commit["type"] != "ignore":
                parsed_commits.append(parsed_commit)
        
        return parsed_commits
    except Exception as error:
        print(f"{Fore.RED}Error parsing git commits: {error}")
        return []


def parse_commit_message(commit: Dict[str, str]) -> Optional[Dict[str, Any]]:
    """
    Parse individual commit message using conventional commit format
    
    Args:
        commit: Raw commit object
        
    Returns:
        Parsed commit with type, scope, subject
    """
    subject = commit.get("subject", "")
    
    # Handle undefined or empty subjects
    if not subject:
        return {"type": "ignore", **commit}
    
    # Handle merge commits
    if subject.startswith("Merge "):
        return {"type": "ignore", **commit}
    
    # Parse conventional commit format: type(scope): subject
    # Tolerate an optional leading gitmoji/emoji prefix ("✨ feat: x") and a "!" breaking marker
    # so emoji-prefixed commits yield a clean description bullet instead of "✨ feat: x".
    conventional_pattern = r"^(?:[^\w\s]+\s*)?(\w+)(\([^)]+\))?!?: (.+)$"
    match = re.match(conventional_pattern, subject)
    
    if match:
        type_name, scope, description = match.groups()
        result = {
            "hash": commit["hash"][:8] if commit["hash"] else "",
            "type": type_name.lower(),
            "scope": scope[1:-1] if scope else None,
            "subject": description,
            "body": commit.get("body", ""),
            "author": commit.get("author", ""),
            "pr": extract_pr_number(subject, commit.get("body", ""))
        }
        return result
    
    # Handle non-conventional commits
    commit_type = infer_commit_type(subject)
    return {
        "hash": commit["hash"][:8] if commit["hash"] else "",
        "type": commit_type,
        "scope": None,
        "subject": subject,
        "body": commit.get("body", ""),
        "author": commit.get("author", ""),
        "pr": extract_pr_number(subject, commit.get("body", ""))
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
    changelog_path = Path.cwd() / "CHANGELOG.md"
    
    if not changelog_path.exists():
        initialize_changelog_file(changelog_path)
    
    current_content = changelog_path.read_text(encoding="utf-8")
    
    # Find the [Unreleased] section and add after it
    unreleased_pattern = r"## \[Unreleased\]\s*\n"
    match = re.search(unreleased_pattern, current_content)
    
    if not match:
        current_content = current_content.rstrip() + "\n\n## [Unreleased]\n"
        match = re.search(unreleased_pattern, current_content)
    
    # Split content at the unreleased section
    before_unreleased = current_content[:match.end()]
    after_unreleased = current_content[match.end():]
    
    # Insert new entry after unreleased section
    new_content = before_unreleased + "\n" + changelog_entry + "\n" + after_unreleased
    
    # Update version comparison links at the bottom
    updated_content = update_version_links(new_content, version)
    
    changelog_path.write_text(updated_content, encoding="utf-8")


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
    try:
        repo_url = get_repository_url()
        
        # Create new unreleased link
        new_unreleased_link = f"[Unreleased]: {repo_url}/compare/v{version}...HEAD"
        
        # Try to find existing links and update them
        if "[Unreleased]:" in content:
            # Update existing unreleased link
            updated_content = re.sub(
                r"\[Unreleased\]: .+",
                new_unreleased_link,
                content
            )
            
            # Add the new version link
            version_link = f"[{version}]: {repo_url}/releases/tag/v{version}"
            
            # Insert after unreleased link
            updated_content = re.sub(
                r"(\[Unreleased\]: .+\n)",
                f"\\1{version_link}\n",
                updated_content
            )
            return updated_content
        else:
            # Add links section if it doesn't exist
            return (content + 
                   "\n\n## Links\n" + 
                   new_unreleased_link + "\n" +
                   f"[{version}]: {repo_url}/releases/tag/v{version}\n")
    
    except Exception as error:
        print(f"{Fore.YELLOW}Warning: Could not update version comparison links: {error}")
        return content


def get_repository_url() -> str:
    """
    Resolve the repository URL from package.json or git remote metadata.
    
    Returns:
        Normalized repository URL suitable for changelog comparison links
    """
    package_json_path = Path.cwd() / "package.json"
    if package_json_path.exists():
        package_json = json.loads(package_json_path.read_text())
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

    return "https://github.com/org/repo"


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
