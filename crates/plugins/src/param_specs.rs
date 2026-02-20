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

    // Dialogue detection sub-weights (centroid, variance, coherence)
    pub const DIALOGUE_CENTROID_WEIGHT_DEFAULT: f32 = 0.3;
    pub const DIALOGUE_CENTROID_WEIGHT_MIN: f32 = 0.0;
    pub const DIALOGUE_CENTROID_WEIGHT_MAX: f32 = 1.0;

    pub const DIALOGUE_VARIANCE_WEIGHT_DEFAULT: f32 = 0.2;
    pub const DIALOGUE_VARIANCE_WEIGHT_MIN: f32 = 0.0;
    pub const DIALOGUE_VARIANCE_WEIGHT_MAX: f32 = 1.0;

    pub const DIALOGUE_COHERENCE_WEIGHT_DEFAULT: f32 = 0.5;
    pub const DIALOGUE_COHERENCE_WEIGHT_MIN: f32 = 0.0;
    pub const DIALOGUE_COHERENCE_WEIGHT_MAX: f32 = 1.0;
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
    pub const NUM_BINS_MIN: usize = 8;
    pub const NUM_BINS_MAX: usize = 120;
    pub const MIN_FREQ_DEFAULT: f32 = 20.0;
    pub const MIN_FREQ_MIN: f32 = 10.0;
    pub const MIN_FREQ_MAX: f32 = 100.0;
    pub const MAX_FREQ_DEFAULT: f32 = 20000.0;
    pub const MAX_FREQ_MIN: f32 = 5000.0;
    pub const MAX_FREQ_MAX: f32 = 22050.0;
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
    pub const FREQUENCY_DEFAULT: f64 = 1000.0;

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

    pub const FLOOR_DB_DEFAULT: f32 = -20.0;
    pub const FLOOR_DB_MIN: f32 = -60.0;
    pub const FLOOR_DB_MAX: f32 = -10.0;

    pub const SMOOTHING_DEFAULT: f32 = 0.3;
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
    pub const MCRA_ALPHA_P_DEFAULT: f32 = 0.7; // Speech presence probability smoothing
    pub const MCRA_L_DEFAULT: usize = 50; // Minimum tracking window (frames)
    pub const MCRA_DELTA_DEFAULT: f32 = 5.0; // Speech presence threshold

    pub const POLYPHONIC_DETECTION_DEFAULT: bool = false;

    // Psychoacoustic masking
    pub const PSYCHOACOUSTIC_MASKING_DEFAULT: bool = true;

    // Noise profile capture
    pub const USE_CAPTURED_PROFILE_DEFAULT: bool = false;
    pub const LEARN_FRAMES: usize = 50; // ~1s at typical hop rates

    // Transparency: blend computed gain toward 1.0 (0 = full denoising, 1 = pass-through)
    pub const TRANSPARENCY_DEFAULT: f32 = 0.0;
    pub const TRANSPARENCY_MIN: f32 = 0.0;
    pub const TRANSPARENCY_MAX: f32 = 1.0;

    // Decision-Directed SNR estimation
    pub const DD_ENABLED_DEFAULT: bool = false;
    pub const DD_ALPHA_DEFAULT: f32 = 0.98;
    pub const DD_ALPHA_MIN: f32 = 0.5;
    pub const DD_ALPHA_MAX: f32 = 0.999;
}

// ============================================================================
// Multiband Expander Plugin
// ============================================================================

// ============================================================================
// Fletcher-Munson Loudness Compensation Plugin
// ============================================================================

pub mod fletcher_munson {
    // Playback volume (set by engine/UI when master volume changes)
    pub const PLAYBACK_VOLUME_DB_DEFAULT: f32 = 0.0;
    pub const PLAYBACK_VOLUME_DB_MIN: f32 = -80.0;
    pub const PLAYBACK_VOLUME_DB_MAX: f32 = 0.0;

    // Reference level where response is flat (corresponds to ~80 dB SPL)
    pub const REFERENCE_LEVEL_DB_DEFAULT: f32 = -14.0;
    pub const REFERENCE_LEVEL_DB_MIN: f32 = -40.0;
    pub const REFERENCE_LEVEL_DB_MAX: f32 = 0.0;

    // Smoothing time for gain transitions (ms)
    pub const SMOOTHING_MS_DEFAULT: f32 = 30.0;
    pub const SMOOTHING_MS_MIN: f32 = 1.0;
    pub const SMOOTHING_MS_MAX: f32 = 200.0;

    // Band frequency ranges
    pub const BAND_FREQ_MIN: f64 = 20.0;
    pub const BAND_FREQ_MAX: f64 = 20000.0;

    // Band Q ranges
    pub const BAND_Q_MIN: f64 = 0.1;
    pub const BAND_Q_MAX: f64 = 10.0;

    // Band max gain ranges
    pub const BAND_MAX_GAIN_MIN: f64 = 0.0;
    pub const BAND_MAX_GAIN_MAX: f64 = 24.0;

    // Band slope ranges (dB gain per dB volume delta)
    pub const BAND_SLOPE_MIN: f64 = 0.0;
    pub const BAND_SLOPE_MAX: f64 = 1.0;

    // Band 1: Sub-bass (~60 Hz) - ISO 226 shows largest compensation needed here
    pub const BAND1_FREQ_DEFAULT: f64 = 60.0;
    pub const BAND1_Q_DEFAULT: f64 = 0.5;
    pub const BAND1_MAX_GAIN_DEFAULT: f64 = 15.0;
    pub const BAND1_SLOPE_DEFAULT: f64 = 0.6;

    // Band 2: Mid-bass (~250 Hz) - moderate compensation
    pub const BAND2_FREQ_DEFAULT: f64 = 250.0;
    pub const BAND2_Q_DEFAULT: f64 = 0.707;
    pub const BAND2_MAX_GAIN_DEFAULT: f64 = 8.0;
    pub const BAND2_SLOPE_DEFAULT: f64 = 0.4;

    // Band 3: Presence (~3.5 kHz) - small boost (ear most sensitive here)
    pub const BAND3_FREQ_DEFAULT: f64 = 3500.0;
    pub const BAND3_Q_DEFAULT: f64 = 1.0;
    pub const BAND3_MAX_GAIN_DEFAULT: f64 = 4.0;
    pub const BAND3_SLOPE_DEFAULT: f64 = 0.2;

    // Band 4: Air/brilliance (~12 kHz) - treble compensation
    pub const BAND4_FREQ_DEFAULT: f64 = 12000.0;
    pub const BAND4_Q_DEFAULT: f64 = 0.707;
    pub const BAND4_MAX_GAIN_DEFAULT: f64 = 6.0;
    pub const BAND4_SLOPE_DEFAULT: f64 = 0.3;

    // Enabled default
    pub const ENABLED_DEFAULT: bool = true;

    // Auto-gain parameters
    pub const AUTO_GAIN_ENABLED_DEFAULT: bool = false;
    pub const AUTO_GAIN_MAX_DB_DEFAULT: f32 = 12.0;
    pub const AUTO_GAIN_MAX_DB_MIN: f32 = 0.0;
    pub const AUTO_GAIN_MAX_DB_MAX: f32 = 24.0;
    pub const AUTO_GAIN_SMOOTHING_MS_DEFAULT: f32 = 100.0;
    pub const AUTO_GAIN_SMOOTHING_MS_MIN: f32 = 10.0;
    pub const AUTO_GAIN_SMOOTHING_MS_MAX: f32 = 500.0;
    // 0 = Momentary (400ms), 1 = ShortTerm (3s)
    pub const AUTO_GAIN_LOUDNESS_TYPE_DEFAULT: i32 = 0;
}

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

// ============================================================================
// Band Split Plugin
// ============================================================================

// ============================================================================
// Mono to Stereo Plugin
// ============================================================================

pub mod mono_to_stereo {
    pub const STEREO_WIDTH_DEFAULT: f32 = 0.5;
    pub const STEREO_WIDTH_MIN: f32 = 0.0;
    pub const STEREO_WIDTH_MAX: f32 = 1.0;

    pub const HAAS_DELAY_MS_DEFAULT: f32 = 1.5;
    pub const HAAS_DELAY_MS_MIN: f32 = 0.0;
    pub const HAAS_DELAY_MS_MAX: f32 = 5.0;

    pub const ENABLE_COMP_EQ_DEFAULT: bool = true;

    pub const COMP_EQ_DEPTH_DB_DEFAULT: f32 = 1.0;
    pub const COMP_EQ_DEPTH_DB_MIN: f32 = 0.0;
    pub const COMP_EQ_DEPTH_DB_MAX: f32 = 3.0;

    pub const DECOR_LOW_HZ_DEFAULT: f32 = 300.0;
    pub const DECOR_LOW_HZ_MIN: f32 = 100.0;
    pub const DECOR_LOW_HZ_MAX: f32 = 500.0;

    pub const DECOR_HIGH_HZ_DEFAULT: f32 = 2000.0;
    pub const DECOR_HIGH_HZ_MIN: f32 = 1000.0;
    pub const DECOR_HIGH_HZ_MAX: f32 = 5000.0;
}

// ============================================================================
// Downmix Plugin
// ============================================================================

pub mod downmix {
    pub const CENTER_GAIN_DB_DEFAULT: f32 = -3.0;
    pub const CENTER_GAIN_DB_MIN: f32 = -12.0;
    pub const CENTER_GAIN_DB_MAX: f32 = 0.0;

    pub const SURROUND_GAIN_DB_DEFAULT: f32 = -3.0;
    pub const SURROUND_GAIN_DB_MIN: f32 = -12.0;
    pub const SURROUND_GAIN_DB_MAX: f32 = 0.0;

    pub const HEIGHT_GAIN_DB_DEFAULT: f32 = -6.0;
    pub const HEIGHT_GAIN_DB_MIN: f32 = -60.0;
    pub const HEIGHT_GAIN_DB_MAX: f32 = 0.0;

    pub const LFE_GAIN_DB_DEFAULT: f32 = -10.0;
    pub const LFE_GAIN_DB_MIN: f32 = -60.0;
    pub const LFE_GAIN_DB_MAX: f32 = 0.0;

    pub const PHASE_COHERENCE_DEFAULT: bool = true;

    pub const PHASE_BLEND_LOW_HZ_DEFAULT: f32 = 500.0;
    pub const PHASE_BLEND_LOW_HZ_MIN: f32 = 100.0;
    pub const PHASE_BLEND_LOW_HZ_MAX: f32 = 1000.0;

    pub const PHASE_BLEND_HIGH_HZ_DEFAULT: f32 = 2000.0;
    pub const PHASE_BLEND_HIGH_HZ_MIN: f32 = 1000.0;
    pub const PHASE_BLEND_HIGH_HZ_MAX: f32 = 5000.0;
}

// ============================================================================
// Band Split Plugin
// ============================================================================

pub mod band_split {
    /// Crossover frequency in Hz
    pub const FREQUENCY_DEFAULT: f64 = 300.0;
    pub const FREQUENCY_MIN: f64 = 20.0;
    pub const FREQUENCY_MAX: f64 = 20000.0;

    /// Crossover type: "LR24" (24 dB/oct) or "LR48" (48 dB/oct)
    pub const CROSSOVER_TYPE_DEFAULT: &str = "LR24";
}

// ============================================================================
// Band Merge Plugin
// ============================================================================

pub mod band_merge {
    /// Number of bands to merge
    pub const BANDS_DEFAULT: usize = 2;
    pub const BANDS_MIN: usize = 2;
    pub const BANDS_MAX: usize = 8;
}

// ============================================================================
// XTC (Crosstalk Cancellation) Plugin
// ============================================================================

pub mod xtc {
    pub const DISTANCE_M_DEFAULT: f64 = 2.0;
    pub const DISTANCE_M_MIN: f64 = 0.5;
    pub const DISTANCE_M_MAX: f64 = 10.0;

    pub const SPEAKER_ANGLE_DEG_DEFAULT: f64 = 30.0;
    pub const SPEAKER_ANGLE_DEG_MIN: f64 = 10.0;
    pub const SPEAKER_ANGLE_DEG_MAX: f64 = 90.0;

    pub const HEAD_RADIUS_M_DEFAULT: f64 = 0.0875;
    pub const HEAD_RADIUS_M_MIN: f64 = 0.05;
    pub const HEAD_RADIUS_M_MAX: f64 = 0.12;

    pub const BETA_BASE_DEFAULT: f64 = 0.001;
    pub const BETA_BASE_MIN: f64 = 0.0001;
    pub const BETA_BASE_MAX: f64 = 0.1;

    pub const BETA_LOW_FREQ_BOOST_DEFAULT: f64 = 10.0;
    pub const BETA_LOW_FREQ_BOOST_MIN: f64 = 0.0;
    pub const BETA_LOW_FREQ_BOOST_MAX: f64 = 30.0;

    pub const BETA_HIGH_FREQ_BOOST_DEFAULT: f64 = 10.0;
    pub const BETA_HIGH_FREQ_BOOST_MIN: f64 = 0.0;
    pub const BETA_HIGH_FREQ_BOOST_MAX: f64 = 30.0;

    pub const HEAD_SHADOW_CUTOFF_HZ_DEFAULT: f64 = 4000.0;
    pub const HEAD_SHADOW_CUTOFF_HZ_MIN: f64 = 1000.0;
    pub const HEAD_SHADOW_CUTOFF_HZ_MAX: f64 = 10000.0;

    pub const HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_DEFAULT: f64 = 6.0;
    pub const HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MIN: f64 = 0.0;
    pub const HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MAX: f64 = 12.0;

    pub const MAX_GAIN_DB_DEFAULT: f64 = 12.0;
    pub const MAX_GAIN_DB_MIN: f64 = 3.0;
    pub const MAX_GAIN_DB_MAX: f64 = 30.0;

    pub const AUTO_GAIN_ENABLED_DEFAULT: bool = true;
    pub const AUTO_GAIN_MAX_DB_DEFAULT: f32 = 12.0;
    pub const AUTO_GAIN_MAX_DB_MIN: f32 = 0.0;
    pub const AUTO_GAIN_MAX_DB_MAX: f32 = 24.0;
    pub const AUTO_GAIN_SMOOTHING_MS_DEFAULT: f32 = 100.0;
    pub const AUTO_GAIN_SMOOTHING_MS_MIN: f32 = 10.0;
    pub const AUTO_GAIN_SMOOTHING_MS_MAX: f32 = 500.0;
}

// ============================================================================
// AB Compare Plugin
// ============================================================================

pub mod ab_compare {
    pub const MIX_DEFAULT: f64 = 0.0;
    pub const MIX_MIN: f64 = -1.0;
    pub const MIX_MAX: f64 = 1.0;

    pub const MIX_MODE_DEFAULT: i32 = 0;
    pub const MIX_MODE_MIN: i32 = 0;
    pub const MIX_MODE_MAX: i32 = 1;

    pub const SELECTED_PATH_DEFAULT: i32 = 0;
    pub const SELECTED_PATH_MIN: i32 = 0;
    pub const SELECTED_PATH_MAX: i32 = 1;

    pub const MAX_AUTO_GAIN_DB_DEFAULT: f64 = 12.0;
    pub const MAX_AUTO_GAIN_DB_MIN: f64 = 0.0;
    pub const MAX_AUTO_GAIN_DB_MAX: f64 = 24.0;

    pub const GAIN_SMOOTHING_MS_DEFAULT: f64 = 100.0;
    pub const GAIN_SMOOTHING_MS_MIN: f64 = 1.0;
    pub const GAIN_SMOOTHING_MS_MAX: f64 = 500.0;

    pub const MIX_TRANSITION_MS_DEFAULT: f64 = 50.0;
    pub const MIX_TRANSITION_MS_MIN: f64 = 1.0;
    pub const MIX_TRANSITION_MS_MAX: f64 = 500.0;

    pub const LOUDNESS_TYPE_DEFAULT: i32 = 0;
    pub const LOUDNESS_TYPE_MIN: i32 = 0;
    pub const LOUDNESS_TYPE_MAX: i32 = 1;
}

// ============================================================================
// Crossfeed Plugin
// ============================================================================

pub mod crossfeed {
    pub const CROSSFEED_MODE_DEFAULT: i32 = 0; // Bauer
    pub const CROSSFEED_PRESET_DEFAULT: i32 = 0; // Default

    // Bauer mode
    pub const BAUER_FCUT_DEFAULT: f32 = 700.0;
    pub const BAUER_FCUT_MIN: f32 = 400.0;
    pub const BAUER_FCUT_MAX: f32 = 1000.0;

    pub const BAUER_FEED_DEFAULT: f32 = 4.5;
    pub const BAUER_FEED_MIN: f32 = 0.0;
    pub const BAUER_FEED_MAX: f32 = 15.0;

    // Meier mode
    pub const MEIER_LEVEL_DEFAULT: f32 = 30.0;
    pub const MEIER_LEVEL_MIN: f32 = 0.0;
    pub const MEIER_LEVEL_MAX: f32 = 100.0;

    // Multiband mode
    pub const MB_LOW_FREQ_DEFAULT: f32 = 150.0;
    pub const MB_LOW_FREQ_MIN: f32 = 50.0;
    pub const MB_LOW_FREQ_MAX: f32 = 500.0;

    pub const MB_MID_HIGH_FREQ_DEFAULT: f32 = 5700.0;
    pub const MB_MID_HIGH_FREQ_MIN: f32 = 2000.0;
    pub const MB_MID_HIGH_FREQ_MAX: f32 = 15000.0;

    pub const MB_LOW_FEED_DEFAULT: f32 = 0.0;
    pub const MB_LOW_FEED_MIN: f32 = -20.0;
    pub const MB_LOW_FEED_MAX: f32 = 0.0;

    pub const MB_MID_FEED_DEFAULT: f32 = 6.0;
    pub const MB_MID_FEED_MIN: f32 = 0.0;
    pub const MB_MID_FEED_MAX: f32 = 15.0;

    pub const MB_HIGH_FEED_DEFAULT: f32 = 3.0;
    pub const MB_HIGH_FEED_MIN: f32 = 0.0;
    pub const MB_HIGH_FEED_MAX: f32 = 15.0;

    // Auto gain
    pub const AUTOGAIN_ENABLED_DEFAULT: bool = false;
    pub const AUTOGAIN_TARGET_DEFAULT: f32 = -18.0;
    pub const AUTOGAIN_TARGET_MIN: f32 = -40.0;
    pub const AUTOGAIN_TARGET_MAX: f32 = -12.0;

    pub const AUTOGAIN_MAX_GAIN_DEFAULT: f32 = 12.0;
    pub const AUTOGAIN_MAX_GAIN_MIN: f32 = 0.0;
    pub const AUTOGAIN_MAX_GAIN_MAX: f32 = 24.0;

    pub const AUTOGAIN_SMOOTHING_DEFAULT: f32 = 100.0;
    pub const AUTOGAIN_SMOOTHING_MIN: f32 = 10.0;
    pub const AUTOGAIN_SMOOTHING_MAX: f32 = 5000.0;

    // Global
    pub const MIX_DEFAULT: f32 = 1.0;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;
}
