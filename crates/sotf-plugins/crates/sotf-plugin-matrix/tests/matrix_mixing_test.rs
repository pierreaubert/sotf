use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_matrix::MatrixPlugin;

/// Helper to check simple Matrix plugin usage
/// The plugin works with INTERLEAVED samples: [L0, R0, L1, R1, ...]
///
/// Mono Summing Scenario:
/// The user claims the mono mix is too loud.
/// If we sum L+R with 1.0 gain each:
/// L=1, R=1 (Correlated) -> Out=2 (+6dB)
/// L=1, R=0 (Uncorrelated) -> Out=1
///
/// Standard Downmix usually applies -3dB (0.707) or -6dB (0.5) to avoid clipping.
/// We verify that:
/// 1. [1.0, 1.0] matrix produces +6dB sum (proving the "louder" claim).
/// 2. [0.5, 0.5] matrix produces Unity sum (solving the claim for correlated files).

#[test]
fn test_mono_summing_loudness_correlated() {
    let input_channels = 2; // Stereo
    let output_channels = 1; // Mono

    // Matrix: [1.0, 1.0] -> Out = 1*L + 1*R
    let matrix_data = vec![1.0, 1.0];
    let mut plugin =
        MatrixPlugin::with_matrix(input_channels, output_channels, matrix_data).unwrap();

    let context = ProcessContext::new(44100, 64,);

    // Create INTERLEAVED input buffer for 64 frames
    // [L, R, L, R, ...]
    // Correlated: L=1.0, R=1.0
    let num_frames = 64;
    let mut input = Vec::with_capacity(num_frames * input_channels);
    for _ in 0..num_frames {
        input.push(1.0); // L
        input.push(1.0); // R
    }

    // Output buffer: Mono [M, M, M, ...]
    let mut output = vec![0.0; num_frames * output_channels];

    plugin.process(&input, &mut output, &context).unwrap();

    // Verify Output: 1.0 + 1.0 = 2.0
    for sample in output.iter() {
        assert!(
            (sample - 2.0_f32).abs() < 1e-6,
            "Simple sum 1+1 should be 2.0"
        );
    }

    // Now test normalized mixing (-6dB per channel)
    let matrix_normalized = vec![0.5, 0.5];
    plugin.set_matrix(matrix_normalized).unwrap();

    // Process enough frames for the one-pole gain smoother to converge
    // (5ms time constant at 44100Hz needs ~2400 samples to reach 1e-5 threshold)
    let settle_frames = 4096;
    let settle_input = vec![1.0_f32; settle_frames * input_channels];
    let mut settle_output = vec![0.0_f32; settle_frames * output_channels];
    let settle_context = ProcessContext::new(44100, settle_frames,);
    plugin
        .process(&settle_input, &mut settle_output, &settle_context)
        .unwrap();

    // Clear output and process one more block — smoother should have converged
    output.fill(0.0);
    plugin.process(&input, &mut output, &context).unwrap();

    // Verify Output: 0.5*1.0 + 0.5*1.0 = 1.0
    for sample in output.iter() {
        assert!(
            (sample - 1.0_f32).abs() < 1e-6,
            "Normalized sum 0.5+0.5 should be 1.0"
        );
    }
}

#[test]
fn test_mid_side_encoding_decoding() {
    // M/S Encoding Matrix:
    // Mid  = 1.0*L + 1.0*R
    // Side = 1.0*L - 1.0*R

    let input_channels = 2;
    let output_channels = 2;
    let ms_matrix = vec![
        1.0, 1.0, // Mid row
        1.0, -1.0, // Side row
    ];

    let mut plugin = MatrixPlugin::with_matrix(input_channels, output_channels, ms_matrix).unwrap();
    let context = ProcessContext::new(44100, 64,);

    let num_frames = 64;
    let mut output = vec![0.0; num_frames * output_channels];

    // Case 1: Correlated [1.0, 1.0] -> Mid=2.0, Side=0.0
    let mut input_corr = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        input_corr.push(1.0); // L
        input_corr.push(1.0); // R
    }

    plugin.process(&input_corr, &mut output, &context).unwrap();

    for i in 0..num_frames {
        let mid = output[i * 2];
        let side = output[i * 2 + 1];
        assert!((mid - 2.0_f32).abs() < 1e-6, "Mid should be 2.0");
        assert!((side - 0.0_f32).abs() < 1e-6, "Side should be 0.0");
    }

    // Case 2: Left Only [1.0, 0.0] -> Mid=1.0, Side=1.0
    let mut input_left = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        input_left.push(1.0); // L
        input_left.push(0.0); // R
    }

    plugin.process(&input_left, &mut output, &context).unwrap();

    for i in 0..num_frames {
        let mid = output[i * 2];
        let side = output[i * 2 + 1];
        assert!((mid - 1.0_f32).abs() < 1e-6, "Left Only: Mid should be 1.0");
        assert!(
            (side - 1.0_f32).abs() < 1e-6,
            "Left Only: Side should be 1.0"
        );
    }

    // Case 3: Right Only [0.0, 1.0] -> Mid=1.0, Side=-1.0
    let mut input_right = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        input_right.push(0.0); // L
        input_right.push(1.0); // R
    }

    plugin.process(&input_right, &mut output, &context).unwrap();

    for i in 0..num_frames {
        let mid = output[i * 2];
        let side = output[i * 2 + 1];
        assert!(
            (mid - 1.0_f32).abs() < 1e-6,
            "Right Only: Mid should be 1.0"
        );
        assert!(
            (side - -1.0_f32).abs() < 1e-6,
            "Right Only: Side should be -1.0"
        );
    }
}

#[test]
fn test_mid_side_roundtrip() {
    // Roundtrip M/S -> Stereo
    // Encode: [1, 1; 1, -1]
    // Decode: [0.5, 0.5; 0.5, -0.5]

    let mut encoder = MatrixPlugin::with_matrix(2, 2, vec![1.0, 1.0, 1.0, -1.0]).unwrap();

    let mut decoder = MatrixPlugin::with_matrix(2, 2, vec![0.5, 0.5, 0.5, -0.5]).unwrap();

    let context = ProcessContext::new(44100, 64,);
    let num_frames = 64;

    // Input: L=0.8, R=0.2
    let mut input = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        input.push(0.8);
        input.push(0.2);
    }

    let mut encoded = vec![0.0; num_frames * 2];
    let mut decoded = vec![0.0; num_frames * 2];

    encoder.process(&input, &mut encoded, &context).unwrap();
    decoder.process(&encoded, &mut decoded, &context).unwrap();

    for i in 0..num_frames {
        let l_out = decoded[i * 2];
        let r_out = decoded[i * 2 + 1];
        assert!((l_out - 0.8_f32).abs() < 1e-6, "L should match input");
        assert!((r_out - 0.2_f32).abs() < 1e-6, "R should match input");
    }
}

#[test]
fn test_mono_mix_down_laws() {
    // Validate different pan laws
    let context = ProcessContext::new(44100, 64,);
    let num_frames = 64;

    // -3dB Law Matrix: [0.707, 0.707]
    let frac_1_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
    let mut plugin_3db = MatrixPlugin::with_matrix(2, 1, vec![frac_1_sqrt2, frac_1_sqrt2]).unwrap();

    // -6dB Law Matrix: [0.5, 0.5]
    let mut plugin_6db = MatrixPlugin::with_matrix(2, 1, vec![0.5, 0.5]).unwrap();

    let mut output_3db = vec![0.0; num_frames];
    let mut output_6db = vec![0.0; num_frames];

    // 1. Correlated Input [1, 1]
    let mut input_corr = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        input_corr.push(1.0);
        input_corr.push(1.0);
    }

    plugin_3db
        .process(&input_corr, &mut output_3db, &context)
        .unwrap();
    plugin_6db
        .process(&input_corr, &mut output_6db, &context)
        .unwrap();

    // -3dB sum of 1+1 = 1.414 (+3dB)
    assert!((output_3db[0] - std::f32::consts::SQRT_2).abs() < 1e-5);
    // -6dB sum of 1+1 = 1.0 (0dB)
    assert!((output_6db[0] - 1.0_f32).abs() < 1e-5);

    // 2. Uncorrelated/Panned Input [1, 0]
    let mut input_panned = Vec::with_capacity(num_frames * 2);
    for _ in 0..num_frames {
        input_panned.push(1.0);
        input_panned.push(0.0);
    }

    plugin_3db
        .process(&input_panned, &mut output_3db, &context)
        .unwrap();
    plugin_6db
        .process(&input_panned, &mut output_6db, &context)
        .unwrap();

    // -3dB: 0.707 output
    assert!((output_3db[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    // -6dB: 0.5 output
    assert!((output_6db[0] - 0.5_f32).abs() < 1e-5);
}
