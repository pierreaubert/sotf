// Tests for basic Matrix plugin

use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_matrix::MatrixPlugin;

#[test]
fn test_matrix_plugin() {
    // Stereo swap matrix
    // L_out = 0*L_in + 1*R_in
    // R_out = 1*L_in + 0*R_in

    // Let's create an identity matrix first
    let mut matrix = MatrixPlugin::new(2, 2);
    matrix.initialize(44100).unwrap();

    // By default it might be identity or zero?
    // Let's set it to swap
    // Set (row 0, col 1) = 1.0 -> Out L from In R
    matrix.set_gain(0, 1, 1.0).unwrap();
    // Set (row 1, col 0) = 1.0 -> Out R from In L
    matrix.set_gain(1, 0, 1.0).unwrap();
    // Clear diagonal
    matrix.set_gain(0, 0, 0.0).unwrap();
    matrix.set_gain(1, 1, 0.0).unwrap();

    // Let gain smoother converge (5ms time constant needs ~2400 samples)
    let settle_frames = 4096;
    let settle_input = vec![0.0_f32; settle_frames * 2];
    let mut settle_output = vec![0.0_f32; settle_frames * 2];
    let settle_context = ProcessContext {
        sample_rate: 44100,
        num_frames: settle_frames,
    };
    matrix
        .process(&settle_input, &mut settle_output, &settle_context)
        .unwrap();

    let input = vec![0.1, 0.8]; // L=0.1, R=0.8
    let mut output = vec![0.0; 2];

    let context = ProcessContext {
        sample_rate: 44100,
        num_frames: 1,
    };

    matrix.process(&input, &mut output, &context).unwrap();

    // Expect swapped
    assert!((output[0] - 0.8).abs() < 0.001); // L out = R in
    assert!((output[1] - 0.1).abs() < 0.001); // R out = L in
}
