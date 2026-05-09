//! Plugin chassis theme: a standalone visual layer for plugin UIs.
//!
//! Independent from the global app `Theme`. A `PluginTheme` controls
//! everything inside a plugin chassis: surface gradients, panel backgrounds,
//! section dividers, knob arc color, indicator LEDs, and typography. The
//! rest of the app (library, transport, sidebar) keeps using the global
//! theme.
//!
//! Cascade rule: each plugin instance resolves to the rack's theme by
//! default, but a per-instance override (keyed by plugin index) replaces it.

use crate::theme::Theme;
use gpui::{Rgba, SharedString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The full visual theme for a plugin chassis.
#[derive(Debug, Clone)]
pub struct PluginTheme {
    // ── Surfaces ───────────────────────────────────────────────────────
    /// Top color of the chassis vertical gradient.
    pub chassis_bg_top: Rgba,
    /// Bottom color of the chassis vertical gradient.
    pub chassis_bg_bottom: Rgba,
    /// Outer chassis border.
    pub chassis_border: Rgba,
    /// Background of each section panel inside the chassis.
    pub panel_bg: Rgba,
    /// Recessed-tile background (for read-only readouts).
    pub panel_recess: Rgba,
    /// Hairline divider between sections.
    pub section_divider: Rgba,
    /// Tiny corner-bracket marks on each section.
    pub corner_bracket: Rgba,

    // ── Ink ────────────────────────────────────────────────────────────
    /// Primary text — readouts, titles.
    pub ink_hi: Rgba,
    /// Standard body / label text.
    pub ink: Rgba,
    /// Secondary labels.
    pub ink_mid: Rgba,
    /// De-emphasized text — units, captions.
    pub ink_low: Rgba,
    /// Almost-invisible text — range labels under knobs.
    pub ink_faint: Rgba,

    // ── Accent (the "calibration color") ───────────────────────────────
    /// Base accent — knob arc base.
    pub accent: Rgba,
    /// Bright variant — used on hover / value indicator dot.
    pub accent_bright: Rgba,
    /// Soft glow halo color (alpha already baked in).
    pub accent_glow: Rgba,
    /// The actual stroke color on the value arc.
    pub accent_arc: Rgba,
    /// Faint background ring behind the value arc.
    pub accent_track: Rgba,

    // ── Signal indicators ──────────────────────────────────────────────
    /// LED color when active.
    pub led_active: Rgba,
    /// LED glow halo.
    pub led_glow: Rgba,

    // ── Typography ─────────────────────────────────────────────────────
    /// Italic display font (section titles, plugin name).
    pub font_display: SharedString,
    /// Monospace font (labels, units, readouts).
    pub font_mono: SharedString,
    /// Body / UI sans font.
    pub font_ui: SharedString,

    // ── Dimensions (in raw pixels, all flow through here so every theme
    //     can tune size; renderer treats these as design tokens, not
    //     ad-hoc literals) ─────────────────────────────────────────────
    pub knob_size_px: f32,
    pub arc_stroke_px: f32,
    pub radius_chassis: f32,
    pub radius_panel: f32,
    pub spacing_section: f32,
    pub spacing_knob_row: f32,

    // ── Audio component design tokens overridden by this chassis ─────
    // These mirror the matching fields on `AudioDesignTokens`. `apply_to`
    // writes them into the resolved `Theme.design_tokens` so the knob and
    // meter renderers pick them up automatically.
    /// `AudioDesignTokens::LABEL_BOXED` (default) or `LABEL_UNDERLINED`.
    pub knob_label_style: u8,
    /// Glow halo intensity on the value arc, [0.0, 1.0].
    pub knob_arc_glow: f32,
    /// `AudioDesignTokens::LABEL_BOXED` (default) or `LABEL_UNDERLINED` —
    /// also drives the VerticalSlider chassis.
    pub meter_label_style: u8,
    /// True to render meter fills as a luminance gradient.
    pub meter_use_gradient: bool,
    /// Corner radius (px) for the meter bar/track.
    pub meter_corner_radius: f32,
    /// Glow intensity (0.0–1.0) painted as a colored halo around the meter /
    /// vertical-slider fill. 0.0 = no glow.
    pub meter_glow: f32,
}

impl PluginTheme {
    /// Build a `Theme` whose chassis-affecting fields are replaced by this
    /// plugin theme's palette, while semantic colors (error / warning /
    /// success / meter palette / plugin-type colors) inherit from the
    /// supplied global theme.
    ///
    /// This adapter is the bridge that lets every existing helper which
    /// takes `&Theme` automatically pick up the chassis colors with no
    /// signature changes — saving ~3000 lines of mechanical edits across
    /// the layout and upmixer renderers.
    ///
    /// What gets replaced:
    /// - `background` / `background_secondary` / `background_tertiary`
    /// - `surface` / `surface_hover` / `surface_selected`
    /// - `border` / `border_focused`
    /// - `accent` / `accent_hover` / `accent_muted`
    /// - `text_primary` / `text_secondary` / `text_muted`
    /// - `font_family` (when the chassis theme specifies its own UI font)
    ///
    /// What is left alone:
    /// - Semantic colors (error / warning / success / info)
    /// - Meter colors (normal / warning / clip / peak)
    /// - Plugin-type colors, EQ curve / spectrum / band / channel palettes
    /// - Toast / progress / button colors
    /// - Sizing-related design tokens — only the four "look" tokens
    ///   (`knob_label_style`, `knob_arc_glow`, `meter_*`) are overridden.
    pub fn apply_to(&self, base: &Theme) -> Theme {
        let mut out = base.clone();

        // Audio-component look tokens — patched into the resolved theme so
        // the knob and meter renderers downstream pick them up without
        // signature changes.
        out.design_tokens.knob_label_style = self.knob_label_style;
        out.design_tokens.knob_arc_glow = self.knob_arc_glow;
        out.design_tokens.meter_label_style = self.meter_label_style;
        out.design_tokens.meter_use_gradient = self.meter_use_gradient;
        out.design_tokens.meter_corner_radius = self.meter_corner_radius;
        out.design_tokens.meter_glow = self.meter_glow;

        // Surfaces — the chassis gradient maps to background; panels to
        // background_secondary; recessed tiles to background_tertiary; the
        // generic interactive surface (knob faces, button bg) to `surface`.
        out.background = self.chassis_bg_top;
        out.background_secondary = self.panel_bg;
        out.background_tertiary = self.panel_recess;
        out.surface = self.panel_bg;
        out.surface_hover = lighten(self.panel_bg, 0.06);
        out.surface_selected = self.accent_track;

        // Borders.
        out.border = self.section_divider;
        out.border_focused = self.accent;

        // Accent.
        out.accent = self.accent;
        out.accent_hover = self.accent_bright;
        out.accent_muted = with_alpha(self.accent, 0.20);

        // Ink.
        out.text_primary = self.ink_hi;
        out.text_secondary = self.ink;
        out.text_muted = self.ink_mid;

        // Font family: prefer the chassis-supplied UI font.
        out.font_family = Some(self.font_ui.clone());

        out
    }
}

/// Lighten an opaque color by `amount` (0..=1) by interpolating toward
/// pure white. Used to derive a hover surface from `panel_bg`.
fn lighten(c: Rgba, amount: f32) -> Rgba {
    Rgba {
        r: c.r + (1.0 - c.r) * amount,
        g: c.g + (1.0 - c.g) * amount,
        b: c.b + (1.0 - c.b) * amount,
        a: c.a,
    }
}

/// Replace the alpha channel of a color.
fn with_alpha(c: Rgba, a: f32) -> Rgba {
    Rgba {
        r: c.r,
        g: c.g,
        b: c.b,
        a,
    }
}

/// Stable identifier for a plugin theme preset. Used as the override key
/// and as the value persisted in user prefs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PluginThemeId {
    /// Vintage psychoacoustic instrument — deep graphite + amber.
    #[default]
    Graphite,
    /// Light editorial — warm cream surfaces, tomato accent.
    StudioCream,
    /// High-contrast monochrome — black / white only.
    Brutalist,
}

impl PluginThemeId {
    /// All theme presets in display order.
    pub fn all() -> &'static [PluginThemeId] {
        &[
            PluginThemeId::Graphite,
            PluginThemeId::StudioCream,
            PluginThemeId::Brutalist,
        ]
    }

    /// Human-readable name for UI display.
    pub fn name(&self) -> &'static str {
        match self {
            PluginThemeId::Graphite => "Graphite",
            PluginThemeId::StudioCream => "Studio Cream",
            PluginThemeId::Brutalist => "Brutalist",
        }
    }

    /// Cycle to the next preset (used by the rack header keyboard shortcut).
    pub fn next(&self) -> PluginThemeId {
        match self {
            PluginThemeId::Graphite => PluginThemeId::StudioCream,
            PluginThemeId::StudioCream => PluginThemeId::Brutalist,
            PluginThemeId::Brutalist => PluginThemeId::Graphite,
        }
    }

    /// Build the owned `PluginTheme` for this preset.
    pub fn theme(&self) -> PluginTheme {
        match self {
            PluginThemeId::Graphite => super::graphite::theme(),
            PluginThemeId::StudioCream => super::studio_cream::theme(),
            PluginThemeId::Brutalist => super::brutalist::theme(),
        }
    }
}

/// Per-rack theme state.
///
/// `rack_theme` cascades to every plugin in the rack by default. Entries in
/// `overrides` (keyed by plugin index in the rack) replace that default for
/// that one plugin instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RackThemeState {
    pub rack_theme: PluginThemeId,
    pub overrides: HashMap<usize, PluginThemeId>,
}

impl RackThemeState {
    /// Set the rack-level theme.
    pub fn set_rack_theme(&mut self, theme: PluginThemeId) {
        self.rack_theme = theme;
    }

    /// Pin `theme` to the plugin at `plugin_idx`, replacing the rack default
    /// for that instance.
    pub fn set_override(&mut self, plugin_idx: usize, theme: PluginThemeId) {
        self.overrides.insert(plugin_idx, theme);
    }

    /// Drop the override for `plugin_idx`, reverting to the rack theme.
    pub fn clear_override(&mut self, plugin_idx: usize) {
        self.overrides.remove(&plugin_idx);
    }

    /// Return the resolved theme id for `plugin_idx` (override if present,
    /// else rack default).
    pub fn resolved_id(&self, plugin_idx: usize) -> PluginThemeId {
        self.overrides
            .get(&plugin_idx)
            .copied()
            .unwrap_or(self.rack_theme)
    }

    /// Compact the override map after a plugin is removed at `removed_idx`.
    /// Entries for indices > removed_idx are shifted down by one. The
    /// removed entry itself is dropped.
    pub fn on_plugin_removed(&mut self, removed_idx: usize) {
        let mut compacted: HashMap<usize, PluginThemeId> = HashMap::new();
        for (idx, theme) in self.overrides.drain() {
            if idx == removed_idx {
                continue;
            }
            let new_idx = if idx > removed_idx { idx - 1 } else { idx };
            compacted.insert(new_idx, theme);
        }
        self.overrides = compacted;
    }

    /// Swap override entries for two plugin indices. Called when a plugin
    /// is reordered (move-up / move-down) so per-instance themes follow
    /// their plugin.
    pub fn swap_overrides(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        let ta = self.overrides.remove(&a);
        let tb = self.overrides.remove(&b);
        if let Some(t) = tb {
            self.overrides.insert(a, t);
        }
        if let Some(t) = ta {
            self.overrides.insert(b, t);
        }
    }
}

/// Resolve the active `PluginTheme` for a plugin instance.
///
/// Cascade: per-plugin override (if any) → rack default. Returns an owned
/// snapshot — call site is responsible for caching when redrawing.
pub fn resolve_plugin_theme(
    plugin_idx: usize,
    rack_theme_id: PluginThemeId,
    overrides: &HashMap<usize, PluginThemeId>,
) -> PluginTheme {
    overrides
        .get(&plugin_idx)
        .copied()
        .unwrap_or(rack_theme_id)
        .theme()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_default_is_graphite() {
        assert_eq!(PluginThemeId::default(), PluginThemeId::Graphite);
    }

    #[test]
    fn id_cycle_visits_all_three() {
        let mut id = PluginThemeId::Graphite;
        let mut seen = vec![id];
        for _ in 0..3 {
            id = id.next();
            seen.push(id);
        }
        // Cycle of length 3 returns to start by step 3.
        assert_eq!(seen[0], seen[3]);
        // All three are present in the first three steps.
        assert!(seen.contains(&PluginThemeId::Graphite));
        assert!(seen.contains(&PluginThemeId::StudioCream));
        assert!(seen.contains(&PluginThemeId::Brutalist));
    }

    #[test]
    fn override_wins_over_rack_default() {
        let mut state = RackThemeState {
            rack_theme: PluginThemeId::Graphite,
            ..Default::default()
        };
        state.set_override(2, PluginThemeId::Brutalist);
        assert_eq!(state.resolved_id(2), PluginThemeId::Brutalist);
        assert_eq!(state.resolved_id(0), PluginThemeId::Graphite);
        assert_eq!(state.resolved_id(1), PluginThemeId::Graphite);
    }

    #[test]
    fn clear_override_reverts_to_rack() {
        let mut state = RackThemeState {
            rack_theme: PluginThemeId::Graphite,
            ..Default::default()
        };
        state.set_override(2, PluginThemeId::Brutalist);
        state.clear_override(2);
        assert_eq!(state.resolved_id(2), PluginThemeId::Graphite);
        assert!(state.overrides.is_empty());
    }

    #[test]
    fn swap_overrides_with_two_set_swaps_them() {
        let mut state = RackThemeState {
            rack_theme: PluginThemeId::Graphite,
            ..Default::default()
        };
        state.set_override(1, PluginThemeId::StudioCream);
        state.set_override(2, PluginThemeId::Brutalist);
        state.swap_overrides(1, 2);
        assert_eq!(state.resolved_id(1), PluginThemeId::Brutalist);
        assert_eq!(state.resolved_id(2), PluginThemeId::StudioCream);
    }

    #[test]
    fn swap_overrides_with_one_set_moves_it() {
        let mut state = RackThemeState {
            rack_theme: PluginThemeId::Graphite,
            ..Default::default()
        };
        state.set_override(1, PluginThemeId::Brutalist);
        // index 2 has no override → after swap, index 2 has Brutalist, 1 reverts.
        state.swap_overrides(1, 2);
        assert_eq!(state.resolved_id(1), PluginThemeId::Graphite);
        assert_eq!(state.resolved_id(2), PluginThemeId::Brutalist);
        assert_eq!(state.overrides.len(), 1);
    }

    #[test]
    fn swap_overrides_self_is_noop() {
        let mut state = RackThemeState {
            rack_theme: PluginThemeId::Graphite,
            ..Default::default()
        };
        state.set_override(1, PluginThemeId::Brutalist);
        state.swap_overrides(1, 1);
        assert_eq!(state.resolved_id(1), PluginThemeId::Brutalist);
    }

    #[test]
    fn on_plugin_removed_shifts_higher_indices_down() {
        let mut state = RackThemeState {
            rack_theme: PluginThemeId::Graphite,
            ..Default::default()
        };
        state.set_override(0, PluginThemeId::Brutalist);
        state.set_override(2, PluginThemeId::StudioCream);
        state.set_override(4, PluginThemeId::Brutalist);

        // Remove plugin at index 2: index 0 stays, index 4 → 3, index 2 dropped.
        state.on_plugin_removed(2);

        assert_eq!(state.resolved_id(0), PluginThemeId::Brutalist);
        assert_eq!(state.resolved_id(2), PluginThemeId::Graphite); // dropped
        assert_eq!(state.resolved_id(3), PluginThemeId::Brutalist); // was 4
        assert_eq!(state.overrides.len(), 2);
    }

    #[test]
    fn apply_to_overrides_chassis_fields_only() {
        use crate::theme::ThemeId;
        let base = crate::theme::Theme::from_id(ThemeId::Dark);
        let chassis = PluginThemeId::Brutalist.theme();
        let merged = chassis.apply_to(&base);

        // Chassis-affecting fields are replaced with PluginTheme values.
        assert_eq!(merged.background, chassis.chassis_bg_top);
        assert_eq!(merged.background_secondary, chassis.panel_bg);
        assert_eq!(merged.surface, chassis.panel_bg);
        assert_eq!(merged.border, chassis.section_divider);
        assert_eq!(merged.accent, chassis.accent);
        assert_eq!(merged.text_primary, chassis.ink_hi);
        assert_eq!(merged.text_secondary, chassis.ink);
        assert_eq!(merged.text_muted, chassis.ink_mid);

        // Semantic colors are inherited from the base theme — the brutalist
        // chassis must not erase the global error / warning palette.
        assert_eq!(merged.error, base.error);
        assert_eq!(merged.warning, base.warning);
        assert_eq!(merged.success, base.success);

        // Meter palette is preserved (used by the meter plugin family).
        assert_eq!(merged.meter_normal, base.meter_normal);
        assert_eq!(merged.meter_warning, base.meter_warning);
        assert_eq!(merged.meter_clip, base.meter_clip);
    }

    #[test]
    fn set_override_does_not_silently_validate_index() {
        // The state object itself does not know how many plugins exist —
        // bounds checking is the caller's responsibility. This test pins
        // that contract so a future refactor doesn't accidentally start
        // dropping out-of-range overrides (which would also paper over
        // legitimate caller bugs).
        let mut state = RackThemeState::default();
        state.set_override(99, PluginThemeId::Brutalist);
        assert_eq!(state.overrides.len(), 1);
        assert_eq!(state.resolved_id(99), PluginThemeId::Brutalist);
    }

    #[test]
    fn each_preset_resolves_to_a_distinct_theme() {
        // Sanity: presets must produce theme structs (no panics in match arms).
        let _g = PluginThemeId::Graphite.theme();
        let _s = PluginThemeId::StudioCream.theme();
        let _b = PluginThemeId::Brutalist.theme();

        // Themes should differ in their core accent — otherwise users can't
        // tell them apart.
        let g = PluginThemeId::Graphite.theme();
        let s = PluginThemeId::StudioCream.theme();
        let b = PluginThemeId::Brutalist.theme();
        assert!(
            g.accent_arc != s.accent_arc || g.accent != s.accent,
            "Graphite and StudioCream must have different accents"
        );
        assert!(
            g.accent_arc != b.accent_arc || g.accent != b.accent,
            "Graphite and Brutalist must have different accents"
        );
    }

    #[test]
    fn resolve_uses_override_when_present() {
        let mut overrides = HashMap::new();
        overrides.insert(1, PluginThemeId::Brutalist);
        let theme = resolve_plugin_theme(1, PluginThemeId::Graphite, &overrides);
        // Brutalist's accent should match the brutalist preset.
        assert_eq!(
            theme.accent_arc,
            PluginThemeId::Brutalist.theme().accent_arc
        );
    }

    #[test]
    fn resolve_uses_rack_default_when_no_override() {
        let overrides = HashMap::new();
        let theme = resolve_plugin_theme(7, PluginThemeId::Graphite, &overrides);
        assert_eq!(theme.accent_arc, PluginThemeId::Graphite.theme().accent_arc);
    }
}
