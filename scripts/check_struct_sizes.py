#!/usr/bin/env python3
"""Fail if any Rust struct has more than 30 fields.

Run from the repository root:
    python scripts/check_struct_sizes.py

This is a lightweight structural check; it ignores generated code under
`target/`, `node_modules/`, and paths matching `*_generated*`.
"""

import os
import re
import sys
from pathlib import Path

FIELD_LIMIT = 30

# Existing structs that exceed the limit. New entries need an explicit
# documented decomposition plan; the goal is to drive this list to zero.
ALLOWLIST: set[tuple[str, str]] = {
    # Data table generated from translation sources; decomposition plan TBD.
    ("crates/app-gpui/app/i18n/translations.rs", "Translations"),
    # UI state structs that mix view + transient state; target for MVVM split.
    ("crates/app-gpui/app/state/ui.rs", "UIState"),
    ("crates/app-gpui/ui/tick.rs", "TickSnapshot"),
    # CLI/API compatibility structs. Decompose into domain-specific option
    # groups only when the public command/config surface can absorb it.
    ("crates/app-cli/bin/sotf_player_cli/types.rs", "UpmixerArgs"),
    ("crates/app-gpui/app/config.rs", "Config"),
    ("crates/sotf-types/src/state.rs", "AudioEngineState"),
    # App/remote state models. Planned split: transport/library/server
    # connectivity sub-states with explicit update effects.
    ("crates/app-gpui/app/state/app/remote_state.rs", "RemoteState"),
    # Theme/render state bags. Planned split: palette, typography, meters,
    # control chrome, and per-plugin render models.
    (
        "crates/app-gpui/components/plugins/theme/plugin_theme.rs",
        "PluginTheme",
    ),
    (
        "crates/app-gpui/components/plugins/ui_upmixer/types.rs",
        "UpmixerRenderState",
    ),
    # Optimizer parameter/config DTOs mirror persisted UI/API schemas.
    # Planned split: algorithm, loss, bounds, export, and per-domain groups.
    (
        "crates/sotf-player/src/autoeq/params.rs",
        "OptimizationParamsSerializable",
    ),
    (
        "crates/sotf-player/src/headphone_eq_types.rs",
        "HeadphoneEqOptimizerConfig",
    ),
    (
        "crates/sotf-player/src/room_eq_types/room_eq_optimizer_config.rs",
        "RoomEqOptimizerConfig",
    ),
    (
        "crates/sotf-player/src/spinorama_eq_types.rs",
        "SpinoramaOptimizerConfig",
    ),
    # Shared UI models. Planned split: input selection, optimizer progress,
    # preview/plot state, and export/apply state.
    (
        "crates/sotf-player/src/ui_models/headphone_eq.rs",
        "HeadphoneEqScreenModel",
    ),
    (
        "crates/sotf-player/src/ui_models/recording.rs",
        "RecordingScreenModel",
    ),
    (
        "crates/sotf-player/src/ui_models/room_eq.rs",
        "RoomEqScreenModel",
    ),
    (
        "crates/sotf-player/src/ui_models/spinorama_eq.rs",
        "SpinoramaEqScreenModel",
    ),
    # Audio host/plugin DSP state. Planned split: immutable config, smoothed
    # params, delay/scratch buffers, meters, and analysis caches.
    ("crates/sotf-plugins/crates/sotf-host/src/host/daw_host.rs", "DawHost"),
    (
        "crates/sotf-plugins/crates/sotf-plugin-aae/src/lib/aae_plugin.rs",
        "AaePlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-ab-compare/src/lib/abcompare_plugin.rs",
        "ABComparePlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-downmix/src/lib/downmix_plugin.rs",
        "DownmixPlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-gate/src/lib/gate_plugin.rs",
        "GatePlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-limiter/src/lib/limiter_plugin.rs",
        "LimiterPlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-loudness-compensation/src/lib/loudness_compensation_plugin.rs",
        "LoudnessCompensationPlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-multiband-compressor/src/lib/multiband_compressor_plugin.rs",
        "MultibandCompressorPlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-multiband-expander/src/lib/multiband_expander_plugin.rs",
        "MultibandExpanderPlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-pnd/src/lib/pnd_plugin.rs",
        "PndPlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-saturation/src/lib/saturation_plugin.rs",
        "SaturationPlugin",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-upmixer/src/params.rs",
        "Params",
    ),
    (
        "crates/sotf-plugins/crates/sotf-plugin-xtc/src/config.rs",
        "XtcPluginParams",
    ),
    # Test fixture with many optional response toggles; keep local to tests.
    (
        "crates/sotf-server/crates/sotf-mpd/src/handler/tests.rs",
        "ConfigurableAdapter",
    ),
}


def find_rust_files(root: Path):
    exclude = {
        "target",
        "node_modules",
        ".git",
        ".tokensave",
        ".worktrees",
        "__pycache__",
        "3rdparties",
        ".docker-target",
        "out",
    }
    for path in root.rglob("*.rs"):
        if any(part in exclude or "_generated" in part for part in path.parts):
            continue
        yield path


def count_fields(source: str, start: int) -> int:
    """Count top-level fields inside a struct body starting at source[start]."""
    depth = 0
    fields = 0
    last_comma = -1
    i = start
    length = len(source)
    while i < length:
        ch = source[i]
        if ch in "{([":
            depth += 1
        elif ch in "})]":
            depth -= 1
            if depth == 0:
                break
        elif ch == "," and depth == 1:
            fields += 1
            last_comma = i
        i += 1
    # For braced structs there is usually one more field than commas,
    # unless the last comma is a trailing comma. For tuple structs the
    # comma count is exact.
    if source[start] == "{" and fields > 0:
        trailing_comma = last_comma >= 0 and source[last_comma + 1 : i].strip() == ""
        if not trailing_comma:
            fields += 1
    return fields


# Match:
#   struct Foo { ...
#   pub struct Foo<T> where ... {
#   struct Foo(A, B);
struct_re = re.compile(
    r"\b(struct)\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)(?:\s*<[^>{]*>)?"
    r"(?:\s+where\b[^{(\n]*)?\s*(?P<open>[{(])",
    re.MULTILINE,
)


def check_file(path: Path) -> list[tuple[str, int, int]]:
    offenders = []
    source = path.read_text(encoding="utf-8", errors="ignore")
    for match in struct_re.finditer(source):
        name = match.group("name")
        open_char = match.group("open")
        if open_char == "{":
            # Skip unit structs and struct-like enum variants; require a real body.
            body_start = match.end() - 1
            if source[match.end() : match.end() + 1].strip() == "":
                pass
        else:
            body_start = match.end() - 1
        field_count = count_fields(source, body_start)
        if field_count > FIELD_LIMIT:
            line = source[: match.start()].count("\n") + 1
            offenders.append((name, line, field_count))
    return offenders


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    offenders = []
    for path in sorted(find_rust_files(root)):
        for name, line, count in check_file(path):
            offenders.append((path.relative_to(root), name, line, count))

    if not offenders:
        print(f"OK: no structs exceed {FIELD_LIMIT} fields.")
        return 0

    allowed = []
    new_offenders = []
    for path, name, line, count in offenders:
        if (str(path), name) in ALLOWLIST:
            allowed.append((path, name, line, count))
        else:
            new_offenders.append((path, name, line, count))

    if allowed:
        print(f"WARN: {len(allowed)} allowlisted struct(s) exceed {FIELD_LIMIT} fields:")
        for path, name, line, count in allowed:
            print(f"  {path}:{line}  {name}  ({count} fields)")
        print()

    if not new_offenders:
        print(f"OK: no new structs exceed {FIELD_LIMIT} fields.")
        return 0

    print(f"FAIL: {len(new_offenders)} struct(s) exceed {FIELD_LIMIT} fields:\n")
    for path, name, line, count in new_offenders:
        print(f"  {path}:{line}  {name}  ({count} fields)")
    print("\nEither decompose the struct or add it to the allowlist with a documented plan.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
