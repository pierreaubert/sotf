#!/usr/bin/env python3
"""Design-token drift guard for app-gpui.

Three checks run together:

1. Hardcoded pixel sizes — fails when a raw numeric `px(...)` /
   `gpui::px(...)` call appears in GPUI component or UI code outside
   the design-token source-of-truth files. Migrate to a `Ds` token
   (`d.pad_x`, `spacing::MD`, ...) or mark `// intentional: <reason>`.

2. Manual `Text::new(..)` builder chains that have a semantic constructor.
   E.g. `Text::new(x).size(Xs).muted(true)` should be `Text::caption(x)`,
   `Text::new(x).size(Md).weight(Semibold)` should be
   `Text::section_header(x)`. The full role → constructor map lives in
   `crates/app-gpui/CLAUDE.md` ('Typography conventions').

3. Hardcoded UI colors and raw rem spacing in app UI code. Colors belong in
   `Theme` / `ThemeFeedback` / `ThemePluginPalette` and should be consumed via
   `theme.*` or helpers such as `Theme::with_opacity(...)`; spacing belongs in
   `Ds` or `spacing::*`.

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
import tempfile
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

# Theme source-of-truth files may define colors directly. App chrome should not.
THEME_SOURCE_PATH_FRAGMENTS: tuple[str, ...] = (
    "/app/theme/",
    "/components/plugins/theme/",
)

# Path fragments that exempt a file entirely (tests use raw pixels freely).
EXEMPT_PATH_FRAGMENTS: tuple[str, ...] = ("/tests/",)
EXEMPT_FILE_NAMES: frozenset[str] = frozenset({"tests.rs"})

# Numeric px calls are design-token drift whether positive, negative, integer,
# or fractional. Dynamic `px(width)` / `px(CHART_W)` calls are usually
# layout/math driven, so this guard only catches literal numbers.
PX_RE = re.compile(
    r"(?<![A-Za-z0-9_])(?:gpui::)?px\(\s*[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[fF](?:32|64))?\s*\)"
)
RAW_SPACING_RE = re.compile(
    r"\.(?:gap|p|px|py|pt|pr|pb|pl|m|mx|my|mt|mr|mb|ml)\(\s*rems\(\s*[+-]?(?:\d+(?:\.\d*)?|\.\d+)"
)
RAW_COLOR_RE = re.compile(
    r"(?:\bgpui::)?(?:rgba?|hsla?)\(\s*(?:0x[0-9A-Fa-f]{6,8}|#[0-9A-Fa-f]{3,8})"
    r"|\bgpui::(?:white|black)\(\)"
    r"|\b(?:white|black)\(\)"
)
COLOR_STRUCT_START_RE = re.compile(r"\b(?:gpui::)?(?:Rgba|Hsla)\s*\{")
COLOR_LITERAL_FIELD_RE = re.compile(
    r"\b(?:r|g|b|a|h|s|l):\s*[+-]?(?:\d+(?:\.\d*)?|\.\d+)"
)
INTENTIONAL_RE = re.compile(r"//.*\bintentional\b", re.IGNORECASE)
FILE_OPT_OUT_RE = re.compile(r"//\s*intentional-file\b", re.IGNORECASE)
LOOKBACK_LINES = 8
# Permit harmless builder methods between typography markers, but do not cross
# into sibling GPUI children; that would conflate separate text elements.
TEXT_CHAIN_GAP = r"(?:(?!;|\n\s*\.child(?:ren)?\().){0,240}?"

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
            r"\.size\(TextSize::Xs\)" + TEXT_CHAIN_GAP + r"\.muted\(true\)"
            r"|\.muted\(true\)" + TEXT_CHAIN_GAP + r"\.size\(TextSize::Xs\)",
            re.DOTALL,
        ),
        "Text::caption",
        "Xs + muted(true)",
    ),
    (
        re.compile(
            r"\.size\(TextSize::Xs\)" + TEXT_CHAIN_GAP + r"\.color\(theme\.text_muted\)",
            re.DOTALL,
        ),
        "Text::caption",
        "Xs + theme.text_muted",
    ),
    (
        re.compile(
            r"\.size\(TextSize::Xs\)" + TEXT_CHAIN_GAP + r"\.weight\(TextWeight::Bold\)",
            re.DOTALL,
        ),
        "Text::eyebrow",
        "Xs + Bold",
    ),
    (
        re.compile(
            r"\.size\(TextSize::(?:Md|Sm)\)"
            + TEXT_CHAIN_GAP
            + r"\.weight\(TextWeight::Semibold\)",
            re.DOTALL,
        ),
        "Text::section_header",
        "Md/Sm + Semibold",
    ),
    (
        re.compile(
            r"\.size\(TextSize::Sm\)" + TEXT_CHAIN_GAP + r"\.weight\(TextWeight::Medium\)",
            re.DOTALL,
        ),
        "Text::label",
        "Sm + Medium",
    ),
    (
        re.compile(
            r"\.size\(TextSize::Md\)" + TEXT_CHAIN_GAP + r"\.weight\(TextWeight::Bold\)",
            re.DOTALL,
        ),
        "Heading::h4",
        "Md + Bold",
    ),
)


def is_exempt(relative_path: Path) -> bool:
    if relative_path in ALLOWLIST:
        return True
    if relative_path.name in EXEMPT_FILE_NAMES:
        return True
    path_str = relative_path.as_posix()
    return any(fragment in f"/{path_str}" for fragment in EXEMPT_PATH_FRAGMENTS)


def is_theme_source(relative_path: Path) -> bool:
    path_str = relative_path.as_posix()
    return any(fragment in f"/{path_str}" for fragment in THEME_SOURCE_PATH_FRAGMENTS)


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
    """Return a list of (line_number, line_content) for unjustified numeric `px(...)` sites."""
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


def check_spacing(path: Path) -> list[tuple[int, str]]:
    """Return unjustified raw `rems(...)` calls used as gap/padding/margin."""
    violations: list[tuple[int, str]] = []
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return violations

    if FILE_OPT_OUT_RE.search(text):
        return violations

    lines = text.splitlines()
    for idx, line in enumerate(lines):
        stripped = line.lstrip()
        if stripped.startswith("//"):
            continue
        if not RAW_SPACING_RE.search(line):
            continue
        if _is_justified(lines, idx):
            continue
        violations.append((idx + 1, line.rstrip()))
    return violations


def check_color(path: Path, relative_path: Path) -> list[tuple[int, str]]:
    """Return unjustified raw UI color constructors/literals outside theme sources."""
    violations: list[tuple[int, str]] = []
    if is_theme_source(relative_path):
        return violations
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return violations

    if FILE_OPT_OUT_RE.search(text):
        return violations

    lines = text.splitlines()
    for idx, line in enumerate(lines):
        stripped = line.lstrip()
        if stripped.startswith("//"):
            continue
        has_raw_call = RAW_COLOR_RE.search(line) is not None
        has_raw_struct = False
        is_color_return_signature = re.search(r"\bfn\b", stripped) and (
            "Rgba" in stripped or "Hsla" in stripped
        )
        if not is_color_return_signature and COLOR_STRUCT_START_RE.search(line):
            block = "\n".join(lines[idx : min(len(lines), idx + 8)])
            has_raw_struct = COLOR_LITERAL_FIELD_RE.search(block) is not None
        if not has_raw_call and not has_raw_struct:
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
) -> tuple[
    list[tuple[Path, int, str]],
    list[tuple[Path, int, str, str]],
    list[tuple[Path, int, str]],
    list[tuple[Path, int, str]],
]:
    """Return (px_violations, text_builder_violations, spacing_violations, color_violations).

    px_violations:           (path, line, line_content)
    text_builder_violations: (path, line, suggested_constructor, hint)
    spacing_violations:      (path, line, line_content)
    color_violations:        (path, line, line_content)
    """
    px_findings: list[tuple[Path, int, str]] = []
    tb_findings: list[tuple[Path, int, str, str]] = []
    spacing_findings: list[tuple[Path, int, str]] = []
    color_findings: list[tuple[Path, int, str]] = []
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
            for line_no, content in check_spacing(rs):
                spacing_findings.append((rel, line_no, content))
            for line_no, content in check_color(rs, rel):
                color_findings.append((rel, line_no, content))
    return px_findings, tb_findings, spacing_findings, color_findings


def _write_fixture(repo_root: Path, relative: str, content: str) -> None:
    path = repo_root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def run_self_test() -> int:
    """Exercise checker edge cases that are easy to regress with regex edits."""
    with tempfile.TemporaryDirectory() as tmp:
        repo_root = Path(tmp)
        (repo_root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        _write_fixture(
            repo_root,
            "crates/app-gpui/components/bad.rs",
            """
use gpui::*;
use gpui_ui_kit::{Text, TextSize, TextWeight};

fn bad(theme: &Theme) {
    div().mt(px(-6.0));
    div().gap(gpui::px(.5));
    div().gap(rems(0.35));
    div().border_color(gpui::Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
    div().w(px(width));
    let _ = Text::new("caption")
        .line_height(rems(1.0))
        .size(TextSize::Xs)
        .color(theme.text_muted);
    let _ = Text::new("header")
        .size(TextSize::Sm)
        .line_height(rems(1.0))
        .weight(TextWeight::Semibold);
}
""".lstrip(),
        )
        _write_fixture(
            repo_root,
            "crates/app-gpui/components/good.rs",
            """
use gpui::*;

fn good() {
    // intentional: plugin graph label nudge is pixel-exact
    div().mt(px(-6.0));
    // intentional: legacy theme preview uses direct color sample
    div().border_color(gpui::Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
    div().w(px(width));
    // div().gap(px(12.0));
}
""".lstrip(),
        )

        px_findings, tb_findings, spacing_findings, color_findings = collect_violations(repo_root)
        px_locations = {(path.as_posix(), line) for path, line, _ in px_findings}
        tb_locations = {(path.as_posix(), line, ctor) for path, line, ctor, _ in tb_findings}
        spacing_locations = {(path.as_posix(), line) for path, line, _ in spacing_findings}
        color_locations = {(path.as_posix(), line) for path, line, _ in color_findings}

        expected_px = {
            ("crates/app-gpui/components/bad.rs", 5),
            ("crates/app-gpui/components/bad.rs", 6),
        }
        expected_tb = {
            ("crates/app-gpui/components/bad.rs", 12, "Text::caption"),
            ("crates/app-gpui/components/bad.rs", 15, "Text::section_header"),
        }
        expected_spacing = {
            ("crates/app-gpui/components/bad.rs", 7),
        }
        expected_color = {
            ("crates/app-gpui/components/bad.rs", 8),
        }
        if (
            px_locations != expected_px
            or tb_locations != expected_tb
            or spacing_locations != expected_spacing
            or color_locations != expected_color
        ):
            print("self-test failed", file=sys.stderr)
            print(f"expected px: {sorted(expected_px)}", file=sys.stderr)
            print(f"actual px:   {sorted(px_locations)}", file=sys.stderr)
            print(f"expected text builders: {sorted(expected_tb)}", file=sys.stderr)
            print(f"actual text builders:   {sorted(tb_locations)}", file=sys.stderr)
            print(f"expected spacing: {sorted(expected_spacing)}", file=sys.stderr)
            print(f"actual spacing:   {sorted(spacing_locations)}", file=sys.stderr)
            print(f"expected color: {sorted(expected_color)}", file=sys.stderr)
            print(f"actual color:   {sorted(color_locations)}", file=sys.stderr)
            return 1
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="Run checker fixture tests instead of scanning the repository.",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path.cwd(),
        help="Repository root (defaults to current working directory).",
    )
    args = parser.parse_args(argv)

    if args.self_test:
        return run_self_test()

    repo_root = args.repo_root.resolve()
    if not (repo_root / "Cargo.toml").is_file():
        print(f"error: {repo_root} does not look like the sotf repository root", file=sys.stderr)
        return 2

    px_findings, tb_findings, spacing_findings, color_findings = collect_violations(repo_root)
    if not px_findings and not tb_findings and not spacing_findings and not color_findings:
        return 0

    for rel, line_no, content in px_findings:
        print(f"{rel.as_posix()}:{line_no}: {content.strip()}")
    if px_findings:
        print(
            f"\nerror: {len(px_findings)} raw numeric px(...) call(s) outside the design-token allowlist "
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

    for rel, line_no, content in spacing_findings:
        print(f"{rel.as_posix()}:{line_no}: {content.strip()}")
    if spacing_findings:
        print(
            f"\nerror: {len(spacing_findings)} raw rem spacing call(s) outside the design-token allowlist "
            "without an `// intentional:` comment.",
            file=sys.stderr,
        )
        print(
            "Fix by either:\n"
            "  - replacing gap/padding/margin rems(...) with Ds tokens (d.grid/d.gap/d.pad_x/...) "
            "or spacing::* from app/constants.rs\n"
            "  - or adding a `// intentional: [reason]` comment on the same line or within "
            f"{LOOKBACK_LINES} lines above (not crossing a blank line).",
            file=sys.stderr,
        )

    for rel, line_no, content in color_findings:
        print(f"{rel.as_posix()}:{line_no}: {content.strip()}")
    if color_findings:
        print(
            f"\nerror: {len(color_findings)} raw UI color value(s) outside theme source files "
            "without an `// intentional:` comment.",
            file=sys.stderr,
        )
        print(
            "Fix by either:\n"
            "  - using semantic theme fields (theme.background/surface/border/text/accent/warning/"
            "error/success/info), theme.feedback.*, theme.plugin_palette.*, or "
            "Theme::with_opacity(...)\n"
            "  - moving reusable colors into the app theme source of truth\n"
            "  - or adding a `// intentional: [reason]` comment on the same line or within "
            f"{LOOKBACK_LINES} lines above (not crossing a blank line).",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
