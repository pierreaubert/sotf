pub(super) const SAMPLE_RATE: u32 = 48000;

pub(super) const BUFFER_SIZE: usize = 512;

pub(super) const CHANNELS: usize = 2;

/// Generate a test audio buffer for benchmarking
pub(super) fn generate_test_buffer(num_frames: usize, channels: usize) -> Vec<f32> {
    (0..num_frames * channels)
        .map(|i| {
            let t = i as f32 / (SAMPLE_RATE as f32 * channels as f32);
            (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
        })
        .collect()
}
