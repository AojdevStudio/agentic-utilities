"""
Changelog Updater

Production-ready changelog automation script
Supports automatic git analysis and manual entry modes

Usage:
  changelog [version] [--auto|--manual] [--dry-run]
  changelog --unreleased

Examples:
  changelog 1.5.0 --auto          # Auto-analyze git commits
  changelog 1.5.0 --manual        # Interactive mode
  changelog --auto --dry-run      # Preview without changes
  changelog --unreleased          # Regenerate Unreleased section
"""

import json
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional

import click
from colorama import Fore, Style, init

# Import utility functions from utils.py
from .utils import (
    validate_version,
    parse_commits,
    format_changelog,
    update_changelog_file,
    get_next_version,
    initialize_changelog_file,
    changelog_path,
    package_json_path,
    find_unreleased_span,
    ensure_unreleased_heading,
)

# Initialize colorama for cross-platform color support
init(autoreset=True)

# Configuration
CHANGELOG_CATEGORY_ORDER = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"]


def get_empty_changes() -> Dict[str, List[str]]:
    """
    Create an empty changelog category mapping.
    """
    return {category: [] for category in CHANGELOG_CATEGORY_ORDER}


def get_commit_changelog_category(commit: Dict[str, object]) -> Optional[str]:
    """
    Classify a parsed commit into a Keep a Changelog category.
    """
    commit_type = commit.get("type", "change")

    if commit_type in ["feat", "add"]:
        return "Added"
    if commit_type in ["fix", "bugfix"]:
        return "Fixed"
    if commit_type in ["refactor", "perf", "improve"]:
        return "Changed"
    if commit_type == "remove":
        return "Removed"
    if commit_type == "security":
        return "Security"
    if commit_type == "deprecate":
        return "Deprecated"
    if commit_type in ["docs", "test", "chore", "style"]:
        return None

    return "Changed"


def get_commit_changelog_entry(commit: Dict[str, object]) -> str:
    """
    Format a parsed commit for a changelog bullet.
    """
    subject = commit['subject']
    pr = commit.get('pr')

    # Squash-merge subjects already carry "(#42)". Appending " [#42]" produced
    # "... (#42) [#42]". The number is matched specifically so "#4" does not
    # suppress the suffix for "#42".
    if not pr or re.search(rf"#{re.escape(str(pr))}\b", subject):
        return subject

    return f"{subject} [#{pr}]"


def get_changes_from_commits(commits: List[Dict[str, object]]) -> Dict[str, List[str]]:
    """
    Group parsed commits by changelog category.
    """
    changes = get_empty_changes()

    for commit in commits:
        category = get_commit_changelog_category(commit)
        if category:
            changes[category].append(get_commit_changelog_entry(commit))

    return changes


def format_unreleased_body(changes: Dict[str, List[str]]) -> str:
    """
    Format categorized changes for the Unreleased section body only.
    """
    sections = []

    for category in CHANGELOG_CATEGORY_ORDER:
        items = changes.get(category, [])
        if items:
            lines = [f"### {category}", ""]
            lines.extend(f"- {item}" for item in items)
            sections.append("\n".join(lines))

    return "\n\n".join(sections)


def replace_unreleased_body(body: str) -> None:
    """
    Replace only the body of the Unreleased section in CHANGELOG.md.
    """
    path = changelog_path()

    if not path.exists():
        initialize_changelog_file(path)

    current_content = path.read_text(encoding="utf-8")
    current_content = ensure_unreleased_heading(current_content)

    # The span helper matches the header line only (through its own newline).
    # It does NOT consume the blank line(s) that follow it with `\s*`, or
    # body_start would shift forward by the existing blank line on each run
    # while the replacement re-adds one, accumulating a blank line per
    # invocation and breaking idempotency.
    _, body_start, body_end = find_unreleased_span(current_content)

    replacement = f"\n{body}\n\n" if body else "\n"
    new_content = current_content[:body_start] + replacement + current_content[body_end:]
    path.write_text(new_content, encoding="utf-8")


def update_unreleased_changelog(verbose: bool, dry_run: bool = False) -> None:
    """
    Regenerate only the Unreleased section body from commits since the last tag.
    """
    try:
        print(f"{Fore.BLUE}🔄 Changelog Updater\n")

        if not changelog_path().exists():
            print(f"{Fore.YELLOW}⚠️  CHANGELOG.md not found. It will be created.")

        print(f"{Fore.BLUE}🔍 Regenerating Unreleased changes from git commits...\n")
        commits = parse_commits()
        changes = get_changes_from_commits(commits)

        # Nothing to write is NOT the same as "write nothing". Repos that keep a
        # hand-curated Unreleased section (infrastructure repos, anything not
        # driven by conventional commits) would otherwise have it silently
        # erased by a run that found no categories. Bail instead of writing an
        # empty body over real content.
        if not commits:
            print(f"{Fore.YELLOW}⚠️  No commits found since last release. Leaving the existing Unreleased section untouched.")
            return
        if not any(changes.get(key) for key in changes):
            print(f"{Fore.YELLOW}⚠️  No categorized changes found. Leaving the existing Unreleased section untouched.")
            return

        print(f"{Fore.GREEN}📊 Found {len(commits)} commits to analyze")
        for category in CHANGELOG_CATEGORY_ORDER:
            items = changes.get(category, [])
            if items:
                print(f"{Fore.BLUE}{category}: {len(items)} items")

        body = format_unreleased_body(changes)

        if dry_run:
            print(f"{Fore.BLUE}\n📋 Preview of Unreleased section:")
            print(f"{Fore.CYAN}{'─' * 60}")
            print(body)
            print(f"{Fore.CYAN}{'─' * 60}")
            print(f"{Fore.YELLOW}\n🔍 Dry run complete. No files were modified.")
            return

        replace_unreleased_body(body)

        print(f"{Fore.GREEN}✅ Successfully updated CHANGELOG.md Unreleased section")
        print(f"{Fore.BLUE}📁 File location: {changelog_path()}")

    except Exception as error:
        print(f"{Fore.RED}❌ Error updating unreleased changelog: {error}")
        if verbose:
            import traceback
            traceback.print_exc()
        sys.exit(1)


def update_changelog(version: Optional[str], auto: bool, manual: bool, dry_run: bool, 
                    verbose: bool, force: bool) -> None:
    """
    Main changelog update function
    """
    try:
        print(f"{Fore.BLUE}🔄 Changelog Updater\n")

        if not changelog_path().exists():
            if dry_run:
                print(f"{Fore.YELLOW}⚠️  CHANGELOG.md not found. Dry run will preview the first generated entry.")
            else:
                print(f"{Fore.YELLOW}⚠️  CHANGELOG.md not found. It will be created.")

        # Determine version
        if not version:
            if package_json_path().exists():
                package_json = json.loads(package_json_path().read_text())
                current_version = package_json.get("version", "0.1.0")
            else:
                # Default version if no package.json exists
                current_version = "0.1.0"
            version = get_next_version(current_version, auto, force)

        # Validate version format
        if not validate_version(version):
            raise ValueError(f"Invalid version format: {version}. Expected format: X.Y.Z")

        print(f"{Fore.GREEN}📝 Updating changelog for version {version}")

        # Get changes based on mode
        if auto or force:
            if force and manual:
                print(f"{Fore.YELLOW}⚠️  Force mode overrides manual mode - using auto mode")
            print(f"{Fore.BLUE}🔍 Analyzing git commits since last release...\n")
            changes = get_changes_from_git()
        else:
            print(f"{Fore.BLUE}✏️  Manual entry mode\n")
            changes = get_changes_manually()

        # Validate changes
        if not changes or not any(changes.get(key) for key in changes):
            print(f"{Fore.YELLOW}⚠️  No changes detected. Aborting.")
            return

        # Format changelog entry
        changelog_entry = format_changelog(changes, version)

        # Preview changes
        print(f"{Fore.BLUE}\n📋 Preview of changelog entry:")
        print(f"{Fore.CYAN}{'─' * 60}")
        print(changelog_entry)
        print(f"{Fore.CYAN}{'─' * 60}")

        # Confirm or dry-run
        if dry_run:
            print(f"{Fore.YELLOW}\n🔍 Dry run complete. No files were modified.")
            return

        # Skip confirmation if force flag is set
        if not force:
            confirm = input(f"\n{Fore.BLUE}Add this entry to CHANGELOG.md? (y/N): ").strip().lower()
            if confirm not in ['y', 'yes']:
                print(f"{Fore.YELLOW}❌ Changelog update cancelled.")
                return
        else:
            print(f"{Fore.GREEN}\n🚀 Force mode enabled - automatically proceeding...")

        # Update changelog file
        update_changelog_file(changelog_entry, version)

        print(f"{Fore.GREEN}✅ Successfully updated CHANGELOG.md for version {version}")
        print(f"{Fore.BLUE}📁 File location: {changelog_path()}")

        # Suggest next steps
        print(f"{Fore.BLUE}\n💡 Next steps:")
        print(f"{Fore.CYAN}   1. Review the changes in CHANGELOG.md")
        print(f"{Fore.CYAN}   2. Update package.json version if needed")
        print(f"{Fore.CYAN}   3. Commit changes: git add CHANGELOG.md && git commit -m \"docs: update changelog for v{version}\"")
        print(f"{Fore.CYAN}   4. Create release tag: git tag v{version}")

    except Exception as error:
        print(f"{Fore.RED}❌ Error updating changelog: {error}")
        if verbose:
            import traceback
            traceback.print_exc()
        sys.exit(1)


def get_changes_from_git() -> Optional[Dict[str, List[str]]]:
    """
    Get changes from git commits since last tag
    """
    try:
        commits = parse_commits()

        if not commits:
            print(f"{Fore.YELLOW}⚠️  No commits found since last release.")
            return None

        print(f"{Fore.GREEN}📊 Found {len(commits)} commits to analyze\n")

        # Group commits by type
        changes = get_changes_from_commits(commits)

        # Show summary
        for category, items in changes.items():
            if items:
                print(f"{Fore.BLUE}{category}: {len(items)} items")

        return changes

    except Exception as error:
        print(f"{Fore.RED}Error parsing git commits: {error}")
        raise error


def get_changes_manually() -> Dict[str, List[str]]:
    """
    Get changes through manual entry
    """
    changes = {
        "Added": [],
        "Changed": [],
        "Deprecated": [],
        "Removed": [],
        "Fixed": [],
        "Security": [],
    }

    descriptions = {
        "Added": "new features or capabilities",
        "Changed": "changes in existing functionality",
        "Deprecated": "features that will be removed in future versions",
        "Removed": "features that have been removed",
        "Fixed": "bug fixes",
        "Security": "security-related changes",
    }

    print(f"{Fore.BLUE}📝 Enter changes for each category (press Enter with empty line to finish each section)\n")

    for category, description in descriptions.items():
        print(f"\n{Fore.BLUE}{category} ({description}):")
        
        items = []
        while True:
            item = input(f"  {category} item (empty to finish): ").strip()
            if not item:
                break
            items.append(item)
        
        changes[category] = items

    return changes


@click.command()
@click.argument('version', required=False)
@click.option('--auto', is_flag=True, help='Automatically analyze git commits since last release')
@click.option('--manual', is_flag=True, help='Manual entry mode')
@click.option('--dry-run', is_flag=True, help='Preview changes without modifying files')
@click.option('--verbose', is_flag=True, help='Show detailed error information')
@click.option('--force', is_flag=True, help='Skip all confirmation prompts for autonomous execution')
@click.option('--unreleased', is_flag=True, help='Regenerate only the Unreleased section from commits since the last tag')
def cli(version: Optional[str], auto: bool, manual: bool, dry_run: bool, verbose: bool, force: bool, unreleased: bool):
    """
    Update CHANGELOG.md with new version entries

    Examples:
      changelog 1.5.0 --auto          # Auto-analyze git commits
      changelog 1.5.0 --manual        # Interactive mode
      changelog --auto --dry-run      # Preview without changes
      changelog --unreleased          # Regenerate Unreleased section
    """
    if unreleased:
        update_unreleased_changelog(verbose, dry_run)
        return

    # Default to auto mode if neither specified
    if not auto and not manual:
        auto = True

    update_changelog(version, auto, manual, dry_run, verbose, force)


def main() -> None:
    """
    Console script entry point.

    There is deliberately no KeyboardInterrupt branch here: Click's standalone
    mode consumes KeyboardInterrupt itself and prints "Aborted!", so such a
    branch is unreachable. The generic handler below IS reachable, because Click
    does not catch arbitrary exceptions raised inside a command.
    """
    try:
        cli()
    except Exception as e:
        print(f"{Fore.RED}Unexpected error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
