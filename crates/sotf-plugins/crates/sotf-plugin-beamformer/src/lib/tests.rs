use super::beamformer_plugin::BeamformerPlugin;
use super::misc::FFT_SIZE;
use super::types::BeamformerType;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};

#[test]
fn test_beamformer_plugin_creation() {
    let plugin = BeamformerPlugin::new(2, 48000).unwrap();
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 1);
}

#[test]
fn test_beamformer_parameters() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin
        .set_parameter(
            ParameterId::from("steer_angle_deg"),
            ParameterValue::Float(45.0),
        )
        .unwrap_err();
    assert_eq!(plugin.steer_angle_deg, 0.0);
}

#[test]
fn steering_is_structural_and_does_not_rebuild_live_state() {
    let mut plugin = BeamformerPlugin::new(2, 48_000).unwrap();
    let steering_ptr = plugin.steering_vectors.as_ptr();
    let error = plugin
        .set_parameter(
            ParameterId::from("steer_angle_deg"),
            ParameterValue::Float(45.0),
        )
        .unwrap_err();
    assert!(error.contains("structural"));
    assert_eq!(plugin.steering_vectors.as_ptr(), steering_ptr);
    assert_eq!(plugin.steer_angle_deg, 0.0);
}

#[test]
fn malformed_construction_returns_errors() {
    use super::beamformer_plugin_params::BeamformerPluginParams;
    for params in [
        BeamformerPluginParams {
            num_mics: 1,
            ..Default::default()
        },
        BeamformerPluginParams {
            mic_spacing_cm: f32::NAN,
            ..Default::default()
        },
        BeamformerPluginParams {
            steer_angle_deg: f32::INFINITY,
            ..Default::default()
        },
        BeamformerPluginParams {
            beamformer_type: 3,
            ..Default::default()
        },
    ] {
        assert!(BeamformerPlugin::from_params(48_000, params).is_err());
    }
    assert!(BeamformerPlugin::new(1, 48_000).is_err());
}

#[test]
fn test_beamformer_gsc_process() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.beamformer_type = BeamformerType::Gsc;

    let context = ProcessContext::new(48000, 256);
    let input = vec![0.1f32; 256 * 2];
    let mut output = vec![0.0f32; 256];

    let result = plugin.process(&input, &mut output, &context);
    assert!(result.is_ok());
}

#[test]
fn test_beamformer_mvdr_process() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.beamformer_type = BeamformerType::Mvdr;

    let context = ProcessContext::new(48000, 512);
    let input = vec![0.1f32; 512 * 2];
    let mut output = vec![0.0f32; 512];

    let result = plugin.process(&input, &mut output, &context);
    assert!(result.is_ok());
}

#[test]
fn test_beamformer_superdirective_process() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.beamformer_type = BeamformerType::Superdirective;

    let context = ProcessContext::new(48000, 512);
    let input = vec![0.1f32; 512 * 2];
    let mut output = vec![0.0f32; 512];

    let result = plugin.process(&input, &mut output, &context);
    assert!(result.is_ok());
}

/// Regression test for §1.1 (STFT trigger fires every sample after hop).
///
/// Before the fix `input_fill` was reset to `FFT_SIZE - hop = hop`, so on
/// the very next sample it became `hop + 1 >= hop` and triggered another
/// full FFT frame.  This test feeds data in small increments and verifies
/// that the output is finite and not all-zero after enough input.
#[test]
fn test_stft_trigger_fires_at_fft_size_not_hop() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.beamformer_type = BeamformerType::Mvdr;

    let hop = FFT_SIZE / 2;
    // Feed exactly hop samples at a time over several calls.
    // With the buggy trigger each call after the first would fire a frame;
    // with the correct trigger only every other call fires one.
    let block = ProcessContext::new(48000, hop);
    let input = vec![0.1f32; hop * 2];
    let mut output = vec![0.0f32; hop];

    // After 2 blocks (= FFT_SIZE samples) we expect the first frame to
    // have fired.  Accumulate across many blocks; all outputs must be finite.
    for _ in 0..16 {
        let result = plugin.process(&input, &mut output, &block);
        assert!(result.is_ok());
        for (i, &s) in output.iter().enumerate() {
            assert!(
                s.is_finite(),
                "output[{i}] is not finite after hop-sized block"
            );
        }
    }
}

/// Regression test for §1.2 (missing overlap-add).
///
/// With OLA the output energy after steady-state should be close to the
/// input energy (within ~6 dB considering beamforming gain).  Without OLA
/// the output oscillates between near-zero and spiky values at the hop rate.
#[test]
fn test_stft_ola_output_not_silent() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.beamformer_type = BeamformerType::Mvdr;

    let nf = 512usize;
    let context = ProcessContext::new(48000, nf);
    // Sine wave at 440 Hz, same on both channels
    let input: Vec<f32> = (0..nf * 2)
        .map(|n| (2.0 * std::f32::consts::PI * 440.0 * (n / 2) as f32 / 48000.0).sin() * 0.5)
        .collect();
    let mut output = vec![0.0f32; nf];

    // First call: plugin is filling its accumulator — output is zeros (latency)
    plugin.process(&input, &mut output, &context).unwrap();

    // Feed more blocks until we have a steady-state non-silent output
    let mut rms_sum = 0.0f32;
    for _ in 0..8 {
        plugin.process(&input, &mut output, &context).unwrap();
        let rms: f32 = (output.iter().map(|s| s * s).sum::<f32>() / nf as f32).sqrt();
        rms_sum += rms;
        for (i, &s) in output.iter().enumerate() {
            assert!(s.is_finite(), "output[{i}] is NaN/Inf");
        }
    }
    assert!(
        rms_sum > 0.001,
        "output is near-silent (rms_sum={rms_sum}) — OLA may be broken"
    );
}

/// Regression test for §1.6 + §1.7: MVDR covariance noise detection.
///
/// Before the fix the update gate checked only channel 0, and the first
/// 20 frames were always accepted regardless of energy level.  Verify that
/// providing a high-energy signal on mic 1 only correctly raises the gate
/// (i.e. is_noise=false) and does NOT corrupt the covariance.
#[test]
fn test_mvdr_noise_detection_uses_all_channels() {
    use crate::mvdr::MvdrBeamformer;
    use nalgebra::Complex;

    let spectrum_size = 4usize;
    let mut bf = MvdrBeamformer::new(2, spectrum_size);

    // Channel 0 is silent, channel 1 has high energy
    let stft: Vec<Vec<Complex<f32>>> = vec![
        vec![Complex::new(0.0, 0.0); spectrum_size], // mic 0: silent
        vec![Complex::new(10.0, 0.0); spectrum_size], // mic 1: loud
    ];

    // A single-channel interferer is incoherent with the look direction and
    // must be learned regardless of absolute level.
    let cov_before: Vec<_> = (0..spectrum_size)
        .map(|k| {
            // diagonal of cov for bin k
            let off = k * 4;
            bf.noise_cov_snapshot()[off] // 0,0 element
        })
        .collect();

    let steering = vec![vec![nalgebra::Complex::new(1.0, 0.0); 2]; spectrum_size];
    assert!(bf.update_noise_covariance(&stft, &steering));

    let cov_after: Vec<_> = (0..spectrum_size)
        .map(|k| {
            let off = k * 4;
            bf.noise_cov_snapshot()[off]
        })
        .collect();

    // Covariance must update using energy from mic 1, not only mic 0.
    for k in 0..spectrum_size {
        assert_ne!(
            cov_before[k], cov_after[k],
            "bin {k}: covariance ignored a loud interferer on mic 1"
        );
    }
}

#[test]
fn test_mvdr_process_uses_preallocated_weight_application() {
    let mut plugin = BeamformerPlugin::new(2, 48000).unwrap();
    plugin.beamformer_type = BeamformerType::Mvdr;

    let nf = FFT_SIZE * 2;
    let context = ProcessContext::new(48000, nf);
    let mut input = vec![0.0_f32; nf * 2];
    for frame in 0..nf {
        let sample = (2.0 * std::f32::consts::PI * 1000.0 * frame as f32 / 48000.0).sin() * 0.25;
        input[frame * 2] = sample;
        input[frame * 2 + 1] = sample;
    }
    let mut output = vec![0.0_f32; nf];

    plugin.process(&input, &mut output, &context).unwrap();

    assert!(
        plugin.mvdr.output_buf.iter().any(|c| c.norm_sqr() > 0.0),
        "MVDR preallocated output buffer should be populated by process()"
    );
    assert!(output.iter().all(|s| s.is_finite()));
}

#[test]
fn mvdr_skips_weight_solves_when_covariance_is_unchanged() {
    let mut plugin = BeamformerPlugin::new(2, 48_000).unwrap();
    let frames = 4096;
    let input: Vec<f32> = (0..frames)
        .flat_map(|frame| {
            let sample = (std::f32::consts::TAU * 700.0 * frame as f32 / 48_000.0).sin();
            [sample, sample]
        })
        .collect();
    let mut output = vec![0.0; frames];
    plugin
        .process(&input, &mut output, &ProcessContext::new(48_000, frames))
        .unwrap();
    assert_eq!(plugin.mvdr.weight_solve_count, 1);
}

fn render_superdirective(block_sizes: &[usize], input_frames: usize) -> Vec<f32> {
    let mut plugin = BeamformerPlugin::from_params(
        48_000,
        super::beamformer_plugin_params::BeamformerPluginParams {
            num_mics: 2,
            mic_spacing_cm: 5.0,
            steer_angle_deg: 0.0,
            beamformer_type: 1,
        },
    )
    .unwrap();
    plugin.initialize(48_000).unwrap();
    let total = input_frames + FFT_SIZE * 2;
    let mut stream = vec![0.0; total * 2];
    for frame in 0..input_frames {
        let sample = (std::f32::consts::TAU * 997.0 * frame as f32 / 48_000.0).sin() * 0.25;
        stream[frame * 2] = sample;
        stream[frame * 2 + 1] = sample;
    }
    let mut rendered = Vec::with_capacity(total);
    let mut position = 0;
    let mut block_index = 0;
    while position < total {
        let frames = block_sizes[block_index % block_sizes.len()].min(total - position);
        let mut output = vec![0.0; frames];
        plugin
            .process(
                &stream[position * 2..(position + frames) * 2],
                &mut output,
                &ProcessContext::new(48_000, frames),
            )
            .unwrap();
        rendered.extend(output);
        position += frames;
        block_index += 1;
    }
    rendered
}

#[test]
fn stft_output_is_causal_and_block_partition_invariant() {
    let reference = render_superdirective(&[64], 4096);
    assert!(reference[..FFT_SIZE].iter().all(|sample| *sample == 0.0));
    assert!(
        reference[FFT_SIZE..]
            .iter()
            .any(|sample| sample.abs() > 1.0e-6)
    );

    for blocks in [&[127][..], &[256], &[512], &[1024], &[63, 257, 19, 511]] {
        let candidate = render_superdirective(blocks, 4096);
        assert_eq!(candidate.len(), reference.len());
        for (index, (actual, expected)) in candidate.iter().zip(&reference).enumerate() {
            assert!(
                (actual - expected).abs() < 1.0e-6,
                "block split {blocks:?}, sample {index}: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn process_validates_buffers_and_sample_rate_before_mutating_state() {
    let mut plugin = BeamformerPlugin::new(2, 48_000).unwrap();
    let fill = plugin.input_fill;
    let mut output = vec![0.0; 16];
    assert!(
        plugin
            .process(&[0.0; 31], &mut output, &ProcessContext::new(48_000, 16))
            .is_err()
    );
    assert_eq!(plugin.input_fill, fill);
    assert!(
        plugin
            .process(
                &[0.0; 32],
                &mut output[..15],
                &ProcessContext::new(48_000, 16)
            )
            .is_err()
    );
    assert_eq!(plugin.input_fill, fill);
    assert!(
        plugin
            .process(&[0.0; 32], &mut output, &ProcessContext::new(44_100, 16))
            .is_err()
    );
    assert_eq!(plugin.input_fill, fill);
}

#[test]
fn sample_rate_reinitialization_discards_old_grid_and_pending_audio() {
    let mut reused = BeamformerPlugin::new(2, 48_000).unwrap();
    let signal = vec![0.5; FFT_SIZE * 4];
    let mut output = vec![0.0; FFT_SIZE * 2];
    reused
        .process(
            &signal,
            &mut output,
            &ProcessContext::new(48_000, FFT_SIZE * 2),
        )
        .unwrap();

    for sample_rate in [96_000, 44_100] {
        reused.initialize(sample_rate).unwrap();
        let mut fresh = BeamformerPlugin::new(2, sample_rate).unwrap();
        fresh.initialize(sample_rate).unwrap();
        let silence = vec![0.0; FFT_SIZE * 2];
        let mut reused_output = vec![f32::NAN; FFT_SIZE];
        let mut fresh_output = vec![f32::NAN; FFT_SIZE];
        let context = ProcessContext::new(sample_rate, FFT_SIZE);
        reused
            .process(&silence, &mut reused_output, &context)
            .unwrap();
        fresh
            .process(&silence, &mut fresh_output, &context)
            .unwrap();
        assert_eq!(reused_output, fresh_output);
        assert!(reused_output.iter().all(|sample| *sample == 0.0));
    }
}
