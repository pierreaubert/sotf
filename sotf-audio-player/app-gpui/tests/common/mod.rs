//! Common test helpers for GPUI component tests
//!
//! Provides test fixtures and utilities for testing component logic.

use std::path::{Path, PathBuf};

/// Test helper: Create a minimal Track for testing album card logic
pub fn create_test_track(path: &str, bit_depth: Option<u32>, sample_rate: Option<u32>) -> TestTrack {
    TestTrack {
        path: PathBuf::from(path),
        bit_depth,
        sample_rate,
    }
}

/// Minimal track structure for testing (mirrors sotf_audio_player::Track)
#[derive(Debug, Clone)]
pub struct TestTrack {
    pub path: PathBuf,
    pub bit_depth: Option<u32>,
    pub sample_rate: Option<u32>,
}

/// Test helper: Create a minimal Album for testing album card logic
pub fn create_test_album(title: &str, tracks: Vec<TestTrack>, dynamic_range: Option<f64>) -> TestAlbum {
    TestAlbum {
        title: title.to_string(),
        tracks,
        dynamic_range,
    }
}

/// Minimal album structure for testing (mirrors sotf_audio_player::Album)
#[derive(Debug, Clone)]
pub struct TestAlbum {
    pub title: String,
    pub tracks: Vec<TestTrack>,
    pub dynamic_range: Option<f64>,
}

// ============================================================================
// Pure function tests (mirrors logic from album_card.rs)
// ============================================================================

/// Format the sample rate and bit depth for display (e.g., "24/44.1k", "16/48k")
/// This is a pure function extracted for testing.
pub fn format_sample_info(bit_depth: Option<u32>, sample_rate: Option<u32>) -> Option<String> {
    match (bit_depth, sample_rate) {
        (Some(bits), Some(rate)) => {
            // Format sample rate: 44100 -> "44.1k", 48000 -> "48k", 96000 -> "96k"
            let rate_str = if rate % 1000 == 0 {
                format!("{}k", rate / 1000)
            } else {
                format!("{:.1}k", rate as f64 / 1000.0)
            };
            Some(format!("{}/{}", bits, rate_str))
        }
        (Some(bits), None) => Some(format!("{}bit", bits)),
        (None, Some(rate)) => {
            let rate_str = if rate % 1000 == 0 {
                format!("{}k", rate / 1000)
            } else {
                format!("{:.1}k", rate as f64 / 1000.0)
            };
            Some(rate_str)
        }
        (None, None) => None,
    }
}

/// Get the audio format (e.g., "FLAC", "MP3") from a file extension
pub fn get_format_from_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str().map(|s| s.to_uppercase()))
}

/// Format the dynamic range for display
pub fn format_dr(dr: Option<f64>) -> Option<String> {
    dr.map(|d| format!("{:.0}", d))
}

/// Album card display mode (mirrors the enum in album_card.rs)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumCardMode {
    Grid,
    List,
    Compact,
}

/// Render height in pixels for a given album card mode
pub fn album_card_height(mode: AlbumCardMode) -> f32 {
    match mode {
        AlbumCardMode::Grid => 180.0,
        AlbumCardMode::List => 80.0,
        AlbumCardMode::Compact => 56.0,
    }
}

/// Format a label with keyboard shortcut indicator
/// e.g., "Threshold" with key 't' -> "[T]hreshold"
pub fn format_shortcut_label(label: &str, shortcut_key: Option<char>) -> String {
    match shortcut_key {
        Some(key) => {
            let key_lower = key.to_ascii_lowercase();
            let label_lower = label.to_lowercase();
            if let Some(pos) = label_lower.find(key_lower) {
                format!(
                    "{}[{}]{}",
                    &label[..pos],
                    label.chars().nth(pos).unwrap().to_ascii_uppercase(),
                    &label[pos + 1..]
                )
            } else {
                format!("[{}] {}", key.to_ascii_uppercase(), label)
            }
        }
        None => label.to_string(),
    }
}

// ============================================================================
// Transfer Curve Logic (mirrors plugins/common.rs)
// ============================================================================

/// Calculate compressor/limiter output dB given input dB
/// Pure function for testing transfer curve logic
pub fn calculate_transfer_output(
    input_db: f64,
    threshold_db: f64,
    ratio: f64,
    knee_db: f64,
    is_limiter: bool,
) -> f64 {
    if is_limiter {
        // Limiter: hard clip at threshold
        input_db.min(threshold_db)
    } else {
        // Compressor: soft knee compression
        if input_db < threshold_db - knee_db / 2.0 {
            input_db
        } else if input_db > threshold_db + knee_db / 2.0 {
            threshold_db + (input_db - threshold_db) / ratio
        } else {
            // Knee region
            let knee_input = input_db - (threshold_db - knee_db / 2.0);
            let knee_ratio = knee_input / knee_db;
            input_db + (knee_ratio * knee_ratio / 2.0) * (1.0 / ratio - 1.0) * knee_db
        }
    }
}

// ============================================================================
// Theme Validation Logic
// ============================================================================

/// Represents theme identifiers (mirrors theme::ThemeId)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    Dark,
    Light,
    Nord,
    Catppuccin,
    Solarized,
    Dracula,
    Gruvbox,
    TokyoNight,
    OneDark,
    Monokai,
}

impl ThemeId {
    pub fn all() -> &'static [ThemeId] {
        &[
            ThemeId::Dark,
            ThemeId::Light,
            ThemeId::Nord,
            ThemeId::Catppuccin,
            ThemeId::Solarized,
            ThemeId::Dracula,
            ThemeId::Gruvbox,
            ThemeId::TokyoNight,
            ThemeId::OneDark,
            ThemeId::Monokai,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ThemeId::Dark => "Dark",
            ThemeId::Light => "Light",
            ThemeId::Nord => "Nord",
            ThemeId::Catppuccin => "Catppuccin",
            ThemeId::Solarized => "Solarized",
            ThemeId::Dracula => "Dracula",
            ThemeId::Gruvbox => "Gruvbox",
            ThemeId::TokyoNight => "Tokyo Night",
            ThemeId::OneDark => "One Dark",
            ThemeId::Monokai => "Monokai",
        }
    }
}

// ============================================================================
// Language Validation Logic
// ============================================================================

/// Represents language identifiers (mirrors i18n::Language)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    French,
    German,
    Spanish,
    Italian,
    Japanese,
    Korean,
    Chinese,
}

impl Language {
    pub fn all() -> &'static [Language] {
        &[
            Language::English,
            Language::French,
            Language::German,
            Language::Spanish,
            Language::Italian,
            Language::Japanese,
            Language::Korean,
            Language::Chinese,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Spanish => "Español",
            Language::Italian => "Italiano",
            Language::Japanese => "日本語",
            Language::Korean => "한국어",
            Language::Chinese => "中文",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
            Language::German => "de",
            Language::Spanish => "es",
            Language::Italian => "it",
            Language::Japanese => "ja",
            Language::Korean => "ko",
            Language::Chinese => "zh",
        }
    }
}

// ============================================================================
// Plugin Parameter Range Helpers
// ============================================================================

/// Verify a parameter value is within valid range
pub fn clamp_parameter(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

/// Calculate normalized value (0.0-1.0) from absolute value
pub fn normalize_parameter(value: f64, min: f64, max: f64) -> f64 {
    if max == min {
        return 0.0;
    }
    (value - min) / (max - min)
}

/// Calculate absolute value from normalized value (0.0-1.0)
pub fn denormalize_parameter(normalized: f64, min: f64, max: f64) -> f64 {
    min + normalized * (max - min)
}

/// Calculate logarithmic normalized value for Hz parameters
pub fn normalize_parameter_log(value: f64, min: f64, max: f64) -> f64 {
    if max == min || min <= 0.0 {
        return 0.0;
    }
    (value.ln() - min.ln()) / (max.ln() - min.ln())
}

/// Calculate absolute value from logarithmic normalized value
pub fn denormalize_parameter_log(normalized: f64, min: f64, max: f64) -> f64 {
    if min <= 0.0 {
        return min;
    }
    (min.ln() + normalized * (max.ln() - min.ln())).exp()
}

// ============================================================================
// Graph Color Utilities (mirrors graphs/common.rs)
// ============================================================================

/// RGBA color representation for testing
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

/// Convert Rgba to u32 color value (mirrors graphs/common.rs)
pub fn rgba_to_u32(rgba: Rgba) -> u32 {
    let r = (rgba.r * 255.0) as u32;
    let g = (rgba.g * 255.0) as u32;
    let b = (rgba.b * 255.0) as u32;
    (r << 16) | (g << 8) | b
}

/// Create a new Rgba with modified alpha
pub fn with_alpha(rgba: Rgba, alpha: f32) -> Rgba {
    Rgba {
        r: rgba.r,
        g: rgba.g,
        b: rgba.b,
        a: alpha,
    }
}

// ============================================================================
// Settings Tab Logic (mirrors app/mod.rs)
// ============================================================================

/// Settings tab identifiers (mirrors app::SettingsTab)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Library,
    Theme,
    Language,
    Keybindings,
    AudioDevice,
}

impl SettingsTab {
    pub fn all() -> &'static [SettingsTab] {
        &[
            SettingsTab::Library,
            SettingsTab::Theme,
            SettingsTab::Language,
            SettingsTab::Keybindings,
            SettingsTab::AudioDevice,
        ]
    }
}

// ============================================================================
// Screen Logic (mirrors app/mod.rs)
// ============================================================================

/// Screen identifiers (mirrors app::Screen)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    Settings,
    HeadphoneEQ,
    SpinoramaEQ,
    RoomEQ,
    Recording,
    Plugins,
}

impl Screen {
    pub fn all() -> &'static [Screen] {
        &[
            Screen::Library,
            Screen::Settings,
            Screen::HeadphoneEQ,
            Screen::SpinoramaEQ,
            Screen::RoomEQ,
            Screen::Recording,
            Screen::Plugins,
        ]
    }
}

// ============================================================================
// EQ Band Logic (mirrors plugins/editing.rs)
// ============================================================================

/// Filter type for EQ bands (mirrors math_audio_iir_fir::BiquadFilterType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

/// EQ Filter representation (mirrors sotf_audio_engine::EQFilter)
#[derive(Debug, Clone)]
pub struct TestEQFilter {
    pub filter_type: FilterType,
    pub frequency: f64,
    pub q: f64,
    pub gain_db: f64,
}

impl TestEQFilter {
    pub fn new(filter_type: FilterType, frequency: f64, q: f64, gain_db: f64) -> Self {
        Self {
            filter_type,
            frequency,
            q,
            gain_db,
        }
    }

    /// Create a default peak filter at 1kHz (matches add_eq_band behavior)
    pub fn default_peak() -> Self {
        Self::new(FilterType::Peak, 1000.0, 1.0, 0.0)
    }
}

/// Add a new EQ band to the filter list
/// Returns the new filter count
pub fn add_eq_band(filters: &mut Vec<TestEQFilter>) -> usize {
    filters.push(TestEQFilter::default_peak());
    filters.len()
}

/// Remove an EQ band at the given index
/// Returns Ok(new count) if successful, Err if index out of bounds
pub fn remove_eq_band(filters: &mut Vec<TestEQFilter>, index: usize) -> Result<usize, String> {
    if index >= filters.len() {
        return Err(format!(
            "Invalid band index {} for {} bands",
            index,
            filters.len()
        ));
    }
    if filters.len() <= 1 {
        return Err("Cannot remove the last EQ band".to_string());
    }
    filters.remove(index);
    Ok(filters.len())
}

/// Validate EQ filter parameters are within acceptable ranges
pub fn validate_eq_filter(filter: &TestEQFilter) -> Result<(), String> {
    // Frequency must be in audible range
    if filter.frequency < 20.0 || filter.frequency > 20000.0 {
        return Err(format!(
            "Frequency {} Hz is outside valid range (20-20000 Hz)",
            filter.frequency
        ));
    }

    // Q must be positive
    if filter.q <= 0.0 {
        return Err(format!("Q factor {} must be positive", filter.q));
    }

    // Q should be reasonable (0.1 to 10)
    if filter.q < 0.1 || filter.q > 10.0 {
        return Err(format!(
            "Q factor {} is outside reasonable range (0.1-10.0)",
            filter.q
        ));
    }

    // Gain should be within reasonable limits (-24 to +24 dB)
    if filter.gain_db < -24.0 || filter.gain_db > 24.0 {
        return Err(format!(
            "Gain {} dB is outside valid range (-24 to +24 dB)",
            filter.gain_db
        ));
    }

    Ok(())
}
