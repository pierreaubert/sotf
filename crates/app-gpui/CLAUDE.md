# app-gpui (lib: `sotf_audio_player_gpui`, binary: `SotF`)

GPUI-based desktop music player with GPU-accelerated rendering.

## Key Features

- Modern desktop UI via GPUI framework
- Real-time spectrum and loudness visualization
- Plugin configuration UI
- Library management
- Read `GPUI.md` at the project root before working on GPUI code

## Architecture

- GPU-accelerated rendering via GPUI (Metal on macOS)
- Business logic delegated to the `player` crate
- Uses `gpui-ui-kit` for reusable components
- Uses `gpui-d3rs` / `gpui-px` for charts and visualizations

## Features

- `hal` - macOS HAL support

## Testing

```bash
cargo test -p app-gpui --lib
cargo check -p app-gpui && cargo clippy -p app-gpui
```

Note: `test = false` in lib config due to GPUI macro stack overflow issues in syn.

## Test Suites

Extensive testing: e2e, negative, proptest, component, lifecycle, event_integration, state_machine, config, migration.

## Running

```bash
cargo run --bin SotF --release
```

## Design-token drift guard

Spacing, text size, corner radius, and icon size must flow through the design
system (`components/design.rs` → `Ds::from_cx(cx)`, or `spacing::*`/`radius::*`
in `app/constants.rs`). This keeps fonts, icons, and layout scaling together
when the user invokes the font-zoom actions.

`scripts/check-design-tokens.py` fails CI when raw `px(N.0)` appears in
`components/` or `ui/` outside the allowlist or without justification. Run it
locally before committing UI changes:

```bash
python3 scripts/check-design-tokens.py
```

Legitimate exceptions:
- Same-line `// intentional: <reason>` trailing comment.
- `// intentional: <reason>` comment within 8 lines above (not crossing a
  blank line).
- File-level `// intentional-file: <reason>` marker anywhere in the file —
  use this for chart/meter/table code where pixel dimensions are
  intrinsically layout-driven.

## Typography conventions

Two typography APIs coexist and (as of Typography Phase 2) resolve to
identical rem values — but they remain distinct surfaces. Pick the right one
per role using the table below.

- `gpui_ui_kit::Text::new(content)...` is the preferred API for UI text. It
  carries theme color semantics (`muted`, `color`) alongside size/weight.
- `.text_size(d.text_*)` on a raw `div()` is for cases where `Text` isn't
  appropriate (e.g. inline spans inside charts, computed labels, or elements
  that need non-text children alongside styled text).

**Role → variant map.** Prefer the semantic `Text::*` constructors
(`Text::eyebrow`, `Text::section_header`, `Text::body`, `Text::label`,
`Text::caption`) over rebuilding the same `.size().weight()` chain by hand.
Each constructor encodes the convention in one place, so changing a role's
styling later only needs to update one function in `gpui-ui-kit/src/text.rs`.

| Role | Constructor / pattern | Size | Weight | Extras |
|------|----------------------|------|--------|--------|
| Eyebrow label (e.g. "RECORDING NAME", "SAVE LOCATION" — small, caps, accent-colored kicker above a card) | `Text::eyebrow(content).color(theme.accent)` | `Xs` (~12 px) | `Bold` | Caller provides UPPERCASE content and the accent color; constructor pins size+weight. |
| Screen / dialog title | `Heading::h1(content)` or `Text::new(content).size(TextSize::Lg).weight(TextWeight::Bold)` | `Lg` (~18 px) | `Bold` | `text_primary` color. Reserve for the top of a screen or dialog. |
| Section header (inside a panel, card, or dialog) | `Text::section_header(content)` | `Md` (~16 px) | `Semibold` | `text_primary` color. One level below screen/dialog title. |
| Body text / descriptions | `Text::body(content)` | `Md` (~16 px) | `Normal` | Default `text_secondary` color. Use for paragraphs, help text on its own row, and anything that should read as normal body copy. |
| Inline form-field label, table column header | `Text::label(content)` | `Sm` (~14 px) | `Medium` | `text_secondary` color. Smaller than body so the label reads as secondary to its value. |
| Data value in a table cell or row | `Text::new(content).size(TextSize::Sm)` | `Sm` (~14 px) | `Normal` | Match the paired label size unless the value needs emphasis. |
| Status / info message (toast, inline alert) | `Text::new(content).size(TextSize::Sm)` | `Sm` (~14 px) | `Normal` or `Medium` | Prominent but not overwhelming; use the theme's semantic color (`theme.error`, `theme.warning`, `theme.success`) not raw accent. |
| Caption / helper text (field hint, timestamp, unit suffix) | `Text::caption(content)` | `Xs` (~12 px) | `Normal` | `muted(true)` pulls `theme.text_muted`. |
| Badge content | `Badge::new(content)` (handles its own sizing) | — | — | Don't hand-build; use the `Badge` component. |
| Chart axis tick, micro-type on a meter, tiny diagnostic readout | `.text_size(d.text_xs)` on a raw `div()` | rems(0.625) (~10 px) | varies | `d.text_xs` is intentionally smaller than `TextSize::Xs`; reserve for chart internals and similar dense micro-type. |

**Rules of thumb**:
- A `.weight(Bold)` call paired with `.size(TextSize::Xs)` usually means
  "eyebrow label" — migrate to `Text::eyebrow`. If it's *not* an eyebrow
  (no caps, no accent color), it's probably a compressed section header
  that should become `Text::section_header`.
- `.muted(true)` paired with `.size(TextSize::Xs)` is the caption pattern —
  migrate to `Text::caption`.
- The `TextSize::Md` default is real `text_base()` (~16 px) since Typography
  Phase 1 — don't compensate for the old buggy Md→Sm aliasing by explicitly
  reaching for `TextSize::Sm` where `Md` is what you mean.

**Changing a convention.** Update the constructor in
`crates/gpui-toolkit/gpui-ui-kit/src/text.rs` and the pinning tests in
`crates/gpui-toolkit/gpui-ui-kit/tests/components/text_test.rs` together.
The tests read the constructor's state via `Text::preset_style()` so any
change surfaces immediately — audit the corresponding call sites before
updating the expected values.
