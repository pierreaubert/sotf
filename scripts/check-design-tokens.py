#!/usr/bin/env python3
"""Design-token drift guard for app-gpui.

Two checks run together:

1. Hardcoded pixel sizes — fails when a raw `px(N.0)` call appears in
   GPUI component or UI code outside the design-token source-of-truth
   files. Migrate to a `Ds` token (`d.pad_x`, `spacing::MD`, ...) or
   mark `// intentional: <reason>`.

2. Manual `Text::new(..)` builder chains that have a semantic constructor.
   E.g. `Text::new(x).size(Xs).muted(true)` should be `Text::caption(x)`,
   `Text::new(x).size(Md).weight(Semibold)` should be
   `Text::section_header(x)`. The full role → constructor map lives in
   `crates/app-gpui/CLAUDE.md` ('Typography conventions').

Both checks share the same exception system: an `// intentional: <reason>`
comment on the same line or within 8 lines above (not crossing a blank
line) opts out. A file-level `// intentional-file: <reason>` marker
exempts the whole file (used for chart/meter code with intrinsically
pixel-driven layout).

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

# Pairs of (regex, suggested_constructor, justification_hint).
# Each regex matches a manual builder chain that has a semantic-Text
# constructor in `gpui_ui_kit::Text` / `Heading`. Matched chains are
# behavior-equivalent to the constructor — migrate the call site or mark
# the line `// intentional: <reason>` to opt out.
#
# Dynamic patterns like `weight(if cond { Semibold } else { Normal })` do
# NOT match these regexes because the literal `(TextWeight::X)` shape is
# required.
TEXT_BUILDER_PATTERNS: tuple[tuple[re.Pattern[str], str, str], ...] = (
    (
        re.compile(
            r"\.size\(TextSize::Xs\)\s*\.muted\(true\)"
            r"|\.muted\(true\)\s*\.size\(TextSize::Xs\)"
        ),
        "Text::caption",
        "Xs + muted(true)",
    ),
    (
        re.compile(r"\.size\(TextSize::Xs\)\s*\.color\(theme\.text_muted\)"),
        "Text::caption",
        "Xs + theme.text_muted",
    ),
    (
        re.compile(r"\.size\(TextSize::Xs\)\s*\.weight\(TextWeight::Bold\)"),
        "Text::eyebrow",
        "Xs + Bold",
    ),
    (
        re.compile(r"\.size\(TextSize::(?:Md|Sm)\)\s*\.weight\(TextWeight::Semibold\)"),
        "Text::section_header",
        "Md/Sm + Semibold",
    ),
    (
        re.compile(r"\.size\(TextSize::Sm\)\s*\.weight\(TextWeight::Medium\)"),
        "Text::label",
        "Sm + Medium",
    ),
    (
        re.compile(r"\.size\(TextSize::Md\)\s*\.weight\(TextWeight::Bold\)"),
        "Heading::h4",
        "Md + Bold",
    ),
)


def is_exempt(relative_path: Path) -> bool:
    if relative_path in ALLOWLIST:
        return True
    path_str = relative_path.as_posix()
    return any(fragment in f"/{path_str}" for fragment in EXEMPT_PATH_FRAGMENTS)


def _is_justified(lines: list[str], idx: int) -> bool:
    """True iff the given line is exempted by an `// intentional:` marker on
    the same line or within LOOKBACK_LINES above (stopping at the first
    blank line)."""
    if INTENTIONAL_RE.search(lines[idx]):
        return True
    start = max(0, idx - LOOKBACK_LINES)
    for prev_idx in range(idx - 1, start - 1, -1):
        prev = lines[prev_idx].rstrip()
        if prev == "":
            return False
        if INTENTIONAL_RE.search(prev):
            return True
    return False


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
        if _is_justified(lines, idx):
            continue
        violations.append((idx + 1, line.rstrip()))
    return violations


def check_text_builder(path: Path) -> list[tuple[int, str, str]]:
    """Return a list of (line_number, suggested_constructor, justification_hint)
    for manual `Text::new(..)` builder chains that have a semantic constructor.

    Matches builder chains across line boundaries (whitespace in regexes
    spans newlines), so e.g. a chain split across `.size(...)\n.weight(...)`
    is detected. The first matching line is reported.
    """
    violations: list[tuple[int, str, str]] = []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return violations

    if FILE_OPT_OUT_RE.search(text):
        return violations

    lines = text.splitlines()
    seen: set[tuple[int, str]] = set()
    for pattern, constructor, hint in TEXT_BUILDER_PATTERNS:
        for match in pattern.finditer(text):
            line_idx = text.count("\n", 0, match.start())
            if (line_idx, constructor) in seen:
                continue
            # Skip if the line is itself a comment (e.g. doc example).
            if lines[line_idx].lstrip().startswith("//"):
                continue
            if _is_justified(lines, line_idx):
                continue
            seen.add((line_idx, constructor))
            violations.append((line_idx + 1, constructor, hint))
    violations.sort(key=lambda v: v[0])
    return violations


def collect_violations(
    repo_root: Path,
) -> tuple[list[tuple[Path, int, str]], list[tuple[Path, int, str, str]]]:
    """Return (px_violations, text_builder_violations).

    px_violations:           (path, line, line_content)
    text_builder_violations: (path, line, suggested_constructor, hint)
    """
    px_findings: list[tuple[Path, int, str]] = []
    tb_findings: list[tuple[Path, int, str, str]] = []
    for search in SEARCH_PATHS:
        search_abs = repo_root / search
        if not search_abs.is_dir():
            continue
        for rs in sorted(search_abs.rglob("*.rs")):
            rel = rs.relative_to(repo_root)
            if is_exempt(rel):
                continue
            for line_no, content in check_file(rs):
                px_findings.append((rel, line_no, content))
            for line_no, ctor, hint in check_text_builder(rs):
                tb_findings.append((rel, line_no, ctor, hint))
    return px_findings, tb_findings


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

    px_findings, tb_findings = collect_violations(repo_root)
    if not px_findings and not tb_findings:
        return 0

    for rel, line_no, content in px_findings:
        print(f"{rel.as_posix()}:{line_no}: {content.strip()}")
    if px_findings:
        print(
            f"\nerror: {len(px_findings)} raw px(N.0) call(s) outside the design-token allowlist "
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

    for rel, line_no, ctor, hint in tb_findings:
        print(f"{rel.as_posix()}:{line_no}: manual {hint} chain — use {ctor}(...)")
    if tb_findings:
        print(
            f"\nerror: {len(tb_findings)} manual Text::new(..) builder chain(s) "
            "match a semantic constructor in `gpui_ui_kit::Text` / `Heading`.",
            file=sys.stderr,
        )
        print(
            "Fix by either:\n"
            "  - replacing the chain with the suggested constructor:\n"
            "      Text::caption / Text::eyebrow / Text::section_header / Text::label / "
            "Text::body / Heading::h4\n"
            "  - or adding a `// intentional: [reason]` comment on the same line or within "
            f"{LOOKBACK_LINES} lines above (not crossing a blank line).\n"
            "See crates/app-gpui/CLAUDE.md → 'Typography conventions' for the role → "
            "constructor map.",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
