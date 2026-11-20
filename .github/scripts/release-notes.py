#!/usr/bin/env python3
"""Generate structured GitHub release notes from git commit history."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

ARTIFACT_ORDER = [".apk", ".msi", ".AppImage", ".tar.gz"]
DEFAULT_FORMATS = ".apk, .msi, .AppImage, .tar.gz"

NOISE_RE = re.compile(
    r"^(update project(mem| memory) summary|update summary|initial commit|first commit)\b",
    re.IGNORECASE,
)
BUGFIX_RE = re.compile(
    r"^(fix(\([^)]*\))?!?:|fix\b|bugfix\b|resolve\b|repair\b|prevent\b|patch\b)",
    re.IGNORECASE,
)
MAINT_RE = re.compile(
    r"^((docs?|ci|build|chore|style|test|refactor|perf)(\([^)]*\))?!?:|"
    r"document\b|set v\d|bump\b|release\b|unpublish\b|"
    r"update (readme|docs|contributing|pkgbuild|changelog|deps|dependencies)\b|"
    r"add (documentation|github actions|ci/cd))",
    re.IGNORECASE,
)
CONVENTIONAL_PREFIX_RE = re.compile(
    r"^(feat|fix|docs?|ci|build|chore|style|test|refactor|perf)(\([^)]*\))?!?:\s*",
    re.IGNORECASE,
)


def run_git(args: list[str]) -> str:
    """Run git and return stdout stripped of trailing whitespace."""
    res = subprocess.run(
        ["git", *args],
        check=True,
        capture_output=True,
        text=True,
    )
    return res.stdout.strip()


def resolve_prev_tag(tag: str) -> str | None:
    """Return the newest v* tag reachable from tag other than tag itself."""
    out = run_git(["tag", "--merged", tag, "--list", "v*", "--sort=-v:refname"])
    for line in out.splitlines():
        candidate = line.strip()
        if candidate and candidate != tag:
            return candidate
    return None


def fetch_commits(tag: str, prev_tag: str | None) -> list[tuple[str, str, str]]:
    """Return (full_sha, short_sha, subject) tuples for commits in range."""
    rev_range = f"{prev_tag}..{tag}" if prev_tag else tag
    out = run_git(["log", "--no-merges", "--format=%H%x1f%h%x1f%s", rev_range])
    commits: list[tuple[str, str, str]] = []
    for line in out.splitlines():
        if not line.strip():
            continue
        parts = line.split("\x1f", 2)
        if len(parts) == 3:
            commits.append((parts[0], parts[1], parts[2]))
    return commits


def clean_subject(subject: str) -> str:
    """Strip conventional commit prefix and capitalize leading ASCII letter."""
    cleaned = CONVENTIONAL_PREFIX_RE.sub("", subject.strip(), count=1).strip()
    if cleaned and "a" <= cleaned[0] <= "z":
        cleaned = cleaned[0].upper() + cleaned[1:]
    return cleaned


def classify_commit(subject: str) -> str | None:
    """Return bucket name for commit subject, or None if noise."""
    stripped = subject.strip()
    if NOISE_RE.match(stripped):
        return None
    if BUGFIX_RE.match(stripped):
        return "Bug Fixes"
    if MAINT_RE.match(stripped):
        return "Packaging, CI & Docs"
    return "What's New"


def detect_formats(dist_dir: str | None, formats_override: str | None) -> str:
    """Detect packaged artifact extensions from dist_dir or fallback options."""
    if dist_dir:
        dist_path = Path(dist_dir)
        if dist_path.is_dir():
            names = [p.name for p in dist_path.iterdir() if p.is_file()]
            present = [
                ext
                for ext in ARTIFACT_ORDER
                if any(name.endswith(ext) for name in names)
            ]
            if present:
                return ", ".join(present)
    if formats_override and formats_override.strip():
        return formats_override.strip()
    return DEFAULT_FORMATS


def render_notes(
    tag: str,
    prev_tag: str | None,
    commits: list[tuple[str, str, str]],
    repo: str,
    formats: str,
) -> str:
    """Render structured markdown release notes from commits."""
    buckets: dict[str, list[str]] = {
        "What's New": [],
        "Bug Fixes": [],
        "Packaging, CI & Docs": [],
    }
    for full_sha, short_sha, subject in commits:
        bucket = classify_commit(subject)
        if bucket is None:
            continue
        text = clean_subject(subject)
        buckets[bucket].append(
            f"- {text} ([{short_sha}](https://github.com/{repo}/commit/{full_sha}))"
        )

    lines: list[str] = ["## What's Changed", ""]
    any_bullets = False
    for heading in ("What's New", "Bug Fixes", "Packaging, CI & Docs"):
        items = buckets[heading]
        if items:
            any_bullets = True
            lines.append(f"### {heading}")
            lines.extend(items)
            lines.append("")

    if not any_bullets:
        lines.append("- Maintenance release.")
        lines.append("")

    if prev_tag:
        lines.append(
            f"**Full Changelog**: https://github.com/{repo}/compare/{prev_tag}...{tag}"
        )
        lines.append("")

    lines.extend(
        [
            "---",
            "",
            "### Downloads & Verification",
            f"Pre-release unsigned artifacts ({formats}):",
            "- **Android**: universal `.apk` (`arm64-v8a`, `armeabi-v7a`, `x86_64`)",
            "- **Windows**: `x86_64` `.msi`",
            "- **Linux**: portable `.AppImage` and `.tar.gz` (`x86_64`, `aarch64`)",
            "",
            "Verify downloads against `SHA256SUMS.txt` before installing:",
            "",
            "```bash",
            "sha256sum -c --ignore-missing SHA256SUMS.txt",
            "```",
        ]
    )
    return "\n".join(lines) + "\n"


def run_self_test() -> None:
    """Validate commit filtering, classification, prefix stripping, and rendering."""
    assert classify_commit("Update projectmem summary") is None
    assert classify_commit("Update project memory summary") is None
    assert classify_commit("update summary") is None
    assert classify_commit("Initial commit") is None
    assert classify_commit("first commit") is None

    assert classify_commit("Fix Wayland layer-shell surface scale") == "Bug Fixes"
    assert classify_commit("fix(ui): avoid panic on empty QR") == "Bug Fixes"
    assert classify_commit("Resolve clock mismatch in settings") == "Bug Fixes"

    assert classify_commit("ci: skip docs-only changes") == "Packaging, CI & Docs"
    assert classify_commit("docs: lead README with releases") == "Packaging, CI & Docs"
    assert classify_commit("Document issue #0002 in projectmem") == "Packaging, CI & Docs"
    assert classify_commit("Release 0.1.1: add Linux AppImage packaging") == "Packaging, CI & Docs"

    assert classify_commit("Overhaul desktop settings UI with new design system") == "What's New"
    assert classify_commit("feat: add Horizon Anchor app icon") == "What's New"

    assert clean_subject("feat(ui): add dark mode toggle") == "Add dark mode toggle"
    assert clean_subject("fix!: handle missing socket") == "Handle missing socket"
    assert clean_subject("Fix Wayland surface scale") == "Fix Wayland surface scale"

    sample_commits = [
        ("aaaa1111aaaa1111", "aaaa111", "update summary"),
        ("bbbb2222bbbb2222", "bbbb222", "feat: add Peripheral Field icon"),
        ("cccc3333cccc3333", "cccc333", "fix(net): retry tether gateway"),
        ("dddd4444dddd4444", "dddd444", "docs: clarify AppImage steps"),
    ]
    md = render_notes("v0.1.1", "v0.1.0", sample_commits, "owner/repo", ".apk, .msi, .AppImage, .tar.gz")
    assert "update summary" not in md
    assert "### What's New\n- Add Peripheral Field icon ([bbbb222](https://github.com/owner/repo/commit/bbbb2222bbbb2222))" in md
    assert "### Bug Fixes\n- Retry tether gateway ([cccc333](https://github.com/owner/repo/commit/cccc3333cccc3333))" in md
    assert "### Packaging, CI & Docs\n- Clarify AppImage steps ([dddd444](https://github.com/owner/repo/commit/dddd4444dddd4444))" in md
    assert "**Full Changelog**: https://github.com/owner/repo/compare/v0.1.0...v0.1.1" in md
    assert "(.apk, .msi, .AppImage, .tar.gz):" in md
    assert "sha256sum -c --ignore-missing SHA256SUMS.txt" in md

    empty_md = render_notes("v0.1.0", None, [("eeee5555", "eeee555", "Initial commit")], "owner/repo", ".apk")
    assert "- Maintenance release." in empty_md
    assert "Full Changelog" not in empty_md


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate structured GitHub release notes.")
    parser.add_argument("--tag", help="Target git tag or ref (e.g. v0.1.1 or HEAD)")
    parser.add_argument(
        "--repo",
        default=os.environ.get("GITHUB_REPOSITORY", "nandazahran/KineAnchor"),
        help="GitHub owner/repo slug",
    )
    parser.add_argument("--dist-dir", help="Directory containing built release artifacts")
    parser.add_argument("--formats", help="Comma-separated artifact format override")
    parser.add_argument("--self-test", action="store_true", help="Run built-in unit checks")
    args = parser.parse_args()

    if args.self_test:
        run_self_test()
        return 0

    if not args.tag:
        parser.error("--tag is required unless --self-test is set")

    prev_tag = resolve_prev_tag(args.tag)
    commits = fetch_commits(args.tag, prev_tag)
    formats = detect_formats(args.dist_dir, args.formats)
    sys.stdout.write(render_notes(args.tag, prev_tag, commits, args.repo, formats))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
