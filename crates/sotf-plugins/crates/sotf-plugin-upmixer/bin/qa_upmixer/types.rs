use super::artifact_event::ArtifactEvent;
use super::artifact_tracker::ArtifactTracker;
use super::diagnostic_max_deltas::DiagnosticMaxDeltas;
use super::misc::safe_filename_fragment;
use sotf_plugin_upmixer::UpmixerPluginParams;
use std::path::PathBuf;

#[derive(Debug)]
pub(super) struct DiagnosticOptions {
    pub(super) input_path: PathBuf,
    pub(super) output_path: PathBuf,
    pub(super) speaker_config: String,
    pub(super) block_size: usize,
    pub(super) fft_size: usize,
    pub(super) frequency_resolution: String,
    pub(super) enable_hr_direct: bool,
    pub(super) bypass_decorrelation: bool,
    pub(super) bypass_transient_detection: bool,
    pub(super) ml_model_path: Option<String>,
}

#[derive(Debug)]
pub(super) struct IsolationOptions {
    pub(super) input_path: PathBuf,
    pub(super) output_dir: PathBuf,
    pub(super) speaker_configs: Vec<String>,
    pub(super) block_size: usize,
    pub(super) fft_size: usize,
    pub(super) seconds: f32,
    pub(super) frequency_resolutions: Vec<String>,
    pub(super) write_wavs: bool,
    pub(super) ml_model_path: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct IsolationVariant {
    pub(super) name: String,
    pub(super) config: String,
    pub(super) frequency_resolution: String,
    pub(super) notes: String,
    pub(super) params: UpmixerPluginParams,
}

#[derive(Debug, Clone)]
pub(super) struct IsolationRunResult {
    pub(super) variant: IsolationVariant,
    pub(super) output_channels: usize,
    pub(super) frames_produced: usize,
    pub(super) block_csv_path: PathBuf,
    pub(super) wav_path: Option<PathBuf>,
    pub(super) artifacts: ArtifactMetrics,
    pub(super) max_deltas: DiagnosticMaxDeltas,
}

pub(super) struct InputAudio {
    pub(super) sample_rate: u32,
    pub(super) samples: Vec<f32>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct InputMetrics {
    pub(super) peak: f32,
    pub(super) rms: f32,
}

pub(super) fn input_metrics(samples: &[f32]) -> InputMetrics {
    if samples.is_empty() {
        return InputMetrics::default();
    }

    let mut peak = 0.0_f32;
    let mut energy = 0.0_f32;
    for &sample in samples {
        peak = peak.max(sample.abs());
        energy += sample * sample;
    }
    InputMetrics {
        peak,
        rms: (energy / samples.len() as f32).sqrt(),
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct ChannelMetrics {
    pub(super) peaks: Vec<f32>,
    pub(super) rms: Vec<f32>,
    pub(super) max_peak: f32,
    pub(super) rms_sum: f32,
    pub(super) step_peak: f32,
}

pub(super) fn channel_metrics(
    samples: &[f32],
    channels: usize,
    frames: usize,
    prev_last: &mut [f32],
) -> ChannelMetrics {
    let mut metrics = ChannelMetrics {
        peaks: vec![0.0; channels],
        rms: vec![0.0; channels],
        ..Default::default()
    };
    if frames == 0 || channels == 0 {
        return metrics;
    }

    for frame in 0..frames {
        for ch in 0..channels {
            let idx = frame * channels + ch;
            let sample = samples[idx];
            let abs = sample.abs();
            metrics.peaks[ch] = metrics.peaks[ch].max(abs);
            metrics.max_peak = metrics.max_peak.max(abs);
            metrics.rms[ch] += sample * sample;

            let prev = if frame == 0 {
                prev_last[ch]
            } else {
                samples[(frame - 1) * channels + ch]
            };
            metrics.step_peak = metrics.step_peak.max((sample - prev).abs());
        }
    }

    for ch in 0..channels {
        metrics.rms[ch] = (metrics.rms[ch] / frames as f32).sqrt();
        metrics.rms_sum += metrics.rms[ch];
        prev_last[ch] = samples[(frames - 1) * channels + ch];
    }

    metrics
}

#[derive(Debug, Clone, Default)]
pub(super) struct ArtifactMetrics {
    pub(super) peak: ArtifactEvent,
    pub(super) max_step: ArtifactEvent,
    pub(super) max_boundary_step: ArtifactEvent,
    pub(super) max_hop_step: ArtifactEvent,
    pub(super) max_second_diff: ArtifactEvent,
    pub(super) max_second_diff_rms: ArtifactEvent,
}

pub(super) fn analyze_input_artifacts(samples: &[f32]) -> ArtifactMetrics {
    let frames = samples.len() / 2;
    let mut tracker = ArtifactTracker::new(2, 64, None);
    tracker.observe_block(samples, frames, 0, 0);
    tracker.finish()
}

pub(super) fn build_isolation_variants(
    opts: &IsolationOptions,
    config: &str,
) -> Vec<IsolationVariant> {
    let mut variants = Vec::new();
    for frequency_resolution in &opts.frequency_resolutions {
        let mut base = UpmixerPluginParams {
            fft_size: opts.fft_size,
            speaker_config: config.to_string(),
            frequency_resolution: frequency_resolution.clone(),
            ..Default::default()
        };
        if let Some(model_path) = opts.ml_model_path.clone() {
            base.enable_ml_detection = true;
            base.ml_model_path = model_path;
        }

        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_full",
            "full high-latency processing",
            base.clone(),
        );

        let mut low_latency = base.clone();
        low_latency.low_latency = true;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "low_full",
            "full low-latency processing",
            low_latency,
        );

        let mut no_hr = base.clone();
        no_hr.enable_hr_direct = false;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_hr",
            "high-latency processing with HR direct path disabled",
            no_hr,
        );

        let mut no_decorrelation = base.clone();
        no_decorrelation.bypass_decorrelation = true;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_decorrelation",
            "high-latency processing with decorrelation bypassed",
            no_decorrelation,
        );

        let mut no_transients = base.clone();
        no_transients.bypass_transient_detection = true;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_transients",
            "high-latency processing with transient-adaptive controls bypassed",
            no_transients,
        );

        let mut no_height = base.clone();
        no_height.height_gain = 0.0;
        no_height.height_direct_leak = 0.0;
        no_height.rear_late_reflection = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_height",
            "high-latency processing with height routing disabled",
            no_height,
        );

        let mut no_ambient = base.clone();
        no_ambient.gain_front_ambient = 0.0;
        no_ambient.gain_rear_ambient = 0.0;
        no_ambient.ambient_boost = 0.5;
        no_ambient.surround_direct_bleed = 0.0;
        no_ambient.rear_ambient_boost = 1.0;
        no_ambient.rear_late_reflection = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_ambient",
            "high-latency processing with ambient/surround routing minimized",
            no_ambient,
        );

        let mut center_off = base.clone();
        center_off.center_spread = 1.0;
        center_off.dialogue_weight = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_center_off",
            "high-latency processing with center extraction spread out",
            center_off,
        );

        let mut fft_front_only = base.clone();
        fft_front_only.gain_front_ambient = 0.0;
        fft_front_only.gain_rear_ambient = 0.0;
        fft_front_only.height_gain = 0.0;
        fft_front_only.height_direct_leak = 0.0;
        fft_front_only.lfe_gain = 0.0;
        fft_front_only.surround_direct_bleed = 0.0;
        fft_front_only.rear_late_reflection = 0.0;
        fft_front_only.center_spread = 1.0;
        fft_front_only.dialogue_weight = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_fft_front_only",
            "high-latency FFT path with only front direct routing left active",
            fft_front_only,
        );

        let mut bypass_all = base;
        bypass_all.bypass_all_processing = true;
        bypass_all.enable_hr_direct = false;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "bypass_all",
            "pure stereo pass-through through the upmixer output contract",
            bypass_all,
        );
    }
    variants
}

pub(super) fn push_isolation_variant(
    variants: &mut Vec<IsolationVariant>,
    config: &str,
    frequency_resolution: &str,
    suffix: &str,
    notes: &str,
    params: UpmixerPluginParams,
) {
    let name = format!(
        "cfg{}_{}_{}",
        safe_filename_fragment(config),
        safe_filename_fragment(frequency_resolution),
        suffix
    );
    variants.push(IsolationVariant {
        name,
        config: config.to_string(),
        frequency_resolution: frequency_resolution.to_string(),
        notes: notes.to_string(),
        params,
    });
}
