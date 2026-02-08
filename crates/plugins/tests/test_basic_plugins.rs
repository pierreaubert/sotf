// Tests for basic processing plugins (Gain, Delay, Matrix)

use sotf_plugins::{
    DelayPlugin, GainPlugin, InPlacePlugin, InPlacePluginAdapter, MatrixPlugin, Plugin, PluginHost,
    ProcessContext,
};

#[test]
fn test_gain_plugin() {
    // Test +6dB gain (2x amplitude)
    let mut gain = GainPlugin::new(2, 6.0);
    gain.initialize(44100).unwrap();

    let mut buffer = vec![0.5; 100];
    let context = ProcessContext {
        sample_rate: 44100,
        num_frames: 50,
    };

    gain.process_in_place(&mut buffer, &context).unwrap();

    // Check first sample
    let expected = 0.5 * 10.0_f32.powf(6.0 / 20.0); // approx 1.0
    assert!((buffer[0] - expected).abs() < 0.001);
}

#[test]
fn test_delay_plugin() {
    let mut host = PluginHost::new(2, 44100);

    // 10ms delay, 0 feedback, 100% wet
    let delay = DelayPlugin::new(2, 10.0, 0.0, 1.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(delay)))
        .unwrap();

    // Input impulse
    let mut input = vec![0.0; 2000]; // enough for delay
    input[0] = 1.0; // Left impulse
    input[1] = 1.0; // Right impulse

    let mut output = vec![0.0; 2000];
    host.process(&input, &mut output).unwrap();

    // 10ms at 44100Hz = 441 samples
    let delay_samples = (10.0 * 44100.0 / 1000.0) as usize;
    let ch_idx = delay_samples * 2;

    // Check that initial output is silence (latency) or delay line filling
    // The delay plugin usually doesn't report latency, it effects the audio.
    // So output[0] should be 0.0 (if mix is 1.0)
    assert_eq!(output[0], 0.0);

    // Check delayed signal
    // Allow small interpolation error if fractional delay
    assert!(
        (output[ch_idx] - 1.0).abs() < 0.1,
        "Expected impulse at sample {}, got {}",
        delay_samples,
        output[ch_idx]
    );
}

#[test]
fn test_matrix_plugin() {
    // Stereo swap matrix
    // L_out = 0*L_in + 1*R_in
    // R_out = 1*L_in + 0*R_in

    // MatrixPlugin usually takes a flat list of gains
    // 2x2 matrix: [0, 1, 1, 0] ? Check implementation or assume identity

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
