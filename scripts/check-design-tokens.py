#!/usr/bin/env python3
"""Design-token drift guard for app-gpui.

Fails when a raw `px(N.0)` call appears in GPUI component or UI code outside
the design-token source-of-truth files, unless an `// intentional: ...`
comment appears on the same line or within an 8-line look-back window
(stopping at the first blank line).

Rationale:
    Phase 1 migrated the `Ds` design tokens to `Rems` so font zoom propagates
    to spacing and text. Phase 2 migrated remaining hardcoded `px()` call
    sites across the component tree to those tokens. This script prevents the
    migrated call sites from silently regressing: any new raw `px(N.0)` must
    either use a design token (`d.pad_x`, `spacing::MD`, `radius::MD`, ...) or
    be explicitly marked intentional with a justification comment.

Usage:
    scripts/check-design-tokens.py
    scripts/check-design-tokens.py --help

Exit codes:
    0  no violations
    1  one or more violations
    2  usage error
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

SEARCH_PATHS: tuple[Path, ...] = (
    Path("crates/app-gpui/components"),
    Path("crates/app-gpui/ui"),
)

# Files exempt from the check. Each is a source-of-truth for design tokens
# or is system plumbing that must work in absolute pixels.
ALLOWLIST: frozenset[Path] = frozenset(
    {
        Path("crates/app-gpui/components/design.rs"),
        Path("crates/app-gpui/components/icons/mod.rs"),
        Path("crates/app-gpui/ui/render.rs"),
    }
)

# Path fragments that exempt a file entirely (tests use raw pixels freely).
EXEMPT_PATH_FRAGMENTS: tuple[str, ...] = ("/tests/",)

PX_RE = re.compile(r"\bpx\(\s*\d")
INTENTIONAL_RE = re.compile(r"//.*\bintentional\b", re.IGNORECASE)
FILE_OPT_OUT_RE = re.compile(r"//\s*intentional-file\b", re.IGNORECASE)
LOOKBACK_LINES = 8


def is_exempt(relative_path: Path) -> bool:
    if relative_path in ALLOWLIST:
        return True
    path_str = relative_path.as_posix()
    return any(fragment in f"/{path_str}" for fragment in EXEMPT_PATH_FRAGMENTS)


def check_file(path: Path) -> list[tuple[int, str]]:
    """Return a list of (line_number, line_content) for unjustified `px(N.0)` sites."""
    violations: list[tuple[int, str]] = []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return violations

    # File-level opt-out: any `// intentional-file: <reason>` marker in the
    # file exempts the whole file. Used for chart/meter/table code whose
    # pixel dimensions are intrinsically layout-driven.
    if FILE_OPT_OUT_RE.search(text):
        return violations

    lines = text.splitlines()

    for idx, line in enumerate(lines):
        stripped = line.lstrip()
        # Skip lines that are themselves comments — `// assert_eq!(... px(12.0))`
        # in commented-out code shouldn't fail the check.
        if stripped.startswith("//"):
            continue
        if not PX_RE.search(line):
            continue
        if INTENTIONAL_RE.search(line):
            continue

        # Look back up to LOOKBACK_LINES, stopping at the first blank line.
        justified = False
        start = max(0, idx - LOOKBACK_LINES)
        for prev_idx in range(idx - 1, start - 1, -1):
            prev = lines[prev_idx].rstrip()
            if prev == "":
                break
            if INTENTIONAL_RE.search(prev):
                justified = True
                break
        if not justified:
            violations.append((idx + 1, line.rstrip()))
    return violations


def collect_violations(repo_root: Path) -> list[tuple[Path, int, str]]:
    findings: list[tuple[Path, int, str]] = []
    for search in SEARCH_PATHS:
        search_abs = repo_root / search
        if not search_abs.is_dir():
            continue
        for rs in sorted(search_abs.rglob("*.rs")):
            rel = rs.relative_to(repo_root)
            if is_exempt(rel):
                continue
            for line_no, content in check_file(rs):
                findings.append((rel, line_no, content))
    return findings


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path.cwd(),
        help="Repository root (defaults to current working directory).",
    )
    args = parser.parse_args(argv)

    repo_root = args.repo_root.resolve()
    if not (repo_root / "Cargo.toml").is_file():
        print(f"error: {repo_root} does not look like the sotf repository root", file=sys.stderr)
        return 2

    findings = collect_violations(repo_root)
    if not findings:
        return 0

    for rel, line_no, content in findings:
        print(f"{rel.as_posix()}:{line_no}: {content.strip()}")
    print(
        f"\nerror: {len(findings)} raw px(N.0) call(s) outside the design-token allowlist "
        "without an `// intentional:` comment.",
        file=sys.stderr,
    )
    print(
        "Fix by either:\n"
        "  - migrating to a design token (Ds::from_cx + d.pad_x/d.gap/d.r_md/d.text_sm, "
        "or spacing::X / radius::X from app/constants.rs)\n"
        "  - or adding a `// intentional: [reason]` comment on the same line or within "
        f"{LOOKBACK_LINES} lines above (not crossing a blank line).",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
