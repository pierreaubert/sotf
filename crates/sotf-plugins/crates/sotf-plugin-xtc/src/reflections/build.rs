use super::super::config::XtcPluginParams;
use super::compute::compute_image_sources;
use super::compute::compute_reflection_beta_boost;
use super::misc::LISTENER_HEIGHT_M;
use super::misc::ROOM_HEIGHT_M;
use super::misc::euclidean_dist;
use super::types::RoomGeometry;
use super::types::RoomReflectionData;
use super::types::sum_reflection_paths;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use std::f32::consts::PI;
use std::sync::Arc;

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
