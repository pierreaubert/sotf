//! Room reflection compensation for the XTC plugin.
//!
//! Adds early reflection awareness via two modes:
//! 1. **Image source model** — analytical first-order reflections from a rectangular room
//! 2. **Measured room IR** — loads a WAV impulse response file
//!
//! Reflections are integrated before the matrix inversion step in filter computation.
//! Each reflection adds a delayed, attenuated, head-shadowed contribution to the
//! ipsilateral and contralateral transfer functions. The XTC inverse then naturally
//! compensates for reflections.

use super::config::XtcPluginParams;
use super::filters::{SPEED_OF_SOUND, head_shadowing_woodworth};
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

/// Room height fixed at 2.5m (floor/ceiling reflections are less critical)
const ROOM_HEIGHT_M: f32 = 2.5;

/// Listener ear height (seated position)
const LISTENER_HEIGHT_M: f32 = 1.2;

/// A single reflection path from an image source to an ear
pub(crate) struct ReflectionPath {
    /// Propagation delay from image source to ear (seconds)
    pub delay_s: f32,
    /// Reflection amplitude: sqrt(1-absorption) * (direct_dist / image_dist).
    /// Uses the pressure reflection coefficient sqrt(1-α) rather than the Sabine
    /// energy coefficient (1-α).
    pub amplitude: f32,
    /// Angle at head center for head_shadowing_woodworth()
    pub shadow_angle: f32,
}

/// Pre-computed per-bin room reflection data for integration into XTC filters
pub(crate) struct RoomReflectionData {
    /// Per-bin complex transfer function for Speaker L -> Ear L
    pub h_ll_ipsi: Vec<Complex<f32>>,
    /// Per-bin complex transfer function for Speaker R -> Ear L
    pub h_lr_contra: Vec<Complex<f32>>,
    /// Per-bin complex transfer function for Speaker L -> Ear R
    pub h_rl_contra: Vec<Complex<f32>>,
    /// Per-bin complex transfer function for Speaker R -> Ear R
    pub h_rr_ipsi: Vec<Complex<f32>>,
    /// Per-bin multiplicative beta boost factor (1.0 = no boost)
    pub beta_boost: Vec<f32>,
}

/// Coordinate system: origin at room center floor.
/// X = left/right, Y = up, Z = front/back (listening axis).
pub(crate) struct RoomGeometry {
    width: f32,
    depth: f32,
    height: f32,
    wall_absorption: f32,
}

/// Compute image source positions for first-order reflections.
///
/// Six image sources per speaker: one for each wall surface (left, right, front, back, floor, ceiling).
/// Each image is the speaker position reflected across the respective surface.
pub(crate) fn compute_image_sources(
    speaker_pos: [f32; 3],
    ear_pos: [f32; 3],
    direct_dist: f32,
    room: &RoomGeometry,
) -> Vec<ReflectionPath> {
    let half_w = room.width / 2.0;
    let half_d = room.depth / 2.0;

    // Six image source positions: mirror speaker across each surface
    let images = [
        // Left wall (x = -half_w): reflect x across x=-half_w
        [
            -half_w - (speaker_pos[0] - (-half_w)),
            speaker_pos[1],
            speaker_pos[2],
        ],
        // Right wall (x = +half_w): reflect x across x=+half_w
        [
            half_w + (half_w - speaker_pos[0]),
            speaker_pos[1],
            speaker_pos[2],
        ],
        // Front wall (z = +half_d): reflect z across z=+half_d
        [
            speaker_pos[0],
            speaker_pos[1],
            half_d + (half_d - speaker_pos[2]),
        ],
        // Back wall (z = -half_d): reflect z across z=-half_d
        [
            speaker_pos[0],
            speaker_pos[1],
            -half_d - (speaker_pos[2] - (-half_d)),
        ],
        // Floor (y = 0): reflect y across y=0
        [speaker_pos[0], -speaker_pos[1], speaker_pos[2]],
        // Ceiling (y = height): reflect y across y=height
        [
            speaker_pos[0],
            2.0 * room.height - speaker_pos[1],
            speaker_pos[2],
        ],
    ];

    let mut paths = Vec::with_capacity(6);

    for image_pos in &images {
        let dx = image_pos[0] - ear_pos[0];
        let dy = image_pos[1] - ear_pos[1];
        let dz = image_pos[2] - ear_pos[2];
        let image_dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if image_dist < 1e-6 {
            continue;
        }

        // wall_absorption is a Sabine energy coefficient (0 = reflective, 1 = absorptive).
        // Pressure reflection coefficient = sqrt(1 - α), not (1 - α).
        let amplitude = (1.0 - room.wall_absorption).sqrt() * (direct_dist / image_dist);
        let delay_s = image_dist / SPEED_OF_SOUND;

        // Shadow angle: azimuth from head center to image source (in horizontal plane)
        let azimuth = (dx).atan2(dz).abs();
        let shadow_angle = (PI / 2.0 + azimuth).min(PI);

        paths.push(ReflectionPath {
            delay_s,
            amplitude,
            shadow_angle,
        });
    }

    paths
}

/// Build room reflection data using the image source model.
///
/// Computes reflection paths for all six surfaces, then sums their frequency-domain
/// contributions per bin.
pub(crate) fn build_reflection_data_image_source(
    params: &XtcPluginParams,
    sample_rate: u32,
    num_bins: usize,
) -> RoomReflectionData {
    let room = RoomGeometry {
        width: params.room_width_m,
        depth: params.room_depth_m,
        height: ROOM_HEIGHT_M,
        wall_absorption: params.wall_absorption,
    };

    // Listener position: center of room, at ear height, offset by head tracking
    let listener_pos = [
        params.head_offset_x,
        LISTENER_HEIGHT_M,
        params.head_offset_z,
    ];

    let head_radius = params.head_radius_m;

    // Speaker positions (symmetric at speaker_angle_deg, distance_m away)
    let theta_rad = params.speaker_angle_deg * PI / 180.0;
    let d = params.distance_m;
    let speaker_height = LISTENER_HEIGHT_M; // speakers at ear level

    let left_speaker = [-d * theta_rad.sin(), speaker_height, d * theta_rad.cos()];
    let right_speaker = [d * theta_rad.sin(), speaker_height, d * theta_rad.cos()];

    // Ear positions
    let left_ear = [
        listener_pos[0] - head_radius,
        listener_pos[1],
        listener_pos[2],
    ];
    let right_ear = [
        listener_pos[0] + head_radius,
        listener_pos[1],
        listener_pos[2],
    ];

    // Direct distances for attenuation normalization
    let direct_left_to_left_ear = euclidean_dist(&left_speaker, &left_ear);
    let direct_right_to_right_ear = euclidean_dist(&right_speaker, &right_ear);
    let direct_left_to_right_ear = euclidean_dist(&left_speaker, &right_ear);
    let direct_right_to_left_ear = euclidean_dist(&right_speaker, &left_ear);

    // Compute reflection paths:
    // Ipsi paths: left speaker → left ear, right speaker → right ear
    // Contra paths: right speaker → left ear, left speaker → right ear
    let ipsi_reflections_l =
        compute_image_sources(left_speaker, left_ear, direct_left_to_left_ear, &room);
    let ipsi_reflections_r =
        compute_image_sources(right_speaker, right_ear, direct_right_to_right_ear, &room);
    let contra_reflections_l =
        compute_image_sources(right_speaker, left_ear, direct_right_to_left_ear, &room);
    let contra_reflections_r =
        compute_image_sources(left_speaker, right_ear, direct_left_to_right_ear, &room);

    let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

    let mut h_ll_ipsi = vec![Complex::new(0.0, 0.0); num_bins];
    let mut h_lr_contra = vec![Complex::new(0.0, 0.0); num_bins];
    let mut h_rl_contra = vec![Complex::new(0.0, 0.0); num_bins];
    let mut h_rr_ipsi = vec![Complex::new(0.0, 0.0); num_bins];

    // Accumulate per-bin contributions from all reflection paths
    for bin in 0..num_bins {
        let freq = bin as f32 * freq_per_bin;

        h_ll_ipsi[bin] = sum_reflection_paths(&ipsi_reflections_l, freq, head_radius);
        h_lr_contra[bin] = sum_reflection_paths(&contra_reflections_l, freq, head_radius);
        h_rl_contra[bin] = sum_reflection_paths(&contra_reflections_r, freq, head_radius);
        h_rr_ipsi[bin] = sum_reflection_paths(&ipsi_reflections_r, freq, head_radius);
    }

    // Compute magnitude of total transfer function (direct + reflections) for beta boost
    let mut h_total_magnitude = vec![0.0_f32; num_bins];
    for bin in 0..num_bins {
        // Approximate total magnitude across all paths
        let total = (Complex::new(1.0, 0.0) + h_ll_ipsi[bin]).norm()
            + h_lr_contra[bin].norm()
            + h_rl_contra[bin].norm()
            + (Complex::new(1.0, 0.0) + h_rr_ipsi[bin]).norm();
        h_total_magnitude[bin] = total / 2.0; // Average per ear
    }

    let beta_boost =
        compute_reflection_beta_boost(&h_total_magnitude, num_bins, params.reflection_beta_boost);

    RoomReflectionData {
        h_ll_ipsi,
        h_lr_contra,
        h_rl_contra,
        h_rr_ipsi,
        beta_boost,
    }
}

/// Sum frequency-domain contributions from a set of reflection paths at a given frequency.
///
/// Each path's contribution includes head shadowing and air absorption attenuation.
fn sum_reflection_paths(paths: &[ReflectionPath], freq: f32, head_radius: f32) -> Complex<f32> {
    let mut sum = Complex::new(0.0, 0.0);
    for path in paths {
        let shadow = head_shadowing_woodworth(freq, path.shadow_angle, head_radius);
        let distance = path.delay_s * SPEED_OF_SOUND;
        let air_atten = air_absorption(freq, distance);
        let gain = path.amplitude * shadow * air_atten;
        let phase = -2.0 * PI * freq * path.delay_s;
        let contribution = Complex::new(gain * phase.cos(), gain * phase.sin());
        sum += contribution;
    }
    sum
}

/// Build room reflection data from a measured impulse response WAV file.
///
/// Mono IR: same data for both ipsi and contra paths.
/// Stereo IR: ch0 = ipsi, ch1 = contra.
///
/// `fft_forward` is an optional pre-planned FFT for `(num_bins - 1) * 2` samples.
/// When provided, it is reused instead of creating a fresh planner (Optimization 4).
pub(crate) fn build_reflection_data_ir(
    ir_path: &str,
    sample_rate: u32,
    num_bins: usize,
    fft_forward: Option<Arc<dyn RealToComplex<f32>>>,
) -> Result<RoomReflectionData, String> {
    use symphonia::core::audio::{Audio, GenericAudioBufferRef};
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
    use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
    use symphonia::core::io::MediaSourceStream;

    // Load WAV file
    let file = std::fs::File::open(ir_path).map_err(|e| format!("IO: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut reader = symphonia_format_riff::WavReader::try_new(mss, FormatOptions::default())
        .map_err(|e| format!("WAV probe: {}", e))?;

    let track = reader
        .default_track(TrackType::Audio)
        .ok_or("No track in WAV file")?;
    let codec_params = match track.codec_params.clone() {
        Some(CodecParameters::Audio(params)) => params,
        _ => return Err("WAV file does not contain an audio track".into()),
    };

    // Validate sample rate
    let ir_sample_rate = codec_params.sample_rate.unwrap_or(0);
    if ir_sample_rate != sample_rate {
        return Err(format!(
            "IR sample rate {} does not match engine sample rate {}. Resampling not supported.",
            ir_sample_rate, sample_rate
        ));
    }

    let num_channels = codec_params
        .channels
        .as_ref()
        .map(|c| c.count())
        .unwrap_or(1);

    let mut decoder =
        symphonia_codec_pcm::PcmDecoder::try_new(&codec_params, &AudioDecoderOptions::default())
            .map_err(|e| format!("Decoder: {}", e))?;

    let mut channels: Vec<Vec<f32>> = vec![Vec::new(); num_channels];
    while let Ok(Some(packet)) = reader.next_packet() {
        let decoded = decoder
            .decode(&packet)
            .map_err(|e| format!("Decode: {}", e))?;
        match &decoded {
            GenericAudioBufferRef::F32(buf) => {
                for (ch, channel) in channels.iter_mut().enumerate() {
                    channel.extend_from_slice(buf.plane(ch).ok_or("Missing IR channel")?);
                }
            }
            _ => return Err("Only F32 WAV format supported for room IR".into()),
        }
    }

    if channels.is_empty() || channels[0].is_empty() {
        return Err("Empty IR file".into());
    }

    let fft_size = (num_bins - 1) * 2;

    // Reuse caller-supplied FFT plan when the size matches; otherwise plan a new one.
    // This avoids allocating a planner for every IR load (Optimization 4).
    let fft_plan: Arc<dyn RealToComplex<f32>> =
        if let Some(plan) = fft_forward.filter(|p| p.len() == fft_size) {
            plan
        } else {
            let mut planner = RealFftPlanner::new();
            planner.plan_fft_forward(fft_size)
        };

    // Process each channel: truncate, window, and FFT.
    // The closure borrows fft_plan by reference so it can be called twice.
    let process_channel = |samples: &[f32]| -> Vec<Complex<f32>> {
        let len = samples.len().min(fft_size);
        let mut padded = vec![0.0_f32; fft_size];
        padded[..len].copy_from_slice(&samples[..len]);

        // Apply half-Hann fade-out on last 10%
        let fade_len = (len as f32 * 0.1) as usize;
        if fade_len > 0 {
            let fade_start = len - fade_len;
            for i in 0..fade_len {
                let t = i as f32 / fade_len as f32;
                let window = 0.5 * (1.0 + (PI * t).cos()); // half-Hann: 1→0
                padded[fade_start + i] *= window;
            }
        }

        let mut spectrum = vec![Complex::new(0.0, 0.0); num_bins];
        fft_plan
            .process(&mut padded, &mut spectrum)
            .expect("FFT processing failed");
        spectrum
    };

    let h_ll_ipsi = process_channel(&channels[0]);
    let h_rr_ipsi = h_ll_ipsi.clone();
    let (h_lr_contra, h_rl_contra) = if num_channels >= 2 {
        let h_lr = process_channel(&channels[1]);
        (h_lr.clone(), h_lr)
    } else {
        // Mono IR: derive contra from ipsi with a simple head-shadowing model.
        // Reflected sound reaching the contralateral ear travels an extra path
        // around the head, causing delay (ITD) and high-frequency attenuation.
        let itd_s = 0.0003_f32; // ~0.3 ms typical ITD for 30° speakers
        let shadow_cutoff_hz = 2000.0_f32;
        let freq_per_bin = sample_rate as f32 / (2.0 * (num_bins - 1) as f32);

        let mut h_lr = vec![Complex::new(0.0, 0.0); num_bins];
        for bin in 0..num_bins {
            let freq = bin as f32 * freq_per_bin;
            // Simple 1st-order lowpass shadow model
            let shadow = if freq <= 0.0 {
                1.0
            } else {
                let ratio = freq / shadow_cutoff_hz;
                1.0 / (1.0 + ratio)
            };
            let phase = -2.0 * PI * freq * itd_s;
            let delay_phasor = Complex::new(phase.cos(), phase.sin());
            h_lr[bin] = h_ll_ipsi[bin] * shadow * delay_phasor;
        }
        (h_lr.clone(), h_lr)
    };

    // Compute beta boost from combined magnitude
    let mut h_total_magnitude = vec![0.0_f32; num_bins];
    for bin in 0..num_bins {
        h_total_magnitude[bin] = h_ll_ipsi[bin].norm() + h_lr_contra[bin].norm();
    }

    // Use a default boost factor of 3.0 for IR mode
    let beta_boost = compute_reflection_beta_boost(&h_total_magnitude, num_bins, 3.0);

    Ok(RoomReflectionData {
        h_ll_ipsi,
        h_lr_contra,
        h_rl_contra,
        h_rr_ipsi,
        beta_boost,
    })
}

/// Detect comb filter nulls and compute per-bin beta boost factors.
///
/// Smooths the magnitude envelope, then boosts beta at bins where magnitude
/// drops significantly below the smoothed envelope (comb filter nulls).
pub(crate) fn compute_reflection_beta_boost(
    h_total_magnitude: &[f32],
    num_bins: usize,
    boost_factor: f32,
) -> Vec<f32> {
    if num_bins < 3 {
        return vec![1.0; num_bins];
    }

    // Step 1: ~1/6 octave smoothing via moving average with frequency-proportional window
    let mut smoothed = vec![0.0_f32; num_bins];
    smoothed[0] = h_total_magnitude[0];
    for (bin, smoothed_val) in smoothed.iter_mut().enumerate().skip(1) {
        // Window width: ~1/6 octave in bins
        // 1/6 octave at bin b spans b * (2^(1/12) - 1) bins on each side
        let half_width = ((bin as f32 * 0.06) as usize).max(1).min(num_bins / 4);
        let start = bin.saturating_sub(half_width);
        let end = (bin + half_width + 1).min(num_bins);
        let count = (end - start) as f32;
        let sum: f32 = h_total_magnitude[start..end].iter().sum();
        *smoothed_val = sum / count;
    }

    // Step 2: Compute raw boost where magnitude is >10 dB below smoothed envelope
    let threshold_db = 10.0;
    let threshold_ratio = 10.0_f32.powf(-threshold_db / 20.0); // ~0.316

    let mut raw_boost = vec![1.0_f32; num_bins];
    for bin in 1..num_bins - 1 {
        if smoothed[bin] > 1e-10 {
            let ratio = h_total_magnitude[bin] / smoothed[bin];
            if ratio < threshold_ratio {
                // Null depth in dB (positive value)
                let null_depth_db = -20.0 * ratio.max(1e-6).log10();
                // Proportional boost, capped at boost_factor × base
                let boost =
                    (1.0 + (null_depth_db / threshold_db) * (boost_factor - 1.0)).min(boost_factor);
                raw_boost[bin] = boost;
            }
        }
    }

    // Step 3: 3-bin smoothing to avoid sharp transitions
    let mut final_boost = vec![1.0_f32; num_bins];
    for bin in 1..num_bins - 1 {
        final_boost[bin] = (raw_boost[bin - 1] + raw_boost[bin] + raw_boost[bin + 1]) / 3.0;
    }
    final_boost[0] = raw_boost[0];
    if num_bins > 1 {
        final_boost[num_bins - 1] = raw_boost[num_bins - 1];
    }

    final_boost
}

/// Frequency-dependent air absorption per ISO 9613-1 (approximation at 20°C, 50% RH).
///
/// Returns a linear attenuation factor (0..1). Only significant for distances >2m
/// and frequencies above a few kHz.
///
/// Formula: α ≈ 0.001 · (f/1000)²  dB/m.
/// This approximates ISO 9613-1 within factor ~2 across 500 Hz–8 kHz for typical
/// indoor conditions (20°C, 50% RH). Overestimates by ~1.8× at 4 kHz and ~2.5× at 8 kHz
/// relative to the full ISO 9613-1 table, but the errors are inaudible for room-scale
/// distances (e.g., at 10 m, 8 kHz: 0.64 dB predicted vs ~0.25 dB actual).
#[inline]
pub(crate) fn air_absorption(freq: f32, distance_m: f32) -> f32 {
    let alpha = 0.001 * (freq / 1000.0).powi(2); // dB/m approximation
    10.0_f32.powf(-alpha * distance_m / 20.0)
}

/// Euclidean distance between two 3D points
#[inline]
fn euclidean_dist(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Test support: expose RoomGeometry construction for unit tests
#[cfg(test)]
pub(crate) mod tests_support {
    use super::RoomGeometry;

    pub fn make_room(width: f32, depth: f32, height: f32, wall_absorption: f32) -> RoomGeometry {
        RoomGeometry {
            width,
            depth,
            height,
            wall_absorption,
        }
    }
}
