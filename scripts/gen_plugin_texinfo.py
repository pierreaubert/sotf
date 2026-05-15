#!/usr/bin/env python3
"""Generate per-plugin Texinfo files from the existing plugin reference markdown.

Source:
    site/src/content/docs/reference/plugins/<slug>.md   (parameter tables)
    crates/sotf-plugins/crates/sotf-plugin-<slug>/README.md   (algorithm description, optional)

Output:
    docs/manuals/plugins/<slug>.texi
    docs/manuals/plugins/_include.texi   (master @include list)
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REF_DIR = ROOT / "site" / "src" / "content" / "docs" / "reference" / "plugins"
CRATES_DIR = ROOT / "crates" / "sotf-plugins" / "crates"
OUT_DIR = ROOT / "docs" / "manuals" / "plugins"

# Texinfo-unfriendly characters: @ { } need quoting.
TEXI_ESCAPE = str.maketrans({"@": "@@", "{": "@{", "}": "@}"})


def texi(s: str) -> str:
    return s.translate(TEXI_ESCAPE)


@dataclass
class Row:
    name: str
    type_: str
    range_: str
    default: str
    unit: str
    description: str


@dataclass
class Group:
    name: str
    rows: list[Row] = field(default_factory=list)


@dataclass
class Plugin:
    slug: str
    title: str
    description: str
    pre_groups: list[Group]
    per_band_groups: list[Group]
    notes: list[str]


def parse_markdown(path: Path) -> Plugin:
    text = path.read_text()
    # Frontmatter.
    fm_match = re.match(r"^---\n(.*?)\n---\n", text, flags=re.DOTALL)
    fm = fm_match.group(1) if fm_match else ""
    title_m = re.search(r'^title:\s*"?([^"\n]+)"?\s*$', fm, flags=re.MULTILINE)
    desc_m = re.search(r'^description:\s*"?([^"\n]+)"?\s*$', fm, flags=re.MULTILINE)
    title = title_m.group(1).strip() if title_m else path.stem
    desc = desc_m.group(1).strip() if desc_m else ""

    body = text[fm_match.end():] if fm_match else text

    # Split body into sections marked by `### <group>` after the
    # "## Parameters" or "### Per-Band Parameters" headings.
    # We track whether we're currently in the per-band region.
    lines = body.splitlines()
    i = 0
    pre: list[Group] = []
    per: list[Group] = []
    notes: list[str] = []
    current_groups = pre
    current_group: Group | None = None

    def flush_group():
        nonlocal current_group
        if current_group is not None and current_group.rows:
            current_groups.append(current_group)
        current_group = None

    while i < len(lines):
        line = lines[i]
        if re.match(r"^##\s+Parameters\b", line):
            i += 1
            continue
        if re.match(r"^###\s+(Per-Band|Single-Band)\s+Parameters\b", line):
            flush_group()
            current_groups = per
            # Some plugins put the per-band table directly under this
            # header with no further @code{### <group>} subheading;
            # create an implicit group so those rows have a home.
            current_group = Group(name="Per-band")
            i += 1
            continue
        if re.match(r"^###\s+Global Parameters\b", line):
            # Redundant marker before the real global groups; skip.
            flush_group()
            i += 1
            continue
        if line.startswith("### "):
            flush_group()
            current_group = Group(name=line[4:].strip())
            i += 1
            continue
        if line.startswith(":::"):
            # Starlight admonition -> collect plain text until matching :::
            note_lines: list[str] = []
            i += 1
            while i < len(lines) and not lines[i].startswith(":::"):
                if lines[i].strip():
                    note_lines.append(lines[i].strip())
                i += 1
            if note_lines:
                notes.append(" ".join(note_lines))
            i += 1
            continue
        # Parameter table.
        if line.startswith("| Parameter "):
            # Skip header + separator.
            i += 2
            while i < len(lines) and lines[i].startswith("|"):
                cells = [c.strip() for c in lines[i].split("|")[1:-1]]
                if len(cells) >= 6 and current_group is not None:
                    current_group.rows.append(
                        Row(
                            name=cells[0],
                            type_=cells[1],
                            range_=cells[2],
                            default=cells[3],
                            unit=cells[4],
                            description=cells[5],
                        )
                    )
                i += 1
            continue
        i += 1
    flush_group()

    return Plugin(
        slug=path.stem,
        title=title,
        description=desc,
        pre_groups=pre,
        per_band_groups=per,
        notes=notes,
    )


def parse_readme(slug: str) -> tuple[str, list[tuple[str, str]]]:
    """Return (intro_paragraph, [(section_title, body), ...])."""
    p = CRATES_DIR / f"sotf-plugin-{slug}" / "README.md"
    if not p.exists():
        return "", []
    text = p.read_text()
    # Skip the H1 heading.
    text = re.sub(r"^#\s.*\n", "", text, count=1)
    # Split on H2.
    parts = re.split(r"^##\s+(.+?)\n", text, flags=re.MULTILINE)
    intro = parts[0].strip()
    sections: list[tuple[str, str]] = []
    for k in range(1, len(parts), 2):
        title = parts[k].strip()
        body = parts[k + 1].strip() if k + 1 < len(parts) else ""
        sections.append((title, body))
    return intro, sections


def md_inline_to_texi(s: str) -> str:
    """Lightly convert inline markdown to texinfo."""
    s = re.sub(r"`([^`]+)`", lambda m: f"@code{{{texi(m.group(1))}}}", s)
    s = re.sub(r"\*\*([^*]+)\*\*", lambda m: f"@strong{{{texi(m.group(1))}}}", s)
    s = re.sub(r"\*([^*]+)\*", lambda m: f"@emph{{{texi(m.group(1))}}}", s)
    # Plain text fragments get escaped, but we've already substituted code/strong/emph.
    # To keep things simple we leave the result as-is: code spans escape internally,
    # and most prose contains no { } @ chars.
    return s


def fmt_table(group: Group) -> list[str]:
    out = [
        "",
        "@multitable @columnfractions .22 .18 .15 .10 .08 .27",
        "@headitem Parameter @tab Type @tab Range @tab Default @tab Unit @tab Description",
    ]
    for r in group.rows:
        unit = "-" if r.unit in ("", "-") else r.unit
        out.append(
            "@item "
            f"@code{{{texi(r.name)}}} @tab {texi(r.type_)} @tab {texi(r.range_)} "
            f"@tab {texi(r.default)} @tab {texi(unit)} @tab {md_inline_to_texi(r.description)}"
        )
    out.append("@end multitable")
    return out


GENERIC_DEV_NOTE = (
    "All parameters reach the audio thread through the host's "
    "@code{rebuild_cached_parameters} / @code{set_parameter} / "
    "@code{get_parameter} triad. Structural parameters (marked in the "
    "notes below) trigger a rebuild; the rest update in real time with "
    "zero dropouts."
)


# Hand-curated brief algorithm description for plugins whose READMEs do
# not include one. Keys are slugs.
ALGORITHM_HINTS = {
    "compressor": (
        "Standard feedforward dynamics processor. Detector path: optional "
        "Butterworth high-pass on the sidechain, peak or RMS level "
        "detection, attack/release smoothing. Gain computer applies the "
        "ratio above the (soft-knee) threshold. Output stage applies "
        "make-up gain and optional dry/wet mix for parallel compression. "
        "Look-ahead inserts a delay line on the audio path so the detector "
        "sees the future of the signal."
    ),
    "expander": (
        "Inverse of the compressor: gain reduction is applied @emph{below} "
        "the threshold so quiet content gets quieter. Useful as a soft "
        "noise gate. Same detector / smoother / gain computer skeleton as "
        "the compressor."
    ),
    "spectrum-analyzer": (
        "Tap-style analyser. Audio is windowed (Hann by default) and fed "
        "through an FFT; magnitudes are smoothed across frames and passed "
        "to the UI through a lock-free SPSC channel. No processing is "
        "applied to the audio signal."
    ),
    "crossover": (
        "N-way crossover with selectable filter family (Butterworth, "
        "Linkwitz-Riley, linear-phase FIR). Each output band carries the "
        "appropriate complementary filter pair so the bands sum flat at "
        "the crossover points."
    ),
    "hal-input": (
        "Reads audio from a system HAL device into the engine. Used by the "
        "macOS system-wide daemon to capture the virtual HAL driver's "
        "output."
    ),
    "hal-output": (
        "Writes audio from the engine to a system HAL device. Pair with "
        "@code{hal-input} to route system audio through a plugin chain."
    ),
    "resampler": (
        "Polyphase sample-rate converter built on @code{rubato}. Used both "
        "internally (engine SR matching) and as an explicit plugin when "
        "you need to insert a SR change in the middle of a chain."
    ),
}


def write_plugin(plugin: Plugin) -> Path:
    intro, sections = parse_readme(plugin.slug)
    out_path = OUT_DIR / f"{plugin.slug}.texi"
    lines: list[str] = []
    push = lines.append

    push(f"@c -*-texinfo-*-")
    push(f"@c Per-plugin manual: {plugin.slug}")
    push(f"@node {plugin.slug} Plugin")
    push(f"@chapter Plugin: @code{{{texi(plugin.slug)}}}")
    push(f"@cindex {plugin.slug} plugin")
    push("")
    push(md_inline_to_texi(plugin.description))
    push("")

    push("@menu")
    push(f"* {plugin.slug} Plugin Parameters::")
    push(f"* {plugin.slug} Plugin Developer Guide::")
    push("@end menu")
    push("")

    # --- User section -------------------------------------------------
    push(f"@node {plugin.slug} Plugin Parameters")
    push("@section Parameters")
    push("")
    if not plugin.pre_groups and not plugin.per_band_groups:
        push("This plugin has no user-tunable parameters.")
        push("")
    else:
        for g in plugin.pre_groups:
            push("")
            push(f"@subsection {texi(g.name)}")
            lines.extend(fmt_table(g))
            push("")
        if plugin.per_band_groups:
            push("")
            push("@subsection Per-band parameters")
            push("")
            push("The following parameters are repeated for each band:")
            for g in plugin.per_band_groups:
                push("")
                push(f"@subsubheading {texi(g.name)}")
                lines.extend(fmt_table(g))
                push("")
    if plugin.notes:
        push("")
        for n in plugin.notes:
            push(md_inline_to_texi(n))
            push("")

    # --- Developer section --------------------------------------------
    push(f"@node {plugin.slug} Plugin Developer Guide")
    push("@section Developer Guide")
    push("")

    if intro:
        push(md_inline_to_texi(intro))
        push("")
    elif plugin.slug in ALGORITHM_HINTS:
        push(ALGORITHM_HINTS[plugin.slug])
        push("")
    else:
        push(
            "This plugin's crate (@file{crates/sotf-plugins/crates/sotf-plugin-"
            f"{plugin.slug}/}}) holds the implementation. Refer to its "
            "@file{src/lib.rs} for the canonical process loop."
        )
        push("")

    # Re-emit any "What It Does", "Features", "Architecture", "Testing"
    # sections we found in the README.
    interesting = {"What It Does", "Features", "Algorithm", "Architecture",
                   "How It Works", "Implementation", "Testing"}
    for title, body in sections:
        if title not in interesting:
            continue
        push(f"@subsubheading {texi(title)}")
        push("")
        # Convert bullet lists; render fenced code as @example.
        i = 0
        body_lines = body.splitlines()
        while i < len(body_lines):
            ln = body_lines[i]
            if ln.startswith("```"):
                i += 1
                push("@example")
                while i < len(body_lines) and not body_lines[i].startswith("```"):
                    push(texi(body_lines[i]))
                    i += 1
                push("@end example")
                push("")
                i += 1
                continue
            if ln.startswith("- "):
                push("@itemize @bullet")
                while i < len(body_lines) and body_lines[i].startswith("- "):
                    push(f"@item {md_inline_to_texi(body_lines[i][2:])}")
                    i += 1
                push("@end itemize")
                push("")
                continue
            push(md_inline_to_texi(ln))
            i += 1
        push("")

    push("@subsubheading Parameter wiring")
    push("")
    push(GENERIC_DEV_NOTE)
    push("")
    push("@subsubheading Source")
    push("")
    if (CRATES_DIR / f"sotf-plugin-{plugin.slug}").exists():
        push(
            f"@file{{crates/sotf-plugins/crates/sotf-plugin-{plugin.slug}/}}"
        )
    else:
        push(
            f"The @code{{{plugin.slug}}} plugin does not have a dedicated "
            "crate at the time of writing; its implementation lives under "
            "@file{crates/sotf-plugins/crates/sotf-host/} or one of the "
            "multi-component crates. Search the workspace for the plugin "
            f"type key @code{{\"{plugin.slug}\"}} to locate it."
        )
    push("")
    push(f"Run the unit tests with @code{{cargo test -p sotf-plugin-{plugin.slug}}}.")
    push("")

    out_path.write_text("\n".join(lines) + "\n")
    return out_path


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    md_files = sorted(p for p in REF_DIR.glob("*.md") if p.stem != "index")
    written: list[str] = []
    for md in md_files:
        plugin = parse_markdown(md)
        write_plugin(plugin)
        written.append(plugin.slug)

    include = OUT_DIR / "_include.texi"
    include.write_text(
        "@c -*-texinfo-*-\n"
        "@c Generated by scripts/gen_plugin_texinfo.py; do not edit by hand.\n"
        + "".join(f"@include manuals/plugins/{slug}.texi\n" for slug in written)
    )
    print(f"Wrote {len(written)} plugin files to {OUT_DIR}")
    print(f"Updated {include}")


if __name__ == "__main__":
    main()
