use super::misc::SAMPLE_RATE;
#[allow(unused_imports)]
use super::*;

#[allow(dead_code)]
pub(super) fn make_test_signal(num_frames: usize, channels: usize, freq_hz: f32) -> Vec<f32> {
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
pub(super) fn make_noisy_signal(
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
fn test_different_sample_rates() {
    for sr in [44100u32, 48000, 96000] {
        let mut plugin = DenoiserPlugin::new(2, false);
        plugin.initialize(sr).unwrap();

        let num_frames = 2048;
        let freq = 1000.0_f32.min(sr as f32 * 0.4);
        let mut input = make_test_signal(num_frames, 2, freq);

        let context = ProcessContext::new(sr, num_frames);

        plugin.process_in_place(&mut input, &context).unwrap();
        assert!(
            input.iter().all(|sample| *sample == 0.0),
            "the first FFT-sized block is the reported startup latency"
        );

        let mut input = make_test_signal(num_frames, 2, freq);
        plugin.process_in_place(&mut input, &context).unwrap();

        let sum: f32 = input.iter().map(|x| x.abs()).sum();
        assert!(sum > 0.0, "Sample rate {} should produce output", sr);
    }
}
