use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_downmix::DownmixPlugin;

/// Verify that a pure tone passing through the phase-coherent STFT path
/// has constant amplitude (no OLA flutter). This proves the sqrt(Hann)
/// window at 50% overlap satisfies COLA.
#[test]
fn test_pure_tone_no_ola_flutter() {
    let mut plugin = DownmixPlugin::new(2);
    plugin.initialize(48000).unwrap();

    // Enable phase coherence (activates STFT path)
    plugin
        .set_parameter(
            plugin.parameters()[4].id.clone(),
            sotf_host::parameters::ParameterValue::Bool(true),
        )
        .unwrap();

    let freq = 1000.0_f32;
    let sr = 48000.0_f32;
    let block_size = 512;
    let num_blocks = 20;

    let mut input = vec![0.0f32; block_size * 2];
    let mut output = vec![0.0f32; block_size * 2];

    // Warm-up: process several blocks to reach steady state
    for block in 0..num_blocks {
        for i in 0..block_size {
            let t = (block * block_size + i) as f32 / sr;
            let sample = (2.0 * std::f32::consts::PI * freq * t).sin();
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }
        let ctx = ProcessContext {
            num_frames: block_size,
            sample_rate: 48000,
        };
        plugin.process(&input, &mut output, &ctx).unwrap();
    }

    // Measure output amplitude over many blocks
    let mut amplitudes = Vec::new();
    for block in 0..50 {
        for i in 0..block_size {
            let t = ((num_blocks + block) * block_size + i) as f32 / sr;
            let sample = (2.0 * std::f32::consts::PI * freq * t).sin();
            input[i * 2] = sample;
            input[i * 2 + 1] = sample;
        }
        let ctx = ProcessContext {
            num_frames: block_size,
            sample_rate: 48000,
        };
        plugin.process(&input, &mut output, &ctx).unwrap();

        // Measure peak amplitude of left channel in this block
        let peak = output
            .iter()
            .step_by(2)
            .map(|&s| s.abs())
            .fold(0.0f32, f32::max);
        amplitudes.push(peak);
    }

    // With COLA satisfied, amplitude should be nearly constant.
    // Allow ±5% tolerance for numerical noise.
    let min_amp = amplitudes.iter().copied().fold(f32::INFINITY, f32::min);
    let max_amp = amplitudes.iter().copied().fold(0.0f32, f32::max);
    let ratio = min_amp / max_amp;
    assert!(
        ratio > 0.95,
        "Amplitude flutter detected: min={:.4}, max={:.4}, ratio={:.4}",
        min_amp,
        max_amp,
        ratio
    );
}
