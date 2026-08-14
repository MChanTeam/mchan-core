#!/usr/bin/env python3
"""Keep the Cargo package, lockfile, and release changelog in sync."""

from __future__ import annotations

import argparse
import datetime as dt
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CARGO_TOML = ROOT / "Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"
CHANGELOG = ROOT / "docs" / "CHANGELOG.md"
HOME_TEMPLATE = ROOT / "templates" / "home.html"
PACKAGE_SECTION = re.compile(r"(?ms)^\[package\]\s*(.*?)(?=^\[|\Z)")
VERSION_ASSIGNMENT = re.compile(r'^version\s*=\s*"([^"]+)"\s*$', re.MULTILINE)
LOCK_PACKAGE = re.compile(r"(?ms)^\[\[package\]\]\s*(.*?)(?=^\[\[package\]\]|\Z)")
NAME_ASSIGNMENT = re.compile(r'^name\s*=\s*"([^"]+)"\s*$', re.MULTILINE)
LOCK_VERSION_ASSIGNMENT = re.compile(r'^version\s*=\s*"([^"]+)"\s*$', re.MULTILINE)

VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)(?:\.(0|[1-9]\d*))?$")
HOMEPAGE_RELEASE = re.compile(r"\bMChan\s+v\d+(?:\.\d+)*(?:[-+][0-9A-Za-z.-]+)?\b")
DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


class ReleaseError(Exception):
    """An expected release validation or update failure."""


def parse_version(value: str) -> tuple[str, str]:
    match = VERSION.fullmatch(value)
    if match is None:
        raise ReleaseError(
            f"malformed version {value!r}; expected MAJOR.MINOR or MAJOR.MINOR.PATCH"
        )
    major, minor, patch = match.groups()
    patch = patch or "0"
    label = f"{major}.{minor}" if patch == "0" else f"{major}.{minor}.{patch}"
    return f"{major}.{minor}.{patch}", label


def parse_date(value: str) -> str:
    if DATE.fullmatch(value) is None:
        raise argparse.ArgumentTypeError("date must use YYYY-MM-DD")
    try:
        dt.date.fromisoformat(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("date must be a valid calendar date") from error
    return value


def expected_files(root: Path) -> tuple[Path, Path, Path, Path]:
    paths = (
        root / "Cargo.toml",
        root / "Cargo.lock",
        root / "docs" / "CHANGELOG.md",
        root / "templates" / "home.html",
    )
    for path in paths:
        if not path.is_file():
            raise ReleaseError(f"expected file is missing: {path}")
    return paths


def cargo_package_version(cargo_text: str) -> tuple[str, str]:
    sections = list(PACKAGE_SECTION.finditer(cargo_text))
    if len(sections) != 1:
        raise ReleaseError("Cargo.toml must contain exactly one [package] section")
    section = sections[0]
    names = NAME_ASSIGNMENT.findall(section.group(1))
    versions = list(VERSION_ASSIGNMENT.finditer(section.group(1)))
    if len(names) != 1 or len(versions) != 1:
        raise ReleaseError("Cargo.toml [package] must contain one name and one version")
    return names[0], versions[0].group(1)


def lock_package_version(lock_text: str, package_name: str) -> tuple[str, re.Match[str]]:
    packages = []
    for package in LOCK_PACKAGE.finditer(lock_text):
        body = package.group(1)
        names = NAME_ASSIGNMENT.findall(body)
        if len(names) != 1:
            raise ReleaseError("Cargo.lock contains a package entry without one name")
        if names[0] == package_name:
            packages.append(package)
    if len(packages) != 1:
        raise ReleaseError(
            f"Cargo.lock must contain exactly one root package entry for {package_name!r}"
        )
    package = packages[0]
    versions = list(LOCK_VERSION_ASSIGNMENT.finditer(package.group(1)))
    if len(versions) != 1:
        raise ReleaseError(f"Cargo.lock root package {package_name!r} must contain one version")
    return versions[0].group(1), versions[0]



def update_cargo_version(cargo_text: str, version: str) -> str:
    section = next(PACKAGE_SECTION.finditer(cargo_text), None)
    if section is None:
        raise ReleaseError("Cargo.toml [package] section is missing")
    assignment = next(VERSION_ASSIGNMENT.finditer(section.group(1)), None)
    if assignment is None:
        raise ReleaseError("Cargo.toml [package] version is missing")
    start = section.start(1) + assignment.start(1)
    end = section.start(1) + assignment.end(1)
    return cargo_text[:start] + version + cargo_text[end:]


def update_lock_version(lock_text: str, package_name: str, version: str) -> str:
    packages = []
    for package in LOCK_PACKAGE.finditer(lock_text):
        names = NAME_ASSIGNMENT.findall(package.group(1))
        if len(names) == 1 and names[0] == package_name:
            packages.append(package)
    if len(packages) != 1:
        raise ReleaseError(
            f"Cargo.lock must contain exactly one root package entry for {package_name!r}"
        )
    package = packages[0]
    assignment = next(LOCK_VERSION_ASSIGNMENT.finditer(package.group(1)), None)
    if assignment is None:
        raise ReleaseError(f"Cargo.lock root package {package_name!r} version is missing")
    start = package.start(1) + assignment.start(1)
    end = package.start(1) + assignment.end(1)
    return lock_text[:start] + version + lock_text[end:]


def changelog_parts(changelog_text: str) -> tuple[list[str], int, int]:
    lines = changelog_text.splitlines()
    headings = [index for index, line in enumerate(lines) if line.startswith("## [")]
    unreleased = [index for index in headings if lines[index] == "## [Unreleased]"]
    if len(unreleased) != 1:
        raise ReleaseError("changelog must contain exactly one ## [Unreleased] heading")
    start = unreleased[0]
    following = [index for index in headings if index > start]
    if not following:
        raise ReleaseError("## [Unreleased] must be followed by a release heading")
    return lines, start, following[0]


def release_labels(lines: list[str]) -> list[str]:
    labels = []
    for line in lines:
        match = re.fullmatch(r"## \[([^\]]+)\](?: - .*)?", line)
        if match:
            labels.append(match.group(1))
    return labels


def promote_changelog(changelog_text: str, label: str, release_date: str) -> str:
    lines, unreleased_start, next_heading = changelog_parts(changelog_text)
    if label in release_labels(lines):
        raise ReleaseError(f"release {label!r} already exists in the changelog")
    notes = lines[unreleased_start + 1 : next_heading]
    while notes and not notes[0].strip():
        notes.pop(0)
    while notes and not notes[-1].strip():
        notes.pop()
    if not notes:
        raise ReleaseError("cannot release with empty Unreleased notes")
    promoted = (
        lines[:unreleased_start]
        + ["## [Unreleased]", "", f"## [{label}] - {release_date}", ""]
        + notes
        + [""]
        + lines[next_heading:]
    )
    return "\n".join(promoted) + "\n"


def check_tree(root: Path) -> None:
    cargo_path, lock_path, changelog_path, homepage_path = expected_files(root)
    cargo_text = cargo_path.read_text()
    lock_text = lock_path.read_text()
    package_name, cargo_version = cargo_package_version(cargo_text)
    lock_version, _ = lock_package_version(lock_text, package_name)
    if cargo_version != lock_version:
        raise ReleaseError(
            f"Cargo.toml version {cargo_version} does not match Cargo.lock version {lock_version}"
        )
    _, label = parse_version(cargo_version)
    changelog_text = changelog_path.read_text()
    lines, unreleased_start, next_heading = changelog_parts(changelog_text)
    current = [
        line
        for line in lines
        if re.fullmatch(r"## \[[^\]]+\](?: - .*)?", line)
        and line.startswith(f"## [{label}]")
    ]
    if len(current) != 1:
        raise ReleaseError(f"changelog must contain exactly one current heading ## [{label}]")
    date_match = re.fullmatch(
        rf"## \[{re.escape(label)}\] - (?P<date>\d{{4}}-\d{{2}}-\d{{2}})", current[0]
    )
    if date_match is None:
        raise ReleaseError(f"current changelog heading must be dated: ## [{label}] - YYYY-MM-DD")
    try:
        dt.date.fromisoformat(date_match.group("date"))
    except ValueError as error:
        raise ReleaseError("current changelog heading has an invalid release date") from error
    if any(line.strip() for line in lines[unreleased_start + 1 : next_heading]):
        raise ReleaseError("## [Unreleased] must be empty after a release")
    homepage = homepage_path.read_text()
    if HOMEPAGE_RELEASE.search(homepage):
        raise ReleaseError("homepage contains a hardcoded MChan v<digits> release string")


def perform_release(root: Path, requested_version: str, release_date: str) -> None:
    try:
        release_date = parse_date(release_date)
    except argparse.ArgumentTypeError as error:
        raise ReleaseError(str(error)) from error
    normalized, label = parse_version(requested_version)
    cargo_path, lock_path, changelog_path, _ = expected_files(root)
    cargo_text = cargo_path.read_text()
    lock_text = lock_path.read_text()
    package_name, current_version = cargo_package_version(cargo_text)
    lock_version, _ = lock_package_version(lock_text, package_name)
    if current_version != lock_version:
        raise ReleaseError(
            f"Cargo.toml version {current_version} does not match Cargo.lock version {lock_version}"
        )
    changelog_text = changelog_path.read_text()
    updated_changelog = promote_changelog(changelog_text, label, release_date)
    updated_cargo = update_cargo_version(cargo_text, normalized)
    updated_lock = update_lock_version(lock_text, package_name, normalized)
    cargo_path.write_text(updated_cargo)
    lock_path.write_text(updated_lock)
    changelog_path.write_text(updated_changelog)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", help="release version (MAJOR.MINOR[.PATCH])")
    parser.add_argument("--date", type=parse_date, help="release date (YYYY-MM-DD)")
    parser.add_argument("--check", action="store_true", help="validate release metadata without editing")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.check and (args.version is not None or args.date is not None):
        parser.error("--check cannot be combined with VERSION or --date")
    try:
        if args.check:
            check_tree(ROOT)
        else:
            if args.version is None:
                parser.error("VERSION is required unless --check is used")
            release_date = args.date or dt.date.today().isoformat()
            perform_release(ROOT, args.version, release_date)
    except (OSError, ReleaseError) as error:
        print(f"release.py: error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
