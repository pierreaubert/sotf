#[cfg(target_os = "ios")]
unsafe extern "C" {
    pub(super) fn sotf_ios_pop_remote_command() -> i32;
    pub(super) fn sotf_ios_take_imported_files_json() -> *mut std::ffi::c_char;
    pub(super) fn sotf_ios_take_scanned_qr_payload() -> *mut std::ffi::c_char;
    pub(super) fn sotf_ios_take_dynamic_type_scale() -> f32;
    pub(super) fn sotf_ios_string_free(value: *mut std::ffi::c_char);
}

/// Compute the responsive scale factor for a given window size.
///
/// Window geometry determines reflow, never typography. Keep this public
/// compatibility helper neutral; display scaling is owned by GPUI and UI
/// zoom by `compute_combined_scale`.
pub fn compute_responsive_scale(_window_width: f32, _window_height: f32) -> f32 {
    1.0
}

/// Single resolved sizing context shared by the shell and its children.
///
/// Bundles the neutral responsive scale, the user-zoomed combined scale
/// (with configured font-size bounds), and the derived rem/breakpoint units
/// so footer, library, and plugin editors make identical breakpoint
/// decisions (ui.md Phase 1 P0: one resolved sizing context).
#[derive(Clone, Copy, Debug)]
pub struct ResolvedSizing {
    pub responsive_scale: f32,
    pub combined_scale: f32,
    pub effective_rem: f32,
    pub window_width_rems: f32,
}

impl ResolvedSizing {
    pub fn desktop_content_width_rems(self, manually_collapsed: bool) -> f32 {
        let rail = if manually_collapsed {
            3.75
        } else if self.window_width_rems < 50.0 {
            0.0
        } else {
            12.0
        };
        (self.window_width_rems - rail).max(0.0)
    }
}

pub fn resolve_sizing_context(
    window_width: f32,
    window_height: f32,
    font_scale: f32,
    min_font_size_px: Option<f32>,
    max_font_size_px: Option<f32>,
) -> ResolvedSizing {
    let responsive_scale = compute_responsive_scale(window_width, window_height);
    let combined_scale = compute_combined_scale(
        window_width,
        window_height,
        font_scale,
        min_font_size_px,
        max_font_size_px,
    );
    let effective_rem = 16.0 * combined_scale;
    let window_width_rems = if window_width.is_finite() {
        window_width.max(0.0) / effective_rem
    } else {
        0.0
    };
    ResolvedSizing {
        responsive_scale,
        combined_scale,
        effective_rem,
        window_width_rems,
    }
}

/// Effective scale used by rem-based UI geometry after responsive scaling,
/// user zoom, and configured font-size bounds are applied.
pub fn compute_combined_scale(
    window_width: f32,
    window_height: f32,
    font_scale: f32,
    min_font_size_px: Option<f32>,
    max_font_size_px: Option<f32>,
) -> f32 {
    let responsive_scale = compute_responsive_scale(window_width, window_height);
    let (scale_min, scale_max) =
        super::consts::combined_scale_bounds(min_font_size_px, max_font_size_px);
    let font_scale = if font_scale.is_finite() && font_scale > 0.0 {
        font_scale
    } else {
        1.0
    };
    (font_scale * responsive_scale).clamp(scale_min, scale_max)
}

pub fn responsive_scale_reference_size(window_width: f32, window_height: f32) -> (f32, f32) {
    if is_phone_sized_window(window_width, window_height) {
        if window_width >= window_height {
            (844.0, 390.0)
        } else {
            (390.0, 844.0)
        }
    } else {
        (1200.0, 800.0)
    }
}

pub fn is_phone_sized_window(window_width: f32, window_height: f32) -> bool {
    let short_axis = window_width.min(window_height);
    let long_axis = window_width.max(window_height);
    short_axis <= 430.0 && long_axis <= 932.0
}

/// Use a navigation rail when full labels would crowd the active workspace.
/// This is a layout decision and does not change the saved sidebar preference.
pub fn navigation_is_compact(
    window_width: f32,
    combined_scale: f32,
    manually_collapsed: bool,
) -> bool {
    manually_collapsed || window_width < 800.0 * combined_scale
}
