//! Optimization parameter types and UI helpers
//!
//! This module provides:
//! - Re-export of autoeq::Args for optimization parameters
//! - UI dropdown options for algorithm selection, loss functions, etc.
//! - Parameter limits for UI validation

use serde::{Deserialize, Serialize};

// Re-export Args from autoeq for direct use
pub use autoeq::Args as OptimizationParams;

/// Wrapper around autoeq::Args with serde support for UI state persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationParamsSerializable {
    pub num_filters: usize,
    pub sample_rate: f64,
    pub min_db: f64,
    pub max_db: f64,
    pub min_q: f64,
    pub max_q: f64,
    pub min_freq: f64,
    pub max_freq: f64,
    pub peq_model: String,
    pub min_spacing_oct: f64,
    pub spacing_weight: f64,
    pub algo: String,
    pub population: usize,
    pub maxeval: usize,
    pub strategy: String,
    pub de_cr: f64,
    pub adaptive_weight_f: f64,
    pub adaptive_weight_cr: f64,
    pub tolerance: f64,
    pub abs_tolerance: f64,
    pub refine: bool,
    pub local_algo: String,
    pub smooth: bool,
    pub smooth_n: usize,
    pub loss: String,
    pub curve_name: String,
}

impl Default for OptimizationParamsSerializable {
    fn default() -> Self {
        let args = autoeq::Args::speaker_defaults();
        Self::from(&args)
    }
}

impl From<&autoeq::Args> for OptimizationParamsSerializable {
    fn from(args: &autoeq::Args) -> Self {
        Self {
            num_filters: args.num_filters,
            sample_rate: args.sample_rate,
            min_db: args.min_db,
            max_db: args.max_db,
            min_q: args.min_q,
            max_q: args.max_q,
            min_freq: args.min_freq,
            max_freq: args.max_freq,
            peq_model: format!("{:?}", args.peq_model).to_lowercase(),
            min_spacing_oct: args.min_spacing_oct,
            spacing_weight: args.spacing_weight,
            algo: args.algo.clone(),
            population: args.population,
            maxeval: args.maxeval,
            strategy: args.strategy.clone(),
            de_cr: args.recombination,
            adaptive_weight_f: args.adaptive_weight_f,
            adaptive_weight_cr: args.adaptive_weight_cr,
            tolerance: args.tolerance,
            abs_tolerance: args.atolerance,
            refine: args.refine,
            local_algo: args.local_algo.clone(),
            smooth: args.smooth,
            smooth_n: args.smooth_n,
            loss: format!("{:?}", args.loss).to_lowercase().replace('_', "-"),
            curve_name: args.curve_name.clone(),
        }
    }
}

impl OptimizationParamsSerializable {
    /// Convert to autoeq::Args
    pub fn to_args(&self) -> autoeq::Args {
        let mut args = autoeq::Args::speaker_defaults();
        args.num_filters = self.num_filters;
        args.sample_rate = self.sample_rate;
        args.min_db = self.min_db;
        args.max_db = self.max_db;
        args.min_q = self.min_q;
        args.max_q = self.max_q;
        args.min_freq = self.min_freq;
        args.max_freq = self.max_freq;
        args.peq_model = match self.peq_model.as_str() {
            "hp-pk" | "hppk" => autoeq::PeqModel::HpPk,
            "hp-pk-lp" | "hppklp" => autoeq::PeqModel::HpPkLp,
            "ls-pk" | "lspk" => autoeq::PeqModel::LsPk,
            "ls-pk-hs" | "lspkhs" => autoeq::PeqModel::LsPkHs,
            "free-pk-free" | "freepkfree" => autoeq::PeqModel::FreePkFree,
            "free" => autoeq::PeqModel::Free,
            _ => autoeq::PeqModel::Pk,
        };
        args.min_spacing_oct = self.min_spacing_oct;
        args.spacing_weight = self.spacing_weight;
        args.algo = self.algo.clone();
        args.population = self.population;
        args.maxeval = self.maxeval;
        args.strategy = self.strategy.clone();
        args.recombination = self.de_cr;
        args.adaptive_weight_f = self.adaptive_weight_f;
        args.adaptive_weight_cr = self.adaptive_weight_cr;
        args.tolerance = self.tolerance;
        args.atolerance = self.abs_tolerance;
        args.refine = self.refine;
        args.local_algo = self.local_algo.clone();
        args.smooth = self.smooth;
        args.smooth_n = self.smooth_n;
        args.loss = match self.loss.as_str() {
            "speaker-flat" | "speakerflat" => autoeq::LossType::SpeakerFlat,
            "speaker-score" | "speakerscore" => autoeq::LossType::SpeakerScore,
            "headphone-flat" | "headphoneflat" => autoeq::LossType::HeadphoneFlat,
            "headphone-score" | "headphonescore" => autoeq::LossType::HeadphoneScore,
            _ => autoeq::LossType::SpeakerFlat,
        };
        args.curve_name = self.curve_name.clone();
        args
    }
}

// ============================================================================
// Parameter Limits for UI Validation
// ============================================================================

/// Parameter limits for UI validation
pub struct ParamLimits {
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

impl ParamLimits {
    pub const NUM_FILTERS: Self = Self {
        min: 1.0,
        max: 20.0,
        step: 1.0,
    };
    pub const SAMPLE_RATE: Self = Self {
        min: 8000.0,
        max: 192000.0,
        step: 1000.0,
    };
    pub const DB: Self = Self {
        min: -25.0,
        max: 25.0,
        step: 0.5,
    };
    pub const Q: Self = Self {
        min: 0.1,
        max: 10.0,
        step: 0.1,
    };
    pub const FREQUENCY: Self = Self {
        min: 20.0,
        max: 20000.0,
        step: 10.0,
    };
    pub const POPULATION: Self = Self {
        min: 10.0,
        max: 10000.0,
        step: 10.0,
    };
    pub const MAXEVAL: Self = Self {
        min: 100.0,
        max: 100000.0,
        step: 100.0,
    };
    pub const DE_FACTOR: Self = Self {
        min: 0.0,
        max: 2.0,
        step: 0.1,
    };
    pub const DE_CR: Self = Self {
        min: 0.0,
        max: 1.0,
        step: 0.1,
    };
    pub const SPACING_OCT: Self = Self {
        min: 0.01,
        max: 10.0,
        step: 0.1,
    };
    pub const SPACING_WEIGHT: Self = Self {
        min: 0.0,
        max: 1000.0,
        step: 1.0,
    };
}

// ============================================================================
// UI Dropdown Options
// ============================================================================

/// Algorithm options for dropdown
pub const ALGORITHM_OPTIONS: &[(&str, &str)] = &[
    ("autoeq:de", "Auto DE (Recommended)"),
    ("nlopt:isres", "NLOPT ISRES"),
    ("nlopt:ags", "NLOPT AGS"),
    ("nlopt:cobyla", "NLOPT COBYLA"),
    ("nlopt:bobyqa", "NLOPT BOBYQA"),
    ("nlopt:neldermead", "NLOPT Nelder-Mead"),
];

/// DE strategy options
pub const DE_STRATEGY_OPTIONS: &[(&str, &str)] = &[
    ("currenttobest1bin", "Current-to-Best/1/Bin (Recommended)"),
    ("rand1bin", "Rand/1/Bin"),
    ("best1bin", "Best/1/Bin"),
    ("rand2bin", "Rand/2/Bin"),
    ("randtobest1bin", "Rand-to-Best/1/Bin"),
    ("adaptivebin", "Adaptive/Bin (Experimental)"),
];

/// PEQ model options
pub const PEQ_MODEL_OPTIONS: &[(&str, &str)] = &[
    ("pk", "PK - All Peak Filters"),
    ("hp-pk", "HP+PK - Highpass + Peaks"),
    ("hp-pk-lp", "HP+PK+LP - Highpass + Peaks + Lowpass"),
    ("ls-pk", "LS+PK - Low Shelf + Peaks"),
    ("ls-pk-hs", "LS+PK+HS - Low Shelf + Peaks + High Shelf"),
    ("free-pk-free", "Free+PK+Free - Flexible ends, peaks middle"),
    ("free", "Free - All filters flexible"),
];

/// Loss function options for speakers
pub const SPEAKER_LOSS_OPTIONS: &[(&str, &str)] = &[
    ("speaker-flat", "Flat Response"),
    ("speaker-score", "Harman Score"),
];

/// Loss function options for headphones
pub const HEADPHONE_LOSS_OPTIONS: &[(&str, &str)] = &[
    ("headphone-flat", "Flat Response"),
    ("headphone-score", "Harman Score"),
];

/// Curve name options for speakers
pub const CURVE_NAME_OPTIONS: &[(&str, &str)] = &[
    ("Listening Window", "Listening Window"),
    ("On Axis", "On Axis"),
    ("Early Reflections", "Early Reflections"),
    ("Sound Power", "Sound Power"),
    ("Estimated In-Room Response", "Estimated In-Room Response"),
];

/// Local refinement algorithm options
pub const LOCAL_ALGO_OPTIONS: &[(&str, &str)] = &[
    ("cobyla", "COBYLA"),
    ("bobyqa", "BOBYQA"),
    ("newuoa", "NEWUOA"),
];

/// All EQ export format options (id, label, extension).
/// Use `eq_export_format_options()` to get the platform-filtered list.
pub const EQ_EXPORT_FORMAT_OPTIONS: &[(&str, &str, &str)] = &[
    ("json", "JSON", ".json"),
    ("apo", "EqualizerAPO", ".txt"),
    ("rme-channel", "RME TotalMix (Channel)", ".xml"),
    ("rme-room", "RME TotalMix (Room)", ".xml"),
    #[cfg(target_os = "macos")]
    ("aupreset", "Apple AUNBandEQ", ".aupreset"),
    ("camilladsp", "CamillaDSP", ".yaml"),
    #[cfg(target_os = "linux")]
    ("pipewire", "PipeWire", ".conf"),
    #[cfg(target_os = "linux")]
    ("easyeffects", "EasyEffects", ".json"),
    ("wavelet", "Wavelet GraphicEQ", ".txt"),
    ("roon", "Roon DSP", ".json"),
];

/// Get file extension for export format
pub fn get_export_extension(format: &str) -> &'static str {
    EQ_EXPORT_FORMAT_OPTIONS
        .iter()
        .find(|(id, _, _)| *id == format)
        .map(|(_, _, ext)| *ext)
        .unwrap_or(".json")
}

/// Format biquad filters in the specified export format.
///
/// Returns `(content, suggested_filename)` or an error.
pub fn format_peq_export(
    format: &str,
    comment: &str,
    biquads: &[math_audio_iir_fir::Biquad],
    sample_rate: u32,
) -> Result<String, String> {
    let peq: math_audio_iir_fir::Peq = biquads.iter().map(|b| (1.0, b.clone())).collect();

    match format {
        "json" => serde_json::to_string_pretty(biquads).map_err(|e| e.to_string()),
        "apo" => Ok(math_audio_iir_fir::peq_format_apo(comment, &peq)),
        "rme-channel" => Ok(math_audio_iir_fir::peq_format_rme_channel(&peq)),
        "rme-room" => Ok(math_audio_iir_fir::peq_format_rme_room(&peq, &peq)),
        #[cfg(target_os = "macos")]
        "aupreset" => Ok(math_audio_iir_fir::peq_format_aupreset(&peq, comment)),
        "camilladsp" => Ok(math_audio_iir_fir::peq_format_camilladsp(comment, &peq, sample_rate)),
        #[cfg(target_os = "linux")]
        "pipewire" => Ok(math_audio_iir_fir::peq_format_pipewire(comment, &peq)),
        #[cfg(target_os = "linux")]
        "easyeffects" => Ok(math_audio_iir_fir::peq_format_easyeffects(comment, &peq)),
        "wavelet" => Ok(math_audio_iir_fir::peq_format_wavelet(comment, &peq, sample_rate as f64)),
        "roon" => Ok(math_audio_iir_fir::peq_format_roon(comment, &peq)),
        _ => Err(format!("Unknown export format: {format}")),
    }
}

// ============================================================================
// Helper Functions for UI Config Conversion
// ============================================================================

/// Convert loss string to LossType
pub fn parse_loss_type(loss: &str) -> autoeq::LossType {
    match loss {
        "speaker-flat" | "speakerflat" => autoeq::LossType::SpeakerFlat,
        "speaker-score" | "speakerscore" => autoeq::LossType::SpeakerScore,
        "headphone-flat" | "headphoneflat" => autoeq::LossType::HeadphoneFlat,
        "headphone-score" | "headphonescore" => autoeq::LossType::HeadphoneScore,
        _ => autoeq::LossType::SpeakerFlat,
    }
}

/// Convert PEQ model string to PeqModel
pub fn parse_peq_model(model: &str) -> autoeq::PeqModel {
    match model {
        "hp-pk" | "hppk" => autoeq::PeqModel::HpPk,
        "hp-pk-lp" | "hppklp" => autoeq::PeqModel::HpPkLp,
        "ls-pk" | "lspk" => autoeq::PeqModel::LsPk,
        "ls-pk-hs" | "lspkhs" => autoeq::PeqModel::LsPkHs,
        "free-pk-free" | "freepkfree" => autoeq::PeqModel::FreePkFree,
        "free" => autoeq::PeqModel::Free,
        _ => autoeq::PeqModel::Pk,
    }
}
