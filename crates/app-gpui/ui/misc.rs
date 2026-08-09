#[cfg(target_os = "ios")]
unsafe extern "C" {
    pub(super) fn sotf_ios_pop_remote_command() -> i32;
    pub(super) fn sotf_ios_take_imported_files_json() -> *mut std::ffi::c_char;
    pub(super) fn sotf_ios_take_scanned_qr_payload() -> *mut std::ffi::c_char;
    pub(super) fn sotf_ios_take_dynamic_type_scale() -> f32;
    pub(super) fn sotf_ios_string_free(value: *mut std::ffi::c_char);
}

/// Compute the responsive scale factor for a given window size.
/// Desktop/tablet reference size: 1200×800. Phone-sized windows use an
/// orientation-aware 390×844 reference so iPhone portrait/landscape layouts do
/// not inherit the desktop minimum scale.
pub fn compute_responsive_scale(window_width: f32, window_height: f32) -> f32 {
    let (reference_width, reference_height) =
        responsive_scale_reference_size(window_width, window_height);
    let width_scale = window_width / reference_width;
    let height_scale = window_height / reference_height;
    width_scale.min(height_scale).clamp(0.55, 2.5)
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
