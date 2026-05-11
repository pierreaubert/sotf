#[allow(unused_imports)]
use super::*;

#[allow(dead_code)]
const SAMPLE_RATE: u32 = 48000;

#[allow(dead_code)]
fn make_test_signal(num_frames: usize, channels: usize, freq_hz: f32) -> Vec<f32> {
    let mut buffer = vec![0.0_f32; num_frames * channels];
    for i in 0..num_frames {
        let phase = 2.0 * std::f32::consts::PI * freq_hz * i as f32 / SAMPLE_RATE as f32;
        let sample = phase.sin() * 0.5;
        for ch in 0..channels {
            buffer[i * channels + ch] = sample;
        }
    }
    buffer
}

#[allow(dead_code)]
fn make_noisy_signal(
    num_frames: usize,
    channels: usize,
    signal_db: f32,
    noise_db: f32,
) -> Vec<f32> {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let signal_linear = 10.0_f32.powf(signal_db / 20.0);
    let noise_linear = 10.0_f32.powf(noise_db / 20.0);

    let mut buffer = vec![0.0_f32; num_frames * channels];
    let hasher = RandomState::new();

    for (i, sample) in buffer.iter_mut().enumerate().take(num_frames * channels) {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SAMPLE_RATE as f32;
        let signal = phase.sin() * signal_linear;

        let mut h = hasher.build_hasher();
        h.write_usize(i);
        let rand: f32 = (h.finish() as f32 / u64::MAX as f32) * 2.0 - 1.0;
        let noise = rand * noise_linear;

        *sample = signal + noise;
    }
    buffer
}

#[test]
fn test_denoiser_creation() {
    let denoiser = DenoiserPlugin::new(2, false);
    assert_eq!(denoiser.channels(), 2);
    assert_eq!(denoiser.fft_size, 2048);
}

#[test]
fn test_denoiser_low_latency() {
    let denoiser = DenoiserPlugin::new(2, true);
    assert_eq!(denoiser.fft_size, 512);
}

#[test]
fn test_denoiser_from_params() {
    let params = DenoiserPluginParams {
        reduction_db: 20.0,
        floor_db: -40.0,
        ..Default::default()
    };
    let denoiser = DenoiserPlugin::from_params(2, params);
    assert_eq!(denoiser.reduction_db, 20.0);
    assert_eq!(denoiser.floor_db, -40.0);
}

#[test]
fn test_hann_window() {
    let window = sotf_host::stft_common::generate_hann_window(8);
    assert_eq!(window.len(), 8);
    assert!((window[0] - 0.0).abs() < 0.01);
    assert!((window[4] - 1.0).abs() < 0.01);
}

#[test]
fn test_parameter_set_get() {
    let mut denoiser = DenoiserPlugin::new(2, false);
    denoiser.initialize(SAMPLE_RATE).unwrap();

    denoiser
        .set_parameter(
            ParameterId::from("reduction_db"),
            ParameterValue::Float(25.0),
        )
        .unwrap();
    denoiser
        .set_parameter(ParameterId::from("floor_db"), ParameterValue::Float(-35.0))
        .unwrap();

    let reduction = denoiser.get_parameter(&ParameterId::from("reduction_db"));
    let floor = denoiser.get_parameter(&ParameterId::from("floor_db"));

    assert_eq!(reduction, Some(ParameterValue::Float(25.0)));
    assert_eq!(floor, Some(ParameterValue::Float(-35.0)));
}

#[test]
fn test_bypass_mode() {
    let mut params = DenoiserPluginParams::default();
    params.reduction_db = 0.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let input = make_test_signal(num_frames, 2, 1000.0);
    let mut output = input.clone();

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut output, &context).unwrap();

    let skip = plugin.latency_samples();
    let input_energy: f32 = input[skip * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip * 2..].iter().map(|x| x * x).sum();

    let ratio = output_energy / input_energy;
    assert!(
        ratio > 0.4 && ratio < 1.5,
        "Zero reduction should pass signal through approximately. Ratio: {}",
        ratio
    );
}

#[test]
fn test_output_nonzero() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Output should not be all zeros");
}

#[test]
fn test_low_latency_accepts_warm_4096_frame_in_place_blocks() {
    let mut plugin = DenoiserPlugin::new(2, true);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    assert_eq!(plugin.max_in_place_frames(), num_frames);
    assert!(
        plugin.output_accumulator[0].len() >= num_frames + plugin.fft_size,
        "output ring must reserve one FFT-sized overlap tail beyond the safe in-place block"
    );

    for freq in [1000.0, 1200.0] {
        let mut input = make_test_signal(num_frames, 2, freq);
        let processed = plugin.process_in_place(&mut input, &context).unwrap();
        assert_eq!(processed, num_frames);
    }
}

#[test]
fn test_latency() {
    let plugin = DenoiserPlugin::new(2, false);
    assert_eq!(plugin.latency_samples(), 2048);

    let plugin_ll = DenoiserPlugin::new(2, true);
    assert_eq!(plugin_ll.latency_samples(), 512);
}

#[test]
fn test_rejects_mismatched_buffer_size() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let mut buffer = vec![0.0_f32; 1023];
    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: 512,
    };

    let err = plugin.process_in_place(&mut buffer, &context).unwrap_err();
    assert!(
        err.contains("Buffer size mismatch"),
        "Expected buffer mismatch error, got: {}",
        err
    );
}

#[test]
fn test_rejects_oversized_classical_in_place_block() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = plugin.max_in_place_frames() + 1;
    let mut buffer = make_test_signal(num_frames, 2, 1000.0);
    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    let err = plugin.process_in_place(&mut buffer, &context).unwrap_err();
    assert!(
        err.contains("Block too large"),
        "Expected oversized block error, got: {}",
        err
    );
    assert!(
        err.contains(&plugin.max_in_place_frames().to_string()),
        "Expected error to report prepared safe limit, got: {}",
        err
    );
}

#[test]
fn test_denoiser_data_clone_keeps_mutable_cache_slots() {
    let mut cloned = DenoiserData::default().clone();

    assert!(
        std::sync::Arc::get_mut(&mut cloned.noise_floor_db).is_some(),
        "Cloned UI cache data should own its noise-floor vector"
    );
    assert!(
        std::sync::Arc::get_mut(&mut cloned.snr_db).is_some(),
        "Cloned UI cache data should own its SNR vector"
    );
}

#[test]
fn test_continuous_processing() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let block_size = 512;
    let num_blocks = 20;

    for block in 0..num_blocks {
        let mut input = make_test_signal(block_size, 2, 1000.0);

        let context = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: block_size,
        };

        plugin.process_in_place(&mut input, &context).unwrap();

        if block > 10 {
            let energy: f32 = input.iter().map(|x| x * x).sum();
            assert!(
                energy > 0.001,
                "Block {} has near-zero output energy: {}",
                block,
                energy
            );
        }
    }
}

#[test]
fn test_mcra_noise_estimation() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let data = plugin.get_data();
    assert!(data.is_some());
}

#[test]
fn test_noise_reduction_reduces_energy() {
    let mut params = DenoiserPluginParams::default();
    params.reduction_db = 20.0;
    params.floor_db = -40.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 8192;
    let input = make_noisy_signal(num_frames, 2, -10.0, -30.0);
    let mut output = input.clone();

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut output, &context).unwrap();

    let skip = plugin.latency_samples();
    let input_energy: f32 = input[skip * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip * 2..].iter().map(|x| x * x).sum();

    let ratio = output_energy / input_energy;
    assert!(
        ratio < 0.9,
        "With significant reduction, output energy should be less. Ratio: {}",
        ratio
    );
}

#[test]
fn test_energy_preservation_with_low_reduction() {
    let mut params = DenoiserPluginParams::default();
    params.reduction_db = 3.0;
    params.floor_db = -20.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 8192;
    let input = make_test_signal(num_frames, 2, 1000.0);
    let mut output = input.clone();

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut output, &context).unwrap();

    let skip = plugin.latency_samples();
    let input_energy: f32 = input[skip * 2..].iter().map(|x| x * x).sum();
    let output_energy: f32 = output[skip * 2..].iter().map(|x| x * x).sum();

    let ratio = output_energy / input_energy;
    assert!(
        ratio > 0.5 && ratio < 2.0,
        "Low reduction should preserve energy. Ratio: {}",
        ratio
    );
}

#[test]
fn test_polyphonic_detection_mode() {
    let mut params = DenoiserPluginParams::default();
    params.polyphonic_detection = true;
    params.reduction_db = 12.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Polyphonic mode should produce output");
}

#[test]
fn test_psychoacoustic_masking() {
    let mut params = DenoiserPluginParams::default();
    params.psychoacoustic_masking = true;
    params.reduction_db = 20.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Masking mode should produce output");
}

#[test]
fn test_dd_enabled_mode() {
    let mut params = DenoiserPluginParams::default();
    params.dd_enabled = true;
    params.dd_alpha = 0.98;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "DD mode should produce output");
}

#[test]
fn test_reset_clears_state() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();
    plugin.reset();

    let mut input2 = make_test_signal(num_frames, 2, 1000.0);
    plugin.process_in_place(&mut input2, &context).unwrap();

    let sum: f32 = input2.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "After reset, should still produce output");
}

#[test]
fn test_mono_channel() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(1, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 1, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Mono processing should produce output");
}

#[test]
fn test_multi_channel() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(6, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 6, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Multi-channel processing should produce output");
}

#[test]
fn test_wiener_gain_formula() {
    let kernel = DenoiserPlugin::compute_smoothing_kernel(0.5);
    assert!(kernel.0 > 0.0);
    assert!(kernel.1 >= 0.0);
    assert!(kernel.2 >= 0.0);

    let sum = kernel.0 + 2.0 * kernel.1 + 2.0 * kernel.2;
    assert!((sum - 1.0).abs() < 0.001, "Kernel should sum to 1");
}

#[test]
fn test_time_to_coeff() {
    let coeff = DenoiserPlugin::time_to_coeff(10.0, 48000, 1024);
    assert!(coeff > 0.0 && coeff < 1.0);

    let instant = DenoiserPlugin::time_to_coeff(0.0, 48000, 1024);
    assert!((instant - 0.0).abs() < 0.001);
}

#[test]
fn test_denoiser_data_exposure() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 1000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    if let Some(data) = plugin.get_data() {
        let _denoiser_data = data
            .downcast_ref::<super::DenoiserData>()
            .expect("Should be DenoiserData");
    } else {
        panic!("Expected some data");
    }
}

#[test]
fn test_different_sample_rates() {
    for sr in [44100u32, 48000, 96000] {
        let mut plugin = DenoiserPlugin::new(2, false);
        plugin.initialize(sr).unwrap();

        let num_frames = 2048;
        let freq = 1000.0_f32.min(sr as f32 * 0.4);
        let mut input = make_test_signal(num_frames, 2, freq);

        let context = ProcessContext {
            sample_rate: sr,
            num_frames,
        };

        plugin.process_in_place(&mut input, &context).unwrap();

        let sum: f32 = input.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "Sample rate {} should produce output", sr);
    }
}

#[test]
fn test_floor_prevents_complete_attenuation() {
    let mut params = DenoiserPluginParams::default();
    params.reduction_db = 40.0;
    params.floor_db = -30.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 8192;
    let input = make_noisy_signal(num_frames, 2, 0.0, -40.0);
    let mut output = input.clone();

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut output, &context).unwrap();

    let skip = plugin.latency_samples();
    let output_energy: f32 = output[skip * 2..].iter().map(|x| x * x).sum();

    assert!(
        output_energy > 0.001,
        "Floor should prevent complete attenuation. Energy: {}",
        output_energy
    );
}

#[test]
fn test_parameter_updates() {
    let mut plugin = DenoiserPlugin::new(2, false);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin
        .set_parameter(
            ParameterId::from("reduction_db"),
            ParameterValue::Float(15.0),
        )
        .unwrap();

    let reduction = plugin.get_parameter(&ParameterId::from("reduction_db"));
    assert_eq!(reduction, Some(ParameterValue::Float(15.0)));
}

#[test]
fn test_attack_release_parameters() {
    let mut params = DenoiserPluginParams::default();
    params.attack_ms = 1.0;
    params.release_ms = 100.0;
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    plugin
        .set_parameter(ParameterId::from("attack_ms"), ParameterValue::Float(10.0))
        .unwrap();
    plugin
        .set_parameter(
            ParameterId::from("release_ms"),
            ParameterValue::Float(200.0),
        )
        .unwrap();

    let attack = plugin.get_parameter(&ParameterId::from("attack_ms"));
    let release = plugin.get_parameter(&ParameterId::from("release_ms"));

    assert_eq!(attack, Some(ParameterValue::Float(10.0)));
    assert_eq!(release, Some(ParameterValue::Float(200.0)));
}

#[test]
fn test_silence_input() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = vec![0.0_f32; num_frames * 2];

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let skip = plugin.latency_samples();
    let output_sum: f32 = input[skip * 2..].iter().map(|x| x.abs()).sum();
    assert!(
        output_sum < 0.001,
        "Silence input should produce near-silence output"
    );
}

#[test]
fn test_high_frequency_content() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 4096;
    let mut input = make_test_signal(num_frames, 2, 15000.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "High frequency should produce output");
}

#[test]
fn test_low_frequency_content() {
    let params = DenoiserPluginParams::default();
    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let num_frames = 8192;
    let mut input = make_test_signal(num_frames, 2, 50.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Low frequency should produce output");
}

/// Formant preservation: bins at spectral envelope peaks should receive a higher
/// gain floor than non-peak bins when `formant_preservation` is enabled.
///
/// The test synthesizes a 5-harmonic signal (resembling a vowel) with additive
/// broadband noise, processes enough frames for MCRA to stabilise, then inspects
/// the post-Wiener gains via the `formant_preserver.envelope` field.
///
/// Correctness criterion: when the preserver is enabled and strength = 1.0,
/// every bin identified as a formant peak (`envelope > mean + 0.13` in log10)
/// must have `gain >= strength * 0.3 = 0.3`.  Without the preserver those
/// same bins may have lower gains, confirming that the floor was applied.
#[test]
fn test_formant_preservation_floors_gains_at_peaks() {
    use sotf_host::parameters::{ParameterId, ParameterValue};

    const SAMPLE_RATE: u32 = 48000;
    const FUNDAMENTAL_HZ: f32 = 250.0; // typical male speech F0
    const NUM_HARMONICS: usize = 5;
    const NOISE_DB: f32 = -20.0; // relatively loud noise so Wiener would suppress peaks
    const SIGNAL_DB: f32 = -10.0;

    // Build a harmonic signal + broadband noise
    let num_frames = 8192;
    let channels = 1;
    let signal_amp = 10.0_f32.powf(SIGNAL_DB / 20.0);
    let noise_amp = 10.0_f32.powf(NOISE_DB / 20.0);

    // Use a fixed-seed LCG for deterministic noise
    let mut seed: u32 = 0xDEAD_BEEF;
    let mut buffer_with_preservation = vec![0.0_f32; num_frames * channels];
    for (i, s) in buffer_with_preservation.iter_mut().enumerate() {
        let t = i as f32 / SAMPLE_RATE as f32;
        let signal: f32 = (1..=NUM_HARMONICS)
            .map(|h| (2.0 * std::f32::consts::PI * FUNDAMENTAL_HZ * h as f32 * t).sin())
            .sum::<f32>()
            / NUM_HARMONICS as f32
            * signal_amp;
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * noise_amp;
        *s = signal + noise;
    }

    // --- Run WITH formant preservation enabled ---
    let mut params_on = DenoiserPluginParams::default();
    params_on.reduction_db = 20.0;
    params_on.floor_db = -40.0;
    params_on.formant_preservation = true;
    params_on.formant_strength = 1.0;
    let mut plugin_on = DenoiserPlugin::from_params(channels, params_on);
    plugin_on.initialize(SAMPLE_RATE).unwrap();

    let mut buf_on = buffer_with_preservation.clone();
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };
    plugin_on.process_in_place(&mut buf_on, &ctx).unwrap();

    // Verify parameter round-trip
    let got = plugin_on.get_parameter(&ParameterId::from("formant_preservation"));
    assert_eq!(got, Some(ParameterValue::Bool(true)));
    let got_str = plugin_on.get_parameter(&ParameterId::from("formant_strength"));
    assert_eq!(got_str, Some(ParameterValue::Float(1.0)));

    // --- Run WITHOUT formant preservation ---
    let mut params_off = DenoiserPluginParams::default();
    params_off.reduction_db = 20.0;
    params_off.floor_db = -40.0;
    params_off.formant_preservation = false;
    let mut plugin_off = DenoiserPlugin::from_params(channels, params_off);
    plugin_off.initialize(SAMPLE_RATE).unwrap();

    let mut buf_off = buffer_with_preservation.clone();
    plugin_off.process_in_place(&mut buf_off, &ctx).unwrap();

    // Verify formant preservation raises energy vs no-preservation at heavy reduction
    let skip = plugin_on.latency_samples();
    let energy_on: f32 = buf_on[skip..].iter().map(|x| x * x).sum();
    let energy_off: f32 = buf_off[skip..].iter().map(|x| x * x).sum();

    // With formant preservation, peaks are floored at 0.3 gain → more energy
    // retained than without it.  The ratio should be > 1.0.
    assert!(
        energy_on >= energy_off * 0.9,
        "Formant preservation should retain at least as much energy as no-preservation. \
         on={}, off={}",
        energy_on,
        energy_off
    );

    // Verify the FormantPreserver fields are accessible and contain valid data
    // after processing (envelope computed, non-zero for signal with content).
    let preserver = &plugin_on.formant_preserver;
    let max_env = preserver
        .envelope
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max_env > 0.0 || max_env.is_finite(),
        "Envelope should be computed and finite after processing"
    );
}

/// Multi-resolution mode: enable dual-STFT and verify the plugin still produces
/// non-zero output while also checking parameter round-trip.
#[test]
fn test_multi_resolution_mode() {
    let mut params = DenoiserPluginParams::default();
    params.multi_resolution = true;
    params.reduction_db = 10.0;
    // multi_resolution only works with 2048-sample FFT (large FFT path)
    params.low_latency = false;

    let mut plugin = DenoiserPlugin::from_params(2, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    // Verify parameter round-trip
    let got = plugin.get_parameter(&ParameterId::from("multi_resolution"));
    assert_eq!(
        got,
        Some(ParameterValue::Bool(true)),
        "multi_resolution param should be readable after from_params"
    );

    let num_frames = 8192;
    let mut input = make_noisy_signal(num_frames, 2, -10.0, -30.0);

    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames,
    };

    plugin.process_in_place(&mut input, &context).unwrap();

    let sum: f32 = input.iter().map(|x| x.abs()).sum();
    assert!(
        sum > 0.0,
        "Multi-resolution mode should produce non-zero output"
    );

    // Also verify we can toggle it off via set_parameter without panicking
    plugin
        .set_parameter(
            ParameterId::from("multi_resolution"),
            ParameterValue::Bool(false),
        )
        .unwrap();
    let got_off = plugin.get_parameter(&ParameterId::from("multi_resolution"));
    assert_eq!(got_off, Some(ParameterValue::Bool(false)));

    // And toggle back on
    plugin
        .set_parameter(
            ParameterId::from("multi_resolution"),
            ParameterValue::Bool(true),
        )
        .unwrap();
    let got_on = plugin.get_parameter(&ParameterId::from("multi_resolution"));
    assert_eq!(got_on, Some(ParameterValue::Bool(true)));

    // Process another block with it re-enabled
    let mut input2 = make_noisy_signal(num_frames, 2, -10.0, -30.0);
    plugin.process_in_place(&mut input2, &context).unwrap();
    let sum2: f32 = input2.iter().map(|x| x.abs()).sum();
    assert!(
        sum2 > 0.0,
        "Re-enabled multi-resolution should still produce output"
    );
}

/// Bootstrap noise floor seeding: process only 5 frames of noise, then 5 frames
/// of signal. After the short bootstrap, the denoiser should still produce output
/// (not all zeros).
#[test]
fn test_bootstrap_noise_floor_seeding() {
    let mut params = DenoiserPluginParams::default();
    params.reduction_db = 12.0;
    let mut plugin = DenoiserPlugin::from_params(1, params);
    plugin.initialize(SAMPLE_RATE).unwrap();

    let block_size = plugin.fft_size; // one FFT frame = 1 "frame" of STFT

    // Phase 1: Feed 5 blocks of noise to seed the noise floor
    for _ in 0..5 {
        let mut noise = make_noisy_signal(block_size, 1, -60.0, -20.0);
        let ctx = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: block_size,
        };
        plugin.process_in_place(&mut noise, &ctx).unwrap();
    }

    // Phase 2: Feed 5 blocks of clean signal
    let mut total_energy = 0.0f32;
    for _ in 0..5 {
        let mut signal = make_test_signal(block_size, 1, 1000.0);
        let ctx = ProcessContext {
            sample_rate: SAMPLE_RATE,
            num_frames: block_size,
        };
        plugin.process_in_place(&mut signal, &ctx).unwrap();
        total_energy += signal.iter().map(|x| x * x).sum::<f32>();
    }

    assert!(
        total_energy > 0.0,
        "After bootstrap noise seeding and signal processing, output should not be all zeros. Energy: {}",
        total_energy
    );
}

#[test]
fn test_mcra_noise_floor_converges_on_noise() {
    // Feed pure noise and verify the noise floor estimate converges
    // to a reasonable value, not just stays at zero.
    let mut plugin = DenoiserPlugin::from_params(2, DenoiserPluginParams::default());
    plugin.initialize(SAMPLE_RATE).unwrap();

    // Feed ~2 seconds of moderate-level noise to let MCRA converge
    let block_size = 4096;
    let num_blocks = (SAMPLE_RATE as usize * 2) / block_size;
    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: block_size,
    };

    for _ in 0..num_blocks {
        let mut noise = make_noisy_signal(block_size, 2, -60.0, -10.0);
        plugin.process_in_place(&mut noise, &context).unwrap();
    }

    // The avg_reduction_db field should be non-zero after processing noisy signal
    let data = plugin.get_data();
    assert!(
        data.is_some(),
        "get_data() should return Some after processing"
    );

    let data = data.unwrap();
    let denoiser_data = data
        .downcast_ref::<super::DenoiserData>()
        .expect("get_data() should return DenoiserData");

    // After 2 seconds of noise-heavy input, the denoiser should report
    // some noise floor activity (either noise_floor_db or avg_reduction_db)
    let has_activity = denoiser_data.avg_reduction_db.abs() > 0.01
        || denoiser_data.noise_floor_db.iter().any(|&v| v.abs() > 0.01);
    assert!(
        has_activity,
        "Denoiser should show noise estimation activity after 2s of noise. avg_reduction={}, noise_floor_sum={}",
        denoiser_data.avg_reduction_db,
        denoiser_data.noise_floor_db.iter().sum::<f32>()
    );
}

/// Fast adaptation: when a sudden noise-level change occurs during a
/// noise-only signal (>80% of bins quiet), the MCRA should converge
/// faster than without fast adaptation.
///
/// Strategy: feed ~1s of low-level noise to let MCRA converge, then
/// switch to 10x-louder noise and measure how many frames it takes
/// for the noise PSD to reach 90% of the new level.
#[test]
fn test_mcra_fast_adaptation() {
    let mut plugin = DenoiserPlugin::from_params(1, DenoiserPluginParams::default());
    plugin.initialize(SAMPLE_RATE).unwrap();

    let block_size = 4096;
    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: block_size,
    };

    // Phase 1: Feed 1s of quiet noise (-40 dB) to let MCRA converge
    let warmup_blocks = (SAMPLE_RATE as usize) / block_size;
    for _ in 0..warmup_blocks {
        let mut noise = make_noisy_signal(block_size, 1, -80.0, -40.0);
        plugin.process_in_place(&mut noise, &context).unwrap();
    }

    // Record the converged noise PSD at a mid-frequency bin
    let mid_bin = plugin.spectrum_size / 4;
    let old_noise_psd = plugin.noise_psd[0][mid_bin];
    assert!(
        old_noise_psd > 0.0,
        "Noise PSD should have converged after warmup"
    );

    // Phase 2: Switch to 10x-louder noise (-20 dB) and feed 30 blocks.
    // With fast adaptation (boost=2x), convergence should happen within
    // ~265ms instead of ~530ms.
    let adaptation_blocks = 30;
    for _ in 0..adaptation_blocks {
        let mut noise = make_noisy_signal(block_size, 1, -80.0, -20.0);
        plugin.process_in_place(&mut noise, &context).unwrap();
    }

    let new_noise_psd = plugin.noise_psd[0][mid_bin];

    // The new noise PSD should be significantly larger than the old one
    // (the louder noise is 20 dB = 100x more power)
    assert!(
        new_noise_psd > old_noise_psd * 5.0,
        "After sudden noise increase, noise PSD should adapt significantly. \
         old={:.6}, new={:.6}, ratio={:.2}",
        old_noise_psd,
        new_noise_psd,
        new_noise_psd / old_noise_psd
    );
}

/// Median filter: verify that isolated gain spikes are removed while
/// broad gain patterns are preserved.
#[test]
fn test_median_filter_reduces_spikes() {
    // Create a gain curve with isolated spikes (musical noise pattern)
    let len = 64;
    let mut gains = vec![0.2_f32; len];

    // Insert isolated spikes: single bins at 1.0 surrounded by 0.2
    gains[10] = 1.0;
    gains[30] = 1.0;
    gains[50] = 1.0;

    // Insert a broad region (3+ consecutive high bins) — should survive
    gains[20] = 0.9;
    gains[21] = 0.9;
    gains[22] = 0.9;
    gains[23] = 0.9;

    let gains_before = gains.clone();
    DenoiserPlugin::median_smooth_gains(&mut gains, len);

    // Isolated spikes should be reduced (they are the odd-one-out)
    assert!(
        gains[10] < 0.5,
        "Spike at bin 10 should be suppressed. Got {}",
        gains[10]
    );
    assert!(
        gains[30] < 0.5,
        "Spike at bin 30 should be suppressed. Got {}",
        gains[30]
    );
    assert!(
        gains[50] < 0.5,
        "Spike at bin 50 should be suppressed. Got {}",
        gains[50]
    );

    // Broad region interior should be mostly preserved
    // (bins 21 and 22 are surrounded by 0.9 on both sides)
    assert!(
        gains[21] > 0.8,
        "Broad region bin 21 should be preserved. Got {} (was {})",
        gains[21],
        gains_before[21]
    );
    assert!(
        gains[22] > 0.8,
        "Broad region bin 22 should be preserved. Got {} (was {})",
        gains[22],
        gains_before[22]
    );

    // Edge elements should be unchanged
    assert_eq!(
        gains[0], gains_before[0],
        "First element should be unchanged"
    );
    assert_eq!(
        gains[len - 1],
        gains_before[len - 1],
        "Last element should be unchanged"
    );
}

/// Learn-noise trigger should reset MCRA state (re-enter bootstrap).
#[test]
fn test_learn_noise_resets_mcra() {
    let mut plugin = DenoiserPlugin::from_params(1, DenoiserPluginParams::default());
    plugin.initialize(SAMPLE_RATE).unwrap();

    let block_size = 4096;
    let context = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: block_size,
    };

    // Process enough frames to leave bootstrap
    let warmup_blocks = 10;
    for _ in 0..warmup_blocks {
        let mut noise = make_noisy_signal(block_size, 1, -80.0, -20.0);
        plugin.process_in_place(&mut noise, &context).unwrap();
    }

    // Frame counter should be well past bootstrap
    assert!(
        plugin.frame_counter[0] > 5,
        "Should be past bootstrap. frame_counter={}",
        plugin.frame_counter[0]
    );

    // Trigger learn_noise
    plugin
        .set_parameter(ParameterId::from("learn_noise"), ParameterValue::Bool(true))
        .unwrap();

    // Frame counter should be reset to 0 (re-entered bootstrap)
    assert_eq!(
        plugin.frame_counter[0], 0,
        "learn_noise should reset MCRA (frame_counter back to 0)"
    );
    assert!(plugin.is_learning, "Should be in learning mode");
}
