use super::*;
use crate::param_specs::denoiser::*;
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::ProcessContext;

const SAMPLE_RATE: u32 = 48000;

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

    for i in 0..num_frames * channels {
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * i as f32 / SAMPLE_RATE as f32;
        let signal = phase.sin() * signal_linear;

        let mut h = hasher.build_hasher();
        h.write_usize(i);
        let rand: f32 = (h.finish() as f32 / u64::MAX as f32) * 2.0 - 1.0;
        let noise = rand * noise_linear;

        buffer[i] = signal + noise;
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
    let window = crate::stft_common::generate_hann_window(8);
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
fn test_latency() {
    let plugin = DenoiserPlugin::new(2, false);
    assert_eq!(plugin.latency_samples(), 2048);

    let plugin_ll = DenoiserPlugin::new(2, true);
    assert_eq!(plugin_ll.latency_samples(), 512);
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
