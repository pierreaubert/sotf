use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{Plugin, UpmixerPlugin, UpmixerPluginParams};

pub(super) struct UpmixerFuzzer;

impl PluginFuzzer for UpmixerFuzzer {
    fn create_plugin(&self, _channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        // Upmixer always takes 2 channels input
        let speaker_configs = ["5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4"];
        let speaker_config =
            speaker_configs[rng.random_range(0..speaker_configs.len())].to_string();

        // Random FFT size (power of 2)
        let fft_sizes = [1024, 2048, 4096];
        let fft_size = fft_sizes[rng.random_range(0..fft_sizes.len())];

        // Random parameters with reasonable ranges
        let gain_front_direct = rng.random_range(0.5..1.5);
        let gain_front_ambient = rng.random_range(0.0..1.0);
        let gain_rear_ambient = rng.random_range(0.5..2.0);
        let lfe_cutoff_hz = rng.random_range(80.0..150.0);
        let stereo_width = rng.random_range(0.0..1.0);
        let bandpass_hz = rng.random_range(150.0..400.0);
        let center_spread = rng.random_range(0.0..0.5);
        let height_gain = rng.random_range(0.0..0.5);
        let lfe_gain = rng.random_range(0.5..1.5);
        let subharmonic_gain = rng.random_range(0.0..1.0);
        let hr_sharpen = rng.random_range(0.5..2.0);
        let safety_cap_db = rng.random_range(0.0..6.0);

        let enable_subharmonic_synth = rng.random_bool(0.5);
        let enable_hr_direct = rng.random_bool(0.3); // Less frequent, experimental feature
        let decorrelation_mode = rng.random_range(0..=1); // 0=Velvet, 1=LFO

        let mut params = UpmixerPluginParams::default();
        params.core.fft_size = fft_size;
        params.core.speaker_config = speaker_config.clone();
        params.gains.gain_front_direct = gain_front_direct;
        params.gains.gain_front_ambient = gain_front_ambient;
        params.gains.gain_rear_ambient = gain_rear_ambient;
        params.core.lfe_cutoff_hz = lfe_cutoff_hz;
        params.gains.stereo_width = stereo_width;
        params.core.bandpass_hz = bandpass_hz;
        params.gains.center_spread = center_spread;
        params.height.height_gain = height_gain;
        params.gains.lfe_gain = lfe_gain;
        params.subharmonic.enable_subharmonic_synth = enable_subharmonic_synth;
        params.subharmonic.subharmonic_gain = subharmonic_gain;
        params.core.enable_hr_direct = enable_hr_direct;
        params.gains.hr_sharpen = hr_sharpen;
        params.core.safety_cap_db = safety_cap_db;
        params.decorrelation.decorrelation_mode = decorrelation_mode;
        params.subharmonic.subharmonic_freq_hz = 40.0;
        params.subharmonic.subharmonic_attack_ms = 10.0;
        params.subharmonic.subharmonic_release_ms = 50.0;
        params.decorrelation.decorrelation_lfo_rate_hz = 0.15;
        params.decorrelation.velvet_noise_duration_ms = 30.0;
        params.decorrelation.velvet_noise_density = 2000.0;
        params.height.height_hf_cap_hz = 16000.0;
        params.height.height_transient_reduction = 0.6;
        params.height.height_direct_leak = 0.15;
        params.surround.surround_direct_bleed = 0.50;
        params.surround.rear_ambient_boost = 1.5;
        params.surround.rear_late_reflection = 0.10;
        params.surround.ambient_boost = 1.2;
        params.dialogue.dialogue_weight = 0.4;
        params.dialogue.voice_freq_min_hz = 500.0;
        params.dialogue.voice_freq_max_hz = 3000.0;
        params.dialogue.dialogue_centroid_weight = 0.3;
        params.dialogue.dialogue_variance_weight = 0.2;
        params.dialogue.dialogue_coherence_weight = 0.5;
        params.ml.enable_ml_detection = false;
        params.ml.ml_model_path = String::new();
        params.bypass.bypass_decorrelation = false;
        params.bypass.bypass_transient_detection = false;
        params.bypass.bypass_all_processing = false;
        params.core.low_latency = false;
        params.core.frequency_resolution = "erb".to_string();
        params.spectral.multi_source_extraction = false;
        params.spectral.multi_source_threshold = 0.8;
        params.core.binaural_preview = false;
        params.output.auto_gain_enabled = false;
        params.output.auto_gain_max_db = 12.0;
        params.output.auto_gain_smoothing_ms = 100.0;

        let desc = format!(
            "config={} fft={} g_fd={:.2} g_fa={:.2} g_ra={:.2} lfe_co={:.0}Hz sw={:.2} bp={:.0}Hz cs={:.2} hg={:.2} lfeg={:.2} subh={}/{:.2} hr={}/{:.2} cap={:.1}dB decor={}",
            speaker_config,
            fft_size,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            lfe_cutoff_hz,
            stereo_width,
            bandpass_hz,
            center_spread,
            height_gain,
            lfe_gain,
            enable_subharmonic_synth,
            subharmonic_gain,
            enable_hr_direct,
            hr_sharpen,
            safety_cap_db,
            decorrelation_mode
        );

        (Box::new(UpmixerPlugin::from_params(params)), desc)
    }
}
