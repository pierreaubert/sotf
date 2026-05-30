//! # math-rir: Room Impulse Response Analysis
//!
//! Two complementary analysis paths on a Room Impulse Response (RIR):
//!
//! 1. **SSIR segmentation** ([`analyze_rir`], [`analyze_srir`]) — Spatial
//!    Segmentation of the early RIR into consecutive sound events
//!    (direct sound + reflections), based on Pawlak & Lee, *Spatial
//!    segmentation of impulse response for room reflection analysis and
//!    auralization*, Applied Acoustics 249 (2026).
//! 2. **ISO 3382 room-acoustic metrics** ([`analyze_iso3382`],
//!    [`analyze_iso3382_octaves`], [`analyze_iso3382_third_octaves`]) —
//!    EDT, T20, T30, C50, C80, D50, Centre time (Ts) computed from a
//!    Schroeder backward integration, with optional per-octave or
//!    per-third-octave filtering using zero-phase Butterworth bandpasses.
//!
//! ## Overview
//!
//! The SSIR method segments a Room Impulse Response (RIR) into consecutive,
//! variable-length sound events (direct sound + early reflections), each with
//! a constant direction of arrival (DOA). This preserves the full temporal
//! energy profile while enabling per-reflection manipulation.
//!
//! The ISO 3382 path treats the whole RIR as one signal and reports the
//! classical reverberation/clarity parameters that listening rooms and
//! performance spaces are measured against.
//!
//! ## Usage
//!
//! ```rust
//! use math_rir::{analyze_rir, analyze_iso3382, analyze_iso3382_octaves, SsirConfig};
//!
//! let rir: Vec<f32> = load_impulse_response(); // your RIR data
//! let sr = 48000.0;
//!
//! // 1) SSIR segmentation — per-reflection geometry.
//! let result = analyze_rir(&rir, &SsirConfig::new(sr));
//! println!("Detected {} events ({} reflections)",
//!     result.num_events(), result.num_reflections());
//!
//! // 2) ISO 3382 broadband metrics.
//! let m = analyze_iso3382(&rir, sr);
//! println!("T30 = {:.2}s, EDT = {:.2}s, C80 = {:.1} dB, Ts = {:.0} ms",
//!     m.t30_s, m.edt_s, m.c80_db, m.ts_s * 1000.0);
//!
//! // 3) Per-octave-band ISO 3382 metrics (125 Hz … 8 kHz).
//! for (fc, m) in analyze_iso3382_octaves(&rir, sr) {
//!     println!("  {:>5.0} Hz: T30={:.2}s C50={:.1}dB", fc, m.t30_s, m.c50_db);
//! }
//! # fn load_impulse_response() -> Vec<f32> { vec![0.0; 4800] }
//! ```

pub mod bands;
mod config;
mod detection;
pub mod metrics;
mod mixing_time;
mod segmentation;
mod types;

pub use bands::{
    BandWidth, ISO_OCTAVE_CENTERS_HZ, ISO_THIRD_OCTAVE_CENTERS_HZ, analyze_iso3382_bands,
    analyze_iso3382_octaves, analyze_iso3382_third_octaves, bandpass,
};
pub use config::SsirConfig;
pub use math_audio_iir_fir::filtfilt;
pub use metrics::{
    DecayCurve, Iso3382Metrics, analyze_iso3382, estimate_noise_cutoff, schroeder_curve,
};
pub use types::{RirSegment, SsirResult};

use detection::{detect_reflections, find_direct_sound_toa};
use mixing_time::estimate_mixing_time;
use rayon::prelude::*;
use segmentation::build_segments;

/// Analyze a mono room impulse response using the SSIR method.
///
/// Detects the direct sound, identifies early reflections via Local Energy Ratio,
/// and segments the early RIR into consecutive sound events.
///
/// For mono input, DOA validation is not available — only energy-based and
/// temporal distance criteria are used for reflection detection.
///
/// Returns an [`SsirResult`] with the detected segments and mixing time.
pub fn analyze_rir(rir: &[f32], config: &SsirConfig) -> SsirResult {
    if rir.is_empty() {
        return SsirResult {
            segments: Vec::new(),
            mixing_time_samples: 0,
            sample_rate: config.sample_rate,
        };
    }

    // Step 1: Estimate mixing time (or use configured value)
    let mixing_time_samples = if config.mixing_time_ms.is_some() {
        config.mixing_time_samples()
    } else {
        estimate_mixing_time(rir, config.sample_rate)
    };

    // Step 2: Find direct sound TOA
    let direct_sound_toa = match find_direct_sound_toa(rir, config) {
        Some(toa) => toa,
        None => {
            // No direct sound detected — return empty result
            return SsirResult {
                segments: Vec::new(),
                mixing_time_samples,
                sample_rate: config.sample_rate,
            };
        }
    };

    // Step 3: Detect early reflections (no DOA data for mono)
    let reflections = detect_reflections(rir, direct_sound_toa, None, config);

    // Step 4: Build segments with onset refinement
    let segments = build_segments(
        rir,
        direct_sound_toa,
        None,
        &reflections,
        mixing_time_samples,
        config,
    );

    SsirResult {
        segments,
        mixing_time_samples,
        sample_rate: config.sample_rate,
    }
}

/// Analyze a multi-channel Spatial Room Impulse Response (SRIR) using the full SSIR method.
///
/// Uses the first channel as the omnidirectional pressure signal for energy-based
/// detection, and derives DOA from all channels using the intensity vector method.
///
/// `channels` should contain at least 4 channels (B-format: W, X, Y, Z) for
/// meaningful DOA estimation. The first channel (W) is used as the omnidirectional
/// signal for reflection detection.
///
/// Falls back to mono analysis if fewer than 4 channels are provided.
pub fn analyze_srir(channels: &[&[f32]], config: &SsirConfig) -> SsirResult {
    if channels.is_empty() || channels[0].is_empty() {
        return SsirResult {
            segments: Vec::new(),
            mixing_time_samples: 0,
            sample_rate: config.sample_rate,
        };
    }

    // Use first channel as omnidirectional pressure
    let omni = channels[0];

    // Need at least W, X, Y, Z (4 channels) for DOA estimation
    if channels.len() < 4 {
        return analyze_rir(omni, config);
    }

    // Verify all channels have the same length
    let len = omni.len();
    if channels.iter().any(|ch| ch.len() != len) {
        return analyze_rir(omni, config);
    }

    // Step 1: Estimate mixing time
    let mixing_time_samples = if config.mixing_time_ms.is_some() {
        config.mixing_time_samples()
    } else {
        estimate_mixing_time(omni, config.sample_rate)
    };

    // Step 2: Find direct sound TOA
    let direct_sound_toa = match find_direct_sound_toa(omni, config) {
        Some(toa) => toa,
        None => {
            return SsirResult {
                segments: Vec::new(),
                mixing_time_samples,
                sample_rate: config.sample_rate,
            };
        }
    };

    // Step 3: Compute DOA vectors from band-limited B-format channels
    // B-format: W (omni), X (front-back), Y (left-right), Z (up-down)
    let doa_vectors = compute_bformat_doa(channels, len, config);

    // Step 4: Detect reflections with DOA validation
    let reflections = detect_reflections(omni, direct_sound_toa, Some(&doa_vectors), config);

    // Step 5: Build segments (pass direct sound DOA from the DOA vector at its TOA)
    let ds_doa = doa_vectors.get(direct_sound_toa).copied();
    let segments = build_segments(
        omni,
        direct_sound_toa,
        ds_doa,
        &reflections,
        mixing_time_samples,
        config,
    );

    SsirResult {
        segments,
        mixing_time_samples,
        sample_rate: config.sample_rate,
    }
}

/// Compute per-sample DOA unit vectors from B-format (Ambisonics) channels.
///
/// The channels are band-limited with a zero-phase Butterworth bandpass filter
/// before computing the pseudo-intensity vector. This improves DOA reliability
/// by excluding low frequencies (poor spatial resolution) and high frequencies
/// (spatial aliasing).
///
/// **DOA sign convention.** Uses the pseudo-intensity vector
/// `I = P · V`, with `P = W` (omnidirectional pressure) and
/// `V = [X, Y, Z]` (figure-of-eight channels). For first-order Ambisonics
/// B-format the V channels are pickup patterns oriented along the
/// coordinate axes — *not* raw particle-velocity components — so a source
/// at `+X` produces W and X signals in phase and `I_x = W · X` is positive
/// for a source at `+X`. The DOA (source direction) is therefore
/// `+I / |I|`, consistent with the SSIR paper and standard first-order
/// Ambisonics DOA literature (Pulkki 2007, Merimaa 2002).
///
/// The tests `test_compute_bformat_doa_plane_wave_*` verify the sign
/// against known plane-wave fixtures.
///
/// **Allocations.** This used to allocate up to 8 large heap vectors per
/// call (4 × `Vec<f64>` for the f64 input copy + 4 × `Vec<f32>` for the
/// filtered output). The no-filter branch additionally cloned all 4 input
/// channels. We now:
///   - skip the input clone in the no-filter branch (borrow the caller's
///     slices directly),
///   - keep the filtered branch limited to 2 vectors per channel (one f64
///     scratch input + one f64 filtfilt output that is then materialised
///     into the owned f32 vector — we cannot eliminate that pair without
///     changing the `filtfilt` API to operate in-place).
fn compute_bformat_doa(channels: &[&[f32]], len: usize, config: &SsirConfig) -> Vec<[f32; 3]> {
    let (low_hz, high_hz) = config.doa_bandpass_hz;
    let order = config.doa_bandpass_order;
    let nyquist = config.sample_rate / 2.0;

    // Band-limit all 4 B-format channels with zero-phase filtering.
    // Skip filtering if the band covers the full spectrum or the signal is too short.
    let needs_filtering = low_hz > 0.0 && high_hz < nyquist && len >= 4 && order >= 1;

    // Filtered branch owns four f32 vectors; un-filtered branch borrows
    // the input slices and allocates nothing extra.
    let owned: Option<[Vec<f32>; 4]> = if needs_filtering {
        let mut sections =
            filtfilt::peq_to_coefficients(&math_audio_iir_fir::peq_butterworth_highpass(
                order as usize,
                low_hz,
                config.sample_rate,
            ));
        sections.extend(filtfilt::peq_to_coefficients(
            &math_audio_iir_fir::peq_butterworth_lowpass(
                order as usize,
                high_hz,
                config.sample_rate,
            ),
        ));
        let filter_channel = |ch: &[f32]| -> Vec<f32> {
            // Down from 8 vectors per call to 2 (input scratch + output).
            let mut scratch: Vec<f64> = Vec::with_capacity(ch.len());
            scratch.extend(ch.iter().map(|&s| s as f64));
            let out_f64 = filtfilt::filtfilt(&scratch, &sections);
            let mut out_f32: Vec<f32> = Vec::with_capacity(out_f64.len());
            out_f32.extend(out_f64.into_iter().map(|s| s as f32));
            out_f32
        };
        let ((w, x), (y, z)) = rayon::join(
            || {
                rayon::join(
                    || filter_channel(channels[0]),
                    || filter_channel(channels[1]),
                )
            },
            || {
                rayon::join(
                    || filter_channel(channels[2]),
                    || filter_channel(channels[3]),
                )
            },
        );
        Some([w, x, y, z])
    } else {
        None
    };

    let (w, x, y, z): (&[f32], &[f32], &[f32], &[f32]) = if let Some(o) = owned.as_ref() {
        (&o[0], &o[1], &o[2], &o[3])
    } else {
        (channels[0], channels[1], channels[2], channels[3])
    };

    (0..len)
        .into_par_iter()
        .map(|i| {
            let p = w[i] as f64;
            // Pseudo-intensity vector components I = P · V. For B-format
            // first-order Ambisonics this points TOWARD the source (the V
            // channels are figure-of-eight pickup patterns, not raw
            // particle-velocity components).
            let ix = p * x[i] as f64;
            let iy = p * y[i] as f64;
            let iz = p * z[i] as f64;

            let mag = (ix * ix + iy * iy + iz * iz).sqrt();
            if mag < 1e-12 {
                [0.0f32, 0.0, 0.0]
            } else {
                // DOA = +I / |I| (source direction in B-format convention).
                let inv = 1.0 / mag;
                [(ix * inv) as f32, (iy * inv) as f32, (iz * inv) as f32]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a synthetic RIR with known reflections
    fn make_synthetic_rir(
        sample_rate: f64,
        reflection_times_ms: &[f64],
        reflection_gains: &[f32],
    ) -> Vec<f32> {
        let duration_ms = 100.0;
        let len = (duration_ms * sample_rate / 1000.0) as usize;
        let mut rir = vec![0.0001f32; len]; // low noise floor

        // Direct sound at 1ms
        let ds_sample = (1.0 * sample_rate / 1000.0) as usize;
        rir[ds_sample] = 1.0;

        // Add reflections
        for (&time_ms, &gain) in reflection_times_ms.iter().zip(reflection_gains.iter()) {
            let sample = (time_ms * sample_rate / 1000.0) as usize;
            if sample < len {
                rir[sample] = gain;
            }
        }

        rir
    }

    #[test]
    fn test_analyze_rir_basic() {
        let rir = make_synthetic_rir(48000.0, &[6.0, 10.0, 15.0, 22.0], &[0.5, 0.3, 0.25, 0.15]);

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let result = analyze_rir(&rir, &config);

        // Should detect direct sound + reflections
        assert!(
            result.num_events() >= 3,
            "expected >= 3 events, got {}",
            result.num_events()
        );
        assert!(result.segments[0].is_direct_sound);

        // Segments should be consecutive
        for i in 0..result.segments.len() - 1 {
            assert_eq!(
                result.segments[i].end_sample,
                result.segments[i + 1].onset_sample,
                "segments {} and {} are not consecutive",
                i,
                i + 1
            );
        }

        // All reflection TOAs should be within the early RIR
        for seg in result.reflections() {
            let toa_ms = seg.toa_ms(48000.0);
            assert!(
                toa_ms > 1.0 && toa_ms < 40.0,
                "reflection TOA {toa_ms:.1}ms outside expected range"
            );
        }
    }

    #[test]
    fn test_analyze_rir_empty() {
        let config = SsirConfig::new(48000.0);
        let result = analyze_rir(&[], &config);
        assert_eq!(result.num_events(), 0);
    }

    #[test]
    fn test_analyze_rir_single_impulse() {
        // Anechoic: only direct sound, no reflections
        let mut rir = vec![0.0001f32; 4800]; // 100ms
        rir[48] = 1.0;

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let result = analyze_rir(&rir, &config);

        // Should have at least the direct sound
        assert!(result.num_events() >= 1);
        assert!(result.segments[0].is_direct_sound);
    }

    #[test]
    fn test_analyze_srir_fallback_to_mono() {
        let rir = make_synthetic_rir(48000.0, &[6.0, 10.0], &[0.5, 0.3]);

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        // Only 2 channels — should fall back to mono
        let result = analyze_srir(&[&rir, &rir], &config);
        assert!(result.num_events() >= 2);
    }

    #[test]
    fn test_compute_bformat_doa_plane_wave_front() {
        // Plane wave from +X (front): W and X in phase, Y = Z = 0.
        // DOA should point along +X.
        let len = 1024;
        let mut w = vec![0.0f32; len];
        let mut x = vec![0.0f32; len];
        let y = vec![0.0f32; len];
        let z = vec![0.0f32; len];
        for i in 100..120 {
            let s = (-(i as f32 - 110.0).powi(2) / 4.0).exp();
            w[i] = s;
            x[i] = s;
        }
        // Bandpass disabled so we test the raw intensity computation.
        let config = SsirConfig {
            sample_rate: 48000.0,
            doa_bandpass_hz: (0.0, 96000.0),
            doa_bandpass_order: 0,
            ..SsirConfig::default()
        };
        let doa = compute_bformat_doa(&[&w, &x, &y, &z], len, &config);
        let d = doa[110];
        assert!(d[0] > 0.99, "expected DOA[x] ≈ +1, got {:?}", d);
        assert!(d[1].abs() < 0.05, "expected DOA[y] ≈ 0, got {:?}", d);
        assert!(d[2].abs() < 0.05, "expected DOA[z] ≈ 0, got {:?}", d);
    }

    #[test]
    fn test_compute_bformat_doa_plane_wave_left() {
        // Plane wave from +Y (left): W and Y in phase.
        let len = 1024;
        let mut w = vec![0.0f32; len];
        let x = vec![0.0f32; len];
        let mut y = vec![0.0f32; len];
        let z = vec![0.0f32; len];
        for i in 100..120 {
            let s = (-(i as f32 - 110.0).powi(2) / 4.0).exp();
            w[i] = s;
            y[i] = s;
        }
        let config = SsirConfig {
            sample_rate: 48000.0,
            doa_bandpass_hz: (0.0, 96000.0),
            doa_bandpass_order: 0,
            ..SsirConfig::default()
        };
        let doa = compute_bformat_doa(&[&w, &x, &y, &z], len, &config);
        let d = doa[110];
        assert!(d[0].abs() < 0.05, "expected DOA[x] ≈ 0, got {:?}", d);
        assert!(d[1] > 0.99, "expected DOA[y] ≈ +1, got {:?}", d);
        assert!(d[2].abs() < 0.05, "expected DOA[z] ≈ 0, got {:?}", d);
    }

    #[test]
    fn test_analyze_srir_bformat() {
        let len = 4800;
        let mut w = vec![0.0001f32; len]; // omni
        let mut x = vec![0.0f32; len]; // front-back
        let mut y = vec![0.0f32; len]; // left-right
        let z = vec![0.0f32; len]; // up-down

        // Direct sound from front (positive X)
        w[48] = 1.0;
        x[48] = 1.0;
        y[48] = 0.0;

        // Reflection from left at 6ms (positive Y)
        w[288] = 0.5;
        x[288] = 0.0;
        y[288] = 0.5;

        // Reflection from right at 10ms (negative Y)
        w[480] = 0.3;
        x[480] = 0.0;
        y[480] = -0.3;

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let result = analyze_srir(&[&w, &x, &y, &z], &config);

        assert!(
            result.num_events() >= 2,
            "expected >= 2 events, got {}",
            result.num_events()
        );

        // Check that DOA is present on segments
        for seg in &result.segments {
            assert!(seg.doa.is_some(), "SRIR segments should have DOA data");
        }

        let ds_doa = result
            .direct_sound_doa()
            .expect("direct sound should carry DOA");
        assert!(
            ds_doa[0] > 0.5,
            "front direct sound should point toward +X, got {:?}",
            ds_doa
        );
    }

    #[test]
    fn test_segments_cover_early_rir() {
        let rir = make_synthetic_rir(48000.0, &[6.0, 12.0, 20.0], &[0.5, 0.3, 0.2]);

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };

        let result = analyze_rir(&rir, &config);

        // First segment should start at 0
        assert_eq!(result.segments[0].onset_sample, 0);

        // Segments should be non-empty
        for seg in &result.segments {
            assert!(!seg.is_empty(), "segment should have non-zero length");
        }
    }

    #[test]
    fn test_mixing_time_auto_estimation() {
        // Create a RIR with sparse reflections then dense reverb
        let sample_rate = 48000.0;
        let len = (0.200 * sample_rate) as usize;
        let mut rir = vec![0.0f32; len];

        // Direct sound
        rir[48] = 1.0;
        // Sparse reflections
        rir[240] = 0.5;
        rir[480] = 0.3;

        // Dense reverb starting at ~30ms
        let reverb_start = (0.030 * sample_rate) as usize;
        let mut amp = 0.08f32;
        let mut rng: u32 = 12345;
        for sample in rir.iter_mut().take(len).skip(reverb_start) {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let noise = ((rng >> 16) as f32 / 32768.0) - 1.0;
            *sample += noise * amp;
            amp *= 0.9997;
        }

        let config = SsirConfig {
            sample_rate,
            mixing_time_ms: None, // auto-estimate
            ..SsirConfig::default()
        };

        let result = analyze_rir(&rir, &config);

        // Mixing time should be in reasonable range
        let mt_ms = result.mixing_time_ms();
        assert!(
            (10.0..=80.0).contains(&mt_ms),
            "auto mixing time {mt_ms:.1}ms outside expected range"
        );
    }

    #[test]
    fn test_analyze_rir_very_short() {
        // RIR shorter than one LER window (48 samples at 48kHz = 1ms)
        let rir = vec![0.5f32; 10];
        let config = SsirConfig::new(48000.0);
        let result = analyze_rir(&rir, &config);
        // Should not panic, may find 0 or 1 events
        assert!(result.num_events() <= 1);
    }

    #[test]
    fn test_analyze_rir_all_zeros() {
        let rir = vec![0.0f32; 4800];
        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };
        let result = analyze_rir(&rir, &config);
        // All-zero RIR: no detectable direct sound
        assert_eq!(result.num_events(), 0);
    }

    #[test]
    fn test_analyze_rir_dc_offset() {
        // RIR with DC offset — should still detect the impulse
        let mut rir = vec![0.1f32; 4800];
        rir[48] = 1.0;
        rir[288] = 0.6;

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };
        let result = analyze_rir(&rir, &config);
        assert!(result.num_events() >= 1);
    }

    #[test]
    fn test_segment_duration_ms_accuracy() {
        let seg = RirSegment {
            onset_sample: 0,
            end_sample: 480,
            toa_sample: 48,
            doa: None,
            peak_energy: 1.0,
            is_direct_sound: true,
        };
        let dur = seg.duration_ms(48000.0);
        assert!((dur - 10.0).abs() < 0.01, "expected 10ms, got {dur}ms");
    }

    #[test]
    fn test_direct_sound_toa_at_rir_boundary() {
        // Direct sound at the very start
        let mut rir = vec![0.0001f32; 2400];
        rir[0] = 1.0;
        rir[288] = 0.3;

        let config = SsirConfig {
            sample_rate: 48000.0,
            mixing_time_ms: Some(40.0),
            ..SsirConfig::default()
        };
        let result = analyze_rir(&rir, &config);
        assert!(result.num_events() >= 1);
        assert!(result.segments[0].is_direct_sound);
        assert_eq!(result.segments[0].toa_sample, 0);
    }
}
