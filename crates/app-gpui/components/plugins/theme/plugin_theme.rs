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
use std::collections::HashMap;

mod misc;
mod plugin_theme_id;
mod rack_theme_state;
#[cfg(test)]
mod tests;

pub use plugin_theme_id::*;
pub use rack_theme_state::*;

use misc::lighten;
use misc::with_alpha;

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
        out.layout.design_tokens.knob_label_style = self.knob_label_style;
        out.layout.design_tokens.knob_arc_glow = self.knob_arc_glow;
        out.layout.design_tokens.meter_label_style = self.meter_label_style;
        out.layout.design_tokens.meter_use_gradient = self.meter_use_gradient;
        out.layout.design_tokens.meter_corner_radius = self.meter_corner_radius;
        out.layout.design_tokens.meter_glow = self.meter_glow;

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
        out.layout.font_family = Some(self.font_ui.clone());

        out
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
