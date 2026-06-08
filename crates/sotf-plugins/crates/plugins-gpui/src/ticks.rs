//! Compatibility exports for audio tick mark rendering.
//!
//! Keep `plugins_gpui::ticks::*` available for plugin UI callers, but make
//! `gpui-audio-kit` the single implementation owner so presets and rendering
//! behavior cannot drift between the app and AU/plugin surfaces.

pub use gpui_audio_kit::{ScaleType, TickConfig, TickMark, render_tick_row};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_exports_include_audio_kit_presets() {
        let peak_spread = TickConfig::peak_spread();
        assert_eq!(peak_spread.scale, ScaleType::Linear);
        assert_eq!(peak_spread.max, 24.0);
    }
}
