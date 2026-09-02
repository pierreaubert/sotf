#!/usr/bin/env python3
"""Generate QA pseudo-locale constructors from English translation initializers.

The generated Rust stays checked in so normal builds do not need Python. Run
this script after editing app-gpui translation structs; CI/component tests use
``--check`` to reject drift.
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TRANSLATIONS = ROOT / "crates/app-gpui/app/i18n/translations.rs"
GENERATED = ROOT / "crates/app-gpui/app/i18n/translations_pseudo_generated.rs"


def matching(source: str, start: int, opening: str, closing: str) -> int:
    depth = 0
    index = start
    while index < len(source):
        char = source[index]
        if char == '"':
            index += 1
            while index < len(source):
                if source[index] == "\\":
                    index += 2
                elif source[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
            continue
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = len(source) if newline < 0 else newline + 1
            continue
        if source.startswith("/*", index):
            end = source.find("*/", index + 2)
            if end < 0:
                raise ValueError("unterminated block comment")
            index = end + 2
            continue
        if char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index
        index += 1
    raise ValueError(f"unmatched {opening!r} at byte {start}")


def transform_expression(source: str) -> str:
    out: list[str] = []
    index = 0
    while index < len(source):
        if source.startswith("Language::English", index):
            out.append("Language::Pseudo")
            index += len("Language::English")
            continue
        if source.startswith("&[", index):
            end = matching(source, index + 1, "[", "]")
            inner = transform_expression(source[index + 2 : end])
            out.append(f"Box::leak(Box::new([{inner}]))")
            index = end + 1
            continue
        if source[index] == '"':
            end = index + 1
            while end < len(source):
                if source[end] == "\\":
                    end += 2
                elif source[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
            out.append(f"pseudo_static({source[index:end]})")
            index = end
            continue
        out.append(source[index])
        index += 1
    transformed = "".join(out)
    return re.sub(
        r"(?m)^(?P<indent>\s*)language,\s*$",
        r"\g<indent>language: Language::Pseudo,",
        transformed,
    )


def function_body(source: str, function_match: re.Match[str]) -> tuple[int, int]:
    opening = source.find("{", function_match.end() - 1)
    return opening, matching(source, opening, "{", "}")


FOR_LANGUAGE = re.compile(
    r"(?:pub(?:\([^)]*\))?\s+)?fn for_language\s*\(\s*language:\s*Language\s*\)\s*->\s*Self\s*\{"
)


def add_pseudo_dispatch(source: str) -> str:
    insertions: list[tuple[int, str]] = []
    for match in FOR_LANGUAGE.finditer(source):
        opening, closing = function_body(source, match)
        body = source[opening:closing]
        if "Language::Pseudo" in body:
            continue
        match_pos = source.find("match language {", opening, closing)
        if match_pos < 0:
            raise ValueError(f"for_language at byte {match.start()} has no language match")
        brace = source.find("{", match_pos)
        insertions.append((brace + 1, "\n            Language::Pseudo => Self::pseudo(),"))
    for position, text in reversed(insertions):
        source = source[:position] + text + source[position:]
    return source


def english_initializer(source: str, struct_name: str, impl_start: int) -> str | None:
    impl_open = source.find("{", impl_start)
    impl_close = matching(source, impl_open, "{", "}")
    impl_source = source[impl_open:impl_close]

    if struct_name == "Translations":
        english = re.search(r"pub fn english\s*\(\s*\)\s*->\s*Self\s*\{", impl_source)
        if english is None:
            raise ValueError("Translations::english not found")
        absolute = impl_open + english.end()
        initializer = source.find("Self {", absolute, impl_close)
    else:
        language = FOR_LANGUAGE.search(impl_source)
        if language is None:
            return None
        body_start = impl_open + language.end() - 1
        body_end = matching(source, body_start, "{", "}")
        english = source.find("Language::English =>", body_start, body_end)
        if english < 0:
            raise ValueError(f"{struct_name} has no English translation arm")
        initializer = source.find("Self {", english, body_end)

    if initializer < 0:
        return None
    opening = source.find("{", initializer)
    closing = matching(source, opening, "{", "}")
    return source[initializer : closing + 1]


def generate(source: str) -> str:
    impls = list(re.finditer(r"^impl\s+(\w+)\s*\{", source, re.MULTILINE))
    blocks: list[str] = []
    for impl in impls:
        name = impl.group(1)
        initializer = english_initializer(source, name, impl.start())
        if initializer is None:
            continue
        transformed = transform_expression(initializer)
        blocks.append(
            f"impl {name} {{\n"
            "    pub(super) fn pseudo() -> Self {\n"
            f"        {transformed}\n"
            "    }\n"
            "}\n"
        )

    if not blocks:
        raise ValueError("no translation constructors found")
    return (
        "// @generated by scripts/generate_pseudo_locale.py; do not edit by hand.\n"
        "// The Pseudo language is QA-only and excluded from Language::all().\n\n"
        "#[rustfmt::skip]\n"
        "mod pseudo_generated {\n"
        "use super::*;\n\n"
        + "\n".join(blocks)
        + "}\n"
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    source = TRANSLATIONS.read_text()
    updated = add_pseudo_dispatch(source)
    generated = generate(updated)

    if args.check:
        if source != updated:
            print("translations.rs is missing Language::Pseudo dispatch arms")
            return 1
        if not GENERATED.exists() or GENERATED.read_text() != generated:
            print("translations_pseudo_generated.rs is stale")
            return 1
        return 0

    if source != updated:
        TRANSLATIONS.write_text(updated)
    GENERATED.write_text(generated)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
