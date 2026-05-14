//! DSP utilities for audio signal processing
//!
//! This crate provides:
//! - **Signal generation**: Test signals (tones, sweeps, noise)
//! - **Signal analysis**: FFT-based frequency analysis, microphone compensation
//! - **Acoustic metrics**: RT60, clarity (C50/C80), THD, spectrogram
//!
//! # Example
//!
//! ```rust
//! use math_audio_dsp::{signals, analysis};
//!
//! // Generate a 1 kHz tone
//! let signal = signals::gen_tone(1000.0, 0.5, 48000, 1.0);
//!
//! // Analyze a WAV file
//! let config = analysis::WavAnalysisConfig::default();
//! // let result = analysis::analyze_wav_buffer(&signal, 48000, &config);
//! ```

pub mod analysis;
pub mod audio_features;
pub mod binaural_loudness;
pub mod binaural_matrix;
pub mod ebur128;
pub mod esprit;
pub mod fast_math;
pub mod fdn;
pub mod fdw;
pub mod instantaneous_frequency;
pub mod psychoacoustics;
pub mod replaygain;
pub mod response;
pub mod rtpghi;
pub mod signals;
pub mod simd;
pub mod stft;
pub mod tonal_transient;
pub mod waveform;

// DSP building blocks (moved from sotf-host)
pub mod adaa;
pub mod auto_makeup;
pub mod channel_linking;
pub mod dc_blocker;
pub mod delta_monitor;
pub mod detector;
pub mod dynamics_core;
pub mod envelope;
pub mod envelope_follower;
pub mod lookahead;
pub mod smoothing;
pub mod true_peak;

// Re-export commonly used types
pub use analysis::{
    AnalysisResult, CrossCorrelationEnvelopeResult, MicrophoneCompensation, WavAnalysisConfig,
    WavAnalysisOutput, WindowedFrequencyResponse, analyze_recording, analyze_wav_buffer,
    analyze_wav_file, compute_average_response, compute_clarity_broadband,
    compute_clarity_spectrum, compute_group_delay, compute_impulse_response_from_fr,
    compute_rt60_broadband, compute_rt60_spectrum, compute_spectrogram, compute_windowed_fr,
    cross_correlate_envelope, find_db_point, read_analysis_csv, smooth_response_f32,
    smooth_response_f64, write_analysis_csv, write_wav_analysis_csv,
};

pub use fdw::{FdwAnalysis, FdwConfig, analyze_impulse_response_fdw};

pub use binaural_loudness::{
    BinauralChannel, BinauralDownmix, BinauralLoudness, BinauralLoudnessResult, SurroundLayout,
    measure_binaural, measure_binaural_from_surround,
};

pub use binaural_matrix::{
    MatrixInverseBin, TransferMatrixBin, align_ir_to_reference_peak, condition_number,
    deconvolve_sweep_to_ir, direct_peak_sample, direct_peak_windowed_half_spectrum,
    direct_windowed_half_spectrum, fdw_complex_half_spectrum, half_spectrum_to_fir,
    position_errors, solve_minimax_regularized_inverse_bin, solve_regularized_inverse_bin,
    solve_weighted_regularized_inverse_bin, suppress_log_sweep_harmonic_residues,
};

pub use signals::{
    add_silence_padding, apply_fade_in, apply_fade_out, clip, frames_for, gen_allpass_probe,
    gen_dirac, gen_log_sweep, gen_m_noise, gen_mls, gen_narrowband_probe, gen_pink_noise, gen_tone,
    gen_two_tone, gen_white_noise, interleave_per_channel, mono_to_stereo,
    prepare_signal_for_playback, prepare_signal_for_playback_channels, replicate_mono,
};

pub use replaygain::{ReplayGainAnalyzer, ReplayGainInfo, ReplayGainTrackData, compute_album_gain};
pub use response::{biquad_complex_response, fir_complex_response, lr4_crossover_response};
pub use waveform::{WAVEFORM_SAMPLES, compute_waveform};
