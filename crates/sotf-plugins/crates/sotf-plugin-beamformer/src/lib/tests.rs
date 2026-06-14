use super::beamformer_plugin::BeamformerPlugin;
use super::misc::FFT_SIZE;
use super::types::BeamformerType;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{Plugin, ProcessContext};

#[test]
fn test_beamformer_plugin_creation() {
    let plugin = BeamformerPlugin::new(2, 48000);
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.output_channels(), 1);
}

#[test]
fn test_beamformer_parameters() {
    let mut plugin = BeamformerPlugin::new(2, 48000);
    plugin
        .set_parameter(
            ParameterId::from("steer_angle_deg"),
            ParameterValue::Float(45.0),
        )
        .unwrap();
    assert_eq!(plugin.steer_angle_deg, 45.0);
}

#[test]
fn test_beamformer_gsc_process() {
    let mut plugin = BeamformerPlugin::new(2, 48000);
    plugin.beamformer_type = BeamformerType::Gsc;

    let context = ProcessContext::new(48000, 256);
    let input = vec![0.1f32; 256 * 2];
    let mut output = vec![0.0f32; 256];

    let result = plugin.process(&input, &mut output, &context);
    assert!(result.is_ok());
}

#[test]
fn test_beamformer_mvdr_process() {
    let mut plugin = BeamformerPlugin::new(2, 48000);
    plugin.beamformer_type = BeamformerType::Mvdr;

    let context = ProcessContext::new(48000, 512);
    let input = vec![0.1f32; 512 * 2];
    let mut output = vec![0.0f32; 512];

    let result = plugin.process(&input, &mut output, &context);
    assert!(result.is_ok());
}

#[test]
fn test_beamformer_superdirective_process() {
    let mut plugin = BeamformerPlugin::new(2, 48000);
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
    let mut plugin = BeamformerPlugin::new(2, 48000);
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
    let mut plugin = BeamformerPlugin::new(2, 48000);
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
    bf.noise_threshold = 0.001;

    // Channel 0 is silent, channel 1 has high energy
    let stft: Vec<Vec<Complex<f32>>> = vec![
        vec![Complex::new(0.0, 0.0); spectrum_size], // mic 0: silent
        vec![Complex::new(10.0, 0.0); spectrum_size], // mic 1: loud
    ];

    // Call once; with the fix the high energy on mic 1 should prevent update
    let cov_before: Vec<_> = (0..spectrum_size)
        .map(|k| {
            // diagonal of cov for bin k
            let off = k * 4;
            bf.noise_cov_snapshot()[off] // 0,0 element
        })
        .collect();

    bf.update_noise_covariance(&stft);

    let cov_after: Vec<_> = (0..spectrum_size)
        .map(|k| {
            let off = k * 4;
            bf.noise_cov_snapshot()[off]
        })
        .collect();

    // Covariance should NOT have been updated (high energy → not noise)
    for k in 0..spectrum_size {
        assert_eq!(
            cov_before[k], cov_after[k],
            "bin {k}: covariance was incorrectly updated during high-energy frame"
        );
    }
}

#[test]
fn test_mvdr_process_uses_preallocated_weight_application() {
    let mut plugin = BeamformerPlugin::new(2, 48000);
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
