//! Centralized Parameter Specifications
//!
//! This module defines all plugin parameter specifications in one place.
//! Both plugin implementations and UI code should reference these constants
//! to ensure consistency and single-source-of-truth for parameter ranges,
//! defaults, and metadata.

// ============================================================================
// Gain Plugin
// ============================================================================

pub mod gain {
    pub const GAIN_DB_DEFAULT: f32 = 0.0;
    pub const GAIN_DB_MIN: f32 = -60.0;
    pub const GAIN_DB_MAX: f32 = 20.0;
}

// ============================================================================
// Compressor Plugin
// ============================================================================

pub mod compressor {
    pub const THRESHOLD_DEFAULT: f32 = -20.0;
    pub const THRESHOLD_MIN: f32 = -60.0;
    pub const THRESHOLD_MAX: f32 = 0.0;

    pub const RATIO_DEFAULT: f32 = 4.0;
    pub const RATIO_MIN: f32 = 1.0;
    pub const RATIO_MAX: f32 = 20.0;

    pub const ATTACK_DEFAULT: f32 = 5.0;
    pub const ATTACK_MIN: f32 = 0.1;
    pub const ATTACK_MAX: f32 = 100.0;

    pub const RELEASE_DEFAULT: f32 = 50.0;
    pub const RELEASE_MIN: f32 = 10.0;
    pub const RELEASE_MAX: f32 = 1000.0;

    pub const KNEE_DEFAULT: f32 = 6.0;
    pub const KNEE_MIN: f32 = 0.0;
    pub const KNEE_MAX: f32 = 20.0;

    pub const MAKEUP_GAIN_DEFAULT: f32 = 0.0;
    pub const MAKEUP_GAIN_MIN: f32 = -24.0;
    pub const MAKEUP_GAIN_MAX: f32 = 24.0;

    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const AUTO_MAKEUP_DEFAULT: bool = false;
    pub const LINK_CHANNELS_DEFAULT: bool = true;

    pub const SIDECHAIN_HPF_HZ_DEFAULT: f32 = 80.0;
    pub const SIDECHAIN_HPF_HZ_MIN: f32 = 0.0;
    pub const SIDECHAIN_HPF_HZ_MAX: f32 = 200.0;
}

// ============================================================================
// Gate Plugin
// ============================================================================

pub mod gate {
    pub const THRESHOLD_DEFAULT: f32 = -40.0;
    pub const THRESHOLD_MIN: f32 = -80.0;
    pub const THRESHOLD_MAX: f32 = 0.0;

    pub const RATIO_DEFAULT: f32 = 10.0;
    pub const RATIO_MIN: f32 = 1.0;
    pub const RATIO_MAX: f32 = 100.0;

    pub const ATTACK_DEFAULT: f32 = 1.0;
    pub const ATTACK_MIN: f32 = 0.1;
    pub const ATTACK_MAX: f32 = 50.0;

    pub const HOLD_DEFAULT: f32 = 10.0;
    pub const HOLD_MIN: f32 = 0.0;
    pub const HOLD_MAX: f32 = 1000.0;

    pub const RELEASE_DEFAULT: f32 = 100.0;
    pub const RELEASE_MIN: f32 = 10.0;
    pub const RELEASE_MAX: f32 = 2000.0;

    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const LINK_CHANNELS_DEFAULT: bool = true;

    pub const SIDECHAIN_HPF_HZ_DEFAULT: f32 = 0.0;
    pub const SIDECHAIN_HPF_HZ_MIN: f32 = 0.0;
    pub const SIDECHAIN_HPF_HZ_MAX: f32 = 200.0;
}

// ============================================================================
// Expander Plugin
// ============================================================================

pub mod expander {
    pub const THRESHOLD_DEFAULT: f32 = -40.0;
    pub const THRESHOLD_MIN: f32 = -80.0;
    pub const THRESHOLD_MAX: f32 = 0.0;

    pub const RATIO_DEFAULT: f32 = 2.0;
    pub const RATIO_MIN: f32 = 1.0;
    pub const RATIO_MAX: f32 = 20.0;

    pub const ATTACK_DEFAULT: f32 = 1.0;
    pub const ATTACK_MIN: f32 = 0.1;
    pub const ATTACK_MAX: f32 = 50.0;

    pub const RELEASE_DEFAULT: f32 = 100.0;
    pub const RELEASE_MIN: f32 = 10.0;
    pub const RELEASE_MAX: f32 = 2000.0;

    pub const RANGE_DEFAULT: f32 = 40.0;
    pub const RANGE_MIN: f32 = 0.0;
    pub const RANGE_MAX: f32 = 80.0;

    pub const KNEE_DEFAULT: f32 = 6.0;
    pub const KNEE_MIN: f32 = 0.0;
    pub const KNEE_MAX: f32 = 20.0;

    pub const HYSTERESIS_DEFAULT: f32 = 4.0;
    pub const HYSTERESIS_MIN: f32 = 0.0;
    pub const HYSTERESIS_MAX: f32 = 12.0;

    pub const HOLD_DEFAULT: f32 = 10.0;
    pub const HOLD_MIN: f32 = 0.0;
    pub const HOLD_MAX: f32 = 500.0;

    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const LINK_CHANNELS_DEFAULT: bool = true;

    pub const SIDECHAIN_HPF_HZ_DEFAULT: f32 = 80.0;
    pub const SIDECHAIN_HPF_HZ_MIN: f32 = 0.0;
    pub const SIDECHAIN_HPF_HZ_MAX: f32 = 500.0;
}

// ============================================================================
// Limiter Plugin
// ============================================================================

pub mod limiter {
    pub const THRESHOLD_DEFAULT: f32 = -0.1;
    pub const THRESHOLD_MIN: f32 = -20.0;
    pub const THRESHOLD_MAX: f32 = 0.0;

    pub const RELEASE_DEFAULT: f32 = 50.0;
    pub const RELEASE_MIN: f32 = 10.0;
    pub const RELEASE_MAX: f32 = 1000.0;

    pub const LOOKAHEAD_DEFAULT: f32 = 5.0;
    pub const LOOKAHEAD_MIN: f32 = 0.0;
    pub const LOOKAHEAD_MAX: f32 = 20.0;

    pub const SOFT_DEFAULT: bool = false;

    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;
}

// ============================================================================
// Delay Plugin
// ============================================================================

pub mod delay {
    pub const DELAY_MS_DEFAULT: f32 = 100.0;
    pub const DELAY_MS_MIN: f32 = 0.1;
    pub const DELAY_MS_MAX: f32 = 5000.0;

    pub const FEEDBACK_DEFAULT: f32 = 0.3;
    pub const FEEDBACK_MIN: f32 = 0.0;
    pub const FEEDBACK_MAX: f32 = 0.95;

    pub const MIX_DEFAULT: f32 = 0.5;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;
}

// ============================================================================
// Loudness Compensation Plugin
// ============================================================================

pub mod loudness_compensation {
    pub const LOW_FREQ_DEFAULT: f32 = 100.0;
    pub const LOW_FREQ_MIN: f32 = 20.0;
    pub const LOW_FREQ_MAX: f32 = 500.0;

    pub const LOW_GAIN_DEFAULT: f32 = 6.0;
    pub const LOW_GAIN_MIN: f32 = -20.0;
    pub const LOW_GAIN_MAX: f32 = 20.0;

    pub const HIGH_FREQ_DEFAULT: f32 = 10000.0;
    pub const HIGH_FREQ_MIN: f32 = 2000.0;
    pub const HIGH_FREQ_MAX: f32 = 20000.0;

    pub const HIGH_GAIN_DEFAULT: f32 = 6.0;
    pub const HIGH_GAIN_MIN: f32 = -20.0;
    pub const HIGH_GAIN_MAX: f32 = 20.0;
}

// ============================================================================
// Matrix Plugin
// ============================================================================

pub mod matrix {
    pub const GAIN_DEFAULT: f32 = 0.0; // Identity matrix has 1.0 on diagonal
    pub const GAIN_MIN: f32 = 0.0;
    pub const GAIN_MAX: f32 = 1.0;
}

// ============================================================================
// Upmixer Plugin
// ============================================================================

pub mod upmixer {
    pub const SPEAKER_CONFIG_DEFAULT: i32 = 0;
    pub const SPEAKER_CONFIG_MIN: i32 = 0;
    pub const SPEAKER_CONFIG_MAX: i32 = 9;

    pub const GAIN_FRONT_DIRECT_DEFAULT: f32 = 1.0;
    pub const GAIN_FRONT_DIRECT_MIN: f32 = 0.0;
    pub const GAIN_FRONT_DIRECT_MAX: f32 = 2.0;

    pub const GAIN_FRONT_AMBIENT_DEFAULT: f32 = 0.5;
    pub const GAIN_FRONT_AMBIENT_MIN: f32 = 0.0;
    pub const GAIN_FRONT_AMBIENT_MAX: f32 = 2.0;

    pub const GAIN_REAR_AMBIENT_DEFAULT: f32 = 1.0;
    pub const GAIN_REAR_AMBIENT_MIN: f32 = 0.0;
    pub const GAIN_REAR_AMBIENT_MAX: f32 = 2.0;

    pub const GAIN_HEIGHT_DEFAULT: f32 = 1.0;
    pub const GAIN_HEIGHT_MIN: f32 = 0.0;
    pub const GAIN_HEIGHT_MAX: f32 = 2.0;

    pub const LFE_GAIN_DEFAULT: f32 = 1.0;
    pub const LFE_GAIN_MIN: f32 = 0.0;
    pub const LFE_GAIN_MAX: f32 = 2.0;

    pub const LFE_CUTOFF_HZ_DEFAULT: f32 = 120.0;
    pub const LFE_CUTOFF_HZ_MIN: f32 = 20.0;
    pub const LFE_CUTOFF_HZ_MAX: f32 = 180.0;

    pub const STEREO_WIDTH_DEFAULT: f32 = 0.5;
    pub const STEREO_WIDTH_MIN: f32 = 0.0;
    pub const STEREO_WIDTH_MAX: f32 = 1.0;

    pub const CENTER_SPREAD_DEFAULT: f32 = 0.0;
    pub const CENTER_SPREAD_MIN: f32 = 0.0;
    pub const CENTER_SPREAD_MAX: f32 = 1.0;

    pub const BANDPASS_HZ_DEFAULT: f32 = 250.0;
    pub const BANDPASS_HZ_MIN: f32 = 150.0;
    pub const BANDPASS_HZ_MAX: f32 = 350.0;

    // Surround routing parameters
    pub const SURROUND_DIRECT_BLEED_DEFAULT: f32 = 0.50;
    pub const SURROUND_DIRECT_BLEED_MIN: f32 = 0.0;
    pub const SURROUND_DIRECT_BLEED_MAX: f32 = 1.0;

    pub const REAR_AMBIENT_BOOST_DEFAULT: f32 = 1.5;
    pub const REAR_AMBIENT_BOOST_MIN: f32 = 1.0;
    pub const REAR_AMBIENT_BOOST_MAX: f32 = 3.0;

    pub const REAR_LATE_REFLECTION_DEFAULT: f32 = 0.10;
    pub const REAR_LATE_REFLECTION_MIN: f32 = 0.0;
    pub const REAR_LATE_REFLECTION_MAX: f32 = 0.5;

    // Sub-harmonic synthesis parameters
    pub const ENABLE_SUBHARMONIC_SYNTH_DEFAULT: bool = false;

    pub const SUBHARMONIC_GAIN_DEFAULT: f32 = 0.5;
    pub const SUBHARMONIC_GAIN_MIN: f32 = 0.0;
    pub const SUBHARMONIC_GAIN_MAX: f32 = 1.0;

    pub const SUBHARMONIC_FREQ_HZ_DEFAULT: f32 = 40.0;
    pub const SUBHARMONIC_FREQ_HZ_MIN: f32 = 20.0;
    pub const SUBHARMONIC_FREQ_HZ_MAX: f32 = 80.0;

    pub const SUBHARMONIC_ATTACK_MS_DEFAULT: f32 = 10.0;
    pub const SUBHARMONIC_ATTACK_MS_MIN: f32 = 1.0;
    pub const SUBHARMONIC_ATTACK_MS_MAX: f32 = 100.0;

    pub const SUBHARMONIC_RELEASE_MS_DEFAULT: f32 = 50.0;
    pub const SUBHARMONIC_RELEASE_MS_MIN: f32 = 10.0;
    pub const SUBHARMONIC_RELEASE_MS_MAX: f32 = 500.0;

    // Decorrelation parameters
    pub const DECORRELATION_MODE_DEFAULT: i32 = 0;
    pub const DECORRELATION_MODE_MIN: i32 = 0;
    pub const DECORRELATION_MODE_MAX: i32 = 1;

    pub const DECORRELATION_LFO_RATE_HZ_DEFAULT: f32 = 0.15;
    pub const DECORRELATION_LFO_RATE_HZ_MIN: f32 = 0.01;
    pub const DECORRELATION_LFO_RATE_HZ_MAX: f32 = 1.0;

    pub const VELVET_NOISE_DURATION_MS_DEFAULT: f32 = 30.0;
    pub const VELVET_NOISE_DURATION_MS_MIN: f32 = 10.0;
    pub const VELVET_NOISE_DURATION_MS_MAX: f32 = 100.0;

    pub const VELVET_NOISE_DENSITY_DEFAULT: f32 = 2000.0;
    pub const VELVET_NOISE_DENSITY_MIN: f32 = 500.0;
    pub const VELVET_NOISE_DENSITY_MAX: f32 = 5000.0;

    pub const SAFETY_CAP_DB_DEFAULT: f32 = 3.0;
    pub const SAFETY_CAP_DB_MIN: f32 = 0.0;
    pub const SAFETY_CAP_DB_MAX: f32 = 3.0;

    // Height channel parameters
    pub const ENABLE_HR_DIRECT_DEFAULT: bool = true;

    pub const HR_SHARPEN_DEFAULT: f32 = 1.0;
    pub const HR_SHARPEN_MIN: f32 = 0.0;
    pub const HR_SHARPEN_MAX: f32 = 1.0;

    pub const HEIGHT_HF_CAP_HZ_DEFAULT: f32 = 16000.0;
    pub const HEIGHT_HF_CAP_HZ_MIN: f32 = 8000.0;
    pub const HEIGHT_HF_CAP_HZ_MAX: f32 = 20000.0;

    pub const HEIGHT_TRANSIENT_REDUCTION_DEFAULT: f32 = 0.6;
    pub const HEIGHT_TRANSIENT_REDUCTION_MIN: f32 = 0.0;
    pub const HEIGHT_TRANSIENT_REDUCTION_MAX: f32 = 1.0;

    pub const HEIGHT_DIRECT_LEAK_DEFAULT: f32 = 0.15;
    pub const HEIGHT_DIRECT_LEAK_MIN: f32 = 0.0;
    pub const HEIGHT_DIRECT_LEAK_MAX: f32 = 0.5;

    // Ambient gain boost (sqrt(1-coherence) multiplier)
    pub const AMBIENT_BOOST_DEFAULT: f32 = 1.2;
    pub const AMBIENT_BOOST_MIN: f32 = 0.5;
    pub const AMBIENT_BOOST_MAX: f32 = 2.0;

    // Dialogue detection parameters
    pub const DIALOGUE_WEIGHT_DEFAULT: f32 = 0.4;
    pub const DIALOGUE_WEIGHT_MIN: f32 = 0.0;
    pub const DIALOGUE_WEIGHT_MAX: f32 = 1.0;

    pub const VOICE_FREQ_MIN_HZ_DEFAULT: f32 = 500.0;
    pub const VOICE_FREQ_MIN_HZ_MIN: f32 = 200.0;
    pub const VOICE_FREQ_MIN_HZ_MAX: f32 = 800.0;

    pub const VOICE_FREQ_MAX_HZ_DEFAULT: f32 = 3000.0;
    pub const VOICE_FREQ_MAX_HZ_MIN: f32 = 2000.0;
    pub const VOICE_FREQ_MAX_HZ_MAX: f32 = 5000.0;
}

// ============================================================================
// Convolution Plugin
// ============================================================================

pub mod convolution {
    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const GAIN_DB_DEFAULT: f32 = 0.0;
    pub const GAIN_DB_MIN: f32 = -20.0;
    pub const GAIN_DB_MAX: f32 = 20.0;
}

// ============================================================================
// Channel Mute/Solo Plugin
// ============================================================================

pub mod channel_mute_solo {
    pub const ENABLED_DEFAULT: bool = true;
}

// ============================================================================
// HAL Input/Output Plugins
// ============================================================================

pub mod hal {
    pub const CHANNELS_DEFAULT: i32 = 2;
    pub const CHANNELS_MIN: i32 = 1;
    pub const CHANNELS_MAX: i32 = 16;
}

// ============================================================================
// Binaural Plugin
// ============================================================================

pub mod binaural {
    pub const ENABLE_OPTIMIZATION_DEFAULT: bool = true;

    pub const EXTERNALIZATION_DEFAULT: f32 = 0.0;
    pub const EXTERNALIZATION_MIN: f32 = 0.0;
    pub const EXTERNALIZATION_MAX: f32 = 1.0;

    pub const NEAR_FIELD_STRENGTH_DEFAULT: f32 = 0.0;
    pub const NEAR_FIELD_STRENGTH_MIN: f32 = 0.0;
    pub const NEAR_FIELD_STRENGTH_MAX: f32 = 1.0;

    pub const DIFFUSE_FIELD_EQ_DEFAULT: bool = true;
}

// ============================================================================
// Spectrum Analyzer
// ============================================================================

pub mod spectrum {
    pub const NUM_BINS_DEFAULT: usize = 30;
    pub const MIN_FREQ_DEFAULT: f32 = 20.0;
    pub const MAX_FREQ_DEFAULT: f32 = 20000.0;
    pub const SMOOTHING_DEFAULT: f32 = 0.7;
    pub const SMOOTHING_MIN: f32 = 0.0;
    pub const SMOOTHING_MAX: f32 = 1.0;
}

// ============================================================================
// EQ Plugin
// ============================================================================

pub mod eq {
    // EQ filters are dynamic, but we can define common ranges for filter parameters
    pub const FREQUENCY_MIN: f64 = 20.0;
    pub const FREQUENCY_MAX: f64 = 20000.0;

    pub const Q_MIN: f64 = 0.1;
    pub const Q_MAX: f64 = 10.0;
    pub const Q_DEFAULT: f64 = 1.0;

    pub const GAIN_DB_MIN: f64 = -24.0;
    pub const GAIN_DB_MAX: f64 = 24.0;
    pub const GAIN_DB_DEFAULT: f64 = 0.0;
}

// ============================================================================
// Multiband Compressor Plugin
// ============================================================================

pub mod multiband_compressor {
    // Number of bands
    pub const NUM_BANDS_DEFAULT: usize = 3;
    pub const NUM_BANDS_MIN: usize = 2;
    pub const NUM_BANDS_MAX: usize = 5;

    // Crossover preset: 0=Custom, 1=200/2k, 2=100/3k, 3=250/4k
    pub const CROSSOVER_PRESET_DEFAULT: i32 = 1;
    pub const CROSSOVER_PRESET_MIN: i32 = 0;
    pub const CROSSOVER_PRESET_MAX: i32 = 3;

    // Crossover frequencies (Hz)
    pub const CROSSOVER_FREQ_1_DEFAULT: f32 = 200.0;
    pub const CROSSOVER_FREQ_1_MIN: f32 = 20.0;
    pub const CROSSOVER_FREQ_1_MAX: f32 = 500.0;

    pub const CROSSOVER_FREQ_2_DEFAULT: f32 = 2000.0;
    pub const CROSSOVER_FREQ_2_MIN: f32 = 500.0;
    pub const CROSSOVER_FREQ_2_MAX: f32 = 5000.0;

    pub const CROSSOVER_FREQ_3_DEFAULT: f32 = 8000.0;
    pub const CROSSOVER_FREQ_3_MIN: f32 = 5000.0;
    pub const CROSSOVER_FREQ_3_MAX: f32 = 15000.0;

    pub const CROSSOVER_FREQ_4_DEFAULT: f32 = 12000.0;
    pub const CROSSOVER_FREQ_4_MIN: f32 = 10000.0;
    pub const CROSSOVER_FREQ_4_MAX: f32 = 18000.0;

    // Global compression parameters (same as compressor)
    pub const THRESHOLD_DEFAULT: f32 = -20.0;
    pub const THRESHOLD_MIN: f32 = -60.0;
    pub const THRESHOLD_MAX: f32 = 0.0;

    pub const RATIO_DEFAULT: f32 = 4.0;
    pub const RATIO_MIN: f32 = 1.0;
    pub const RATIO_MAX: f32 = 20.0;

    pub const ATTACK_DEFAULT: f32 = 5.0;
    pub const ATTACK_MIN: f32 = 0.1;
    pub const ATTACK_MAX: f32 = 100.0;

    pub const RELEASE_DEFAULT: f32 = 50.0;
    pub const RELEASE_MIN: f32 = 10.0;
    pub const RELEASE_MAX: f32 = 1000.0;

    pub const KNEE_DEFAULT: f32 = 6.0;
    pub const KNEE_MIN: f32 = 0.0;
    pub const KNEE_MAX: f32 = 20.0;

    pub const MAKEUP_GAIN_DEFAULT: f32 = 0.0;
    pub const MAKEUP_GAIN_MIN: f32 = -24.0;
    pub const MAKEUP_GAIN_MAX: f32 = 24.0;

    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const LINK_CHANNELS_DEFAULT: bool = true;

    // Per-band flags
    pub const BAND_SOLO_DEFAULT: bool = false;
    pub const BAND_BYPASS_DEFAULT: bool = false;
}

// ============================================================================
// Polyphonic Note Detection (PND) & Varispeed Plugin
// ============================================================================

pub mod pnd {
    pub const CORRECTION_STRENGTH_DEFAULT: f32 = 1.0;
    pub const CORRECTION_STRENGTH_MIN: f32 = 0.0;
    pub const CORRECTION_STRENGTH_MAX: f32 = 2.0; // Allow over-correction for effect

    pub const ANALYSIS_WINDOW_MS_DEFAULT: f32 = 100.0;
    pub const ANALYSIS_WINDOW_MS_MIN: f32 = 20.0;
    pub const ANALYSIS_WINDOW_MS_MAX: f32 = 500.0;

    pub const DRIFT_SMOOTHING_DEFAULT: f32 = 0.1; // Smoothing factor
    pub const DRIFT_SMOOTHING_MIN: f32 = 0.001;
    pub const DRIFT_SMOOTHING_MAX: f32 = 1.0;
}

// ============================================================================
// Denoiser Plugin
// ============================================================================

pub mod denoiser {
    pub const REDUCTION_DB_DEFAULT: f32 = 12.0;
    pub const REDUCTION_DB_MIN: f32 = 0.0;
    pub const REDUCTION_DB_MAX: f32 = 40.0;

    pub const FLOOR_DB_DEFAULT: f32 = -30.0;
    pub const FLOOR_DB_MIN: f32 = -60.0;
    pub const FLOOR_DB_MAX: f32 = -10.0;

    pub const SMOOTHING_DEFAULT: f32 = 0.8;
    pub const SMOOTHING_MIN: f32 = 0.0;
    pub const SMOOTHING_MAX: f32 = 0.99;

    pub const ATTACK_MS_DEFAULT: f32 = 5.0;
    pub const ATTACK_MS_MIN: f32 = 0.1;
    pub const ATTACK_MS_MAX: f32 = 100.0;

    pub const RELEASE_MS_DEFAULT: f32 = 50.0;
    pub const RELEASE_MS_MIN: f32 = 10.0;
    pub const RELEASE_MS_MAX: f32 = 500.0;

    pub const LOW_LATENCY_DEFAULT: bool = false;

    // MCRA-specific parameters (advanced/expert use)
    pub const MCRA_ALPHA_S_DEFAULT: f32 = 0.9; // Noise PSD smoothing
    pub const MCRA_ALPHA_P_DEFAULT: f32 = 0.2; // Speech presence probability smoothing
    pub const MCRA_L_DEFAULT: usize = 50; // Minimum tracking window (frames)
    pub const MCRA_DELTA_DEFAULT: f32 = 5.0; // Speech presence threshold

    pub const POLYPHONIC_DETECTION_DEFAULT: bool = false;
}

// ============================================================================
// Multiband Expander Plugin
// ============================================================================

pub mod multiband_expander {
    // Number of bands (same as multiband compressor)
    pub const NUM_BANDS_DEFAULT: usize = 3;
    pub const NUM_BANDS_MIN: usize = 2;
    pub const NUM_BANDS_MAX: usize = 5;

    // Crossover preset: 0=Custom, 1=200/2k, 2=100/3k, 3=250/4k
    pub const CROSSOVER_PRESET_DEFAULT: i32 = 1;
    pub const CROSSOVER_PRESET_MIN: i32 = 0;
    pub const CROSSOVER_PRESET_MAX: i32 = 3;

    // Crossover frequencies (Hz) - same as multiband compressor
    pub const CROSSOVER_FREQ_1_DEFAULT: f32 = 200.0;
    pub const CROSSOVER_FREQ_1_MIN: f32 = 20.0;
    pub const CROSSOVER_FREQ_1_MAX: f32 = 500.0;

    pub const CROSSOVER_FREQ_2_DEFAULT: f32 = 2000.0;
    pub const CROSSOVER_FREQ_2_MIN: f32 = 500.0;
    pub const CROSSOVER_FREQ_2_MAX: f32 = 5000.0;

    pub const CROSSOVER_FREQ_3_DEFAULT: f32 = 8000.0;
    pub const CROSSOVER_FREQ_3_MIN: f32 = 5000.0;
    pub const CROSSOVER_FREQ_3_MAX: f32 = 15000.0;

    pub const CROSSOVER_FREQ_4_DEFAULT: f32 = 12000.0;
    pub const CROSSOVER_FREQ_4_MIN: f32 = 10000.0;
    pub const CROSSOVER_FREQ_4_MAX: f32 = 18000.0;

    // Global expansion parameters (same as expander)
    pub const THRESHOLD_DEFAULT: f32 = -40.0;
    pub const THRESHOLD_MIN: f32 = -80.0;
    pub const THRESHOLD_MAX: f32 = 0.0;

    pub const RATIO_DEFAULT: f32 = 2.0;
    pub const RATIO_MIN: f32 = 1.0;
    pub const RATIO_MAX: f32 = 20.0;

    pub const ATTACK_DEFAULT: f32 = 1.0;
    pub const ATTACK_MIN: f32 = 0.1;
    pub const ATTACK_MAX: f32 = 50.0;

    pub const RELEASE_DEFAULT: f32 = 100.0;
    pub const RELEASE_MIN: f32 = 10.0;
    pub const RELEASE_MAX: f32 = 2000.0;

    pub const RANGE_DEFAULT: f32 = 40.0;
    pub const RANGE_MIN: f32 = 0.0;
    pub const RANGE_MAX: f32 = 80.0;

    pub const KNEE_DEFAULT: f32 = 6.0;
    pub const KNEE_MIN: f32 = 0.0;
    pub const KNEE_MAX: f32 = 20.0;

    pub const HYSTERESIS_DEFAULT: f32 = 4.0;
    pub const HYSTERESIS_MIN: f32 = 0.0;
    pub const HYSTERESIS_MAX: f32 = 12.0;

    pub const HOLD_DEFAULT: f32 = 10.0;
    pub const HOLD_MIN: f32 = 0.0;
    pub const HOLD_MAX: f32 = 500.0;

    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const LINK_CHANNELS_DEFAULT: bool = true;

    // Per-band flags
    pub const BAND_SOLO_DEFAULT: bool = false;
    pub const BAND_BYPASS_DEFAULT: bool = false;
}
