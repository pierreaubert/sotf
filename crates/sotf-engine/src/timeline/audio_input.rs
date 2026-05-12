// ============================================================================
// AudioInput — Cross-platform audio input capture via cpal
// ============================================================================
//
// Opens a cpal input stream and feeds samples into a ring buffer.
// The recording system reads from the consumer end of the ring buffer.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Configuration for audio input.
#[derive(Debug, Clone)]
pub struct AudioInputConfig {
    /// Preferred sample rate (0 = use device default)
    pub sample_rate: u32,
    /// Number of channels to capture
    pub channels: usize,
    /// Ring buffer capacity in frames
    pub buffer_frames: usize,
}

impl Default for AudioInputConfig {
    fn default() -> Self {
        Self {
            sample_rate: 0,
            channels: 2,
            buffer_frames: 48000, // 1 second at 48kHz
        }
    }
}

/// Shared state between the cpal callback and the consumer.
pub struct AudioInputState {
    /// Total frames captured since start
    pub frames_captured: AtomicU64,
    /// Whether input is actively capturing
    pub active: AtomicBool,
    /// Number of buffer overruns (consumer too slow)
    pub overruns: AtomicU64,
}

impl AudioInputState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            frames_captured: AtomicU64::new(0),
            active: AtomicBool::new(false),
            overruns: AtomicU64::new(0),
        })
    }
}

/// Manages a cpal audio input stream with a ring buffer for consumption.
///
/// Usage:
/// 1. Create with `AudioInput::open(config)`
/// 2. Call `start()` to begin capturing
/// 3. Read samples from the `consumer` ring buffer
/// 4. Call `stop()` when done
///
/// The cpal callback runs on a separate OS thread and writes into the
/// producer end of the ring buffer. The consumer reads from the other end
/// without blocking the audio thread.
pub struct AudioInput {
    /// Ring buffer consumer — read captured audio from here
    pub consumer: rtrb::Consumer<f32>,
    /// Shared state (frame count, overruns)
    pub state: Arc<AudioInputState>,
    /// Actual sample rate of the input stream
    pub sample_rate: u32,
    /// Number of channels
    pub channels: usize,
    /// The cpal stream handle (kept alive to maintain the stream)
    _stream: Option<Box<dyn cpal::traits::StreamTrait>>,
}

impl AudioInput {
    /// Open an audio input stream on the default input device.
    ///
    /// Note: This requires the `cpal` feature and a real audio device.
    /// In tests or headless environments, use `AudioInput::dummy()` instead.
    #[cfg(not(target_os = "ios"))]
    pub fn open(config: AudioInputConfig) -> Result<Self, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let supported = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {e}"))?;

        let sample_rate = if config.sample_rate > 0 {
            config.sample_rate
        } else {
            supported.sample_rate()
        };
        let channels = config.channels;
        let buffer_capacity = config.buffer_frames * channels;

        let (mut producer, consumer) = rtrb::RingBuffer::new(buffer_capacity);
        let state = AudioInputState::new();
        let state_clone = state.clone();

        let stream_config = cpal::StreamConfig {
            channels: channels as u16,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !state_clone.active.load(Ordering::Relaxed) {
                        return;
                    }
                    // Only write complete frames to preserve interleaving
                    let available = producer.slots();
                    let full_frames = (available / channels).min(data.len() / channels);
                    let samples_to_write = full_frames * channels;
                    for &sample in &data[..samples_to_write] {
                        let _ = producer.push(sample);
                    }
                    if samples_to_write < data.len() {
                        state_clone.overruns.fetch_add(1, Ordering::Relaxed);
                    }
                    state_clone
                        .frames_captured
                        .fetch_add(full_frames as u64, Ordering::Relaxed);
                },
                |err| {
                    log::error!("[AudioInput] Stream error: {err}");
                },
                None,
            )
            .map_err(|e| format!("Failed to build input stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start stream: {e}"))?;

        Ok(Self {
            consumer,
            state,
            sample_rate,
            channels,
            _stream: Some(Box::new(stream)),
        })
    }

    /// Create a dummy AudioInput for testing (no actual hardware).
    /// Manually push samples via the returned producer.
    pub fn dummy(
        channels: usize,
        sample_rate: u32,
        buffer_frames: usize,
    ) -> (Self, rtrb::Producer<f32>) {
        let (producer, consumer) = rtrb::RingBuffer::new(buffer_frames * channels);
        let state = AudioInputState::new();
        (
            Self {
                consumer,
                state,
                sample_rate,
                channels,
                _stream: None,
            },
            producer,
        )
    }

    /// Start capturing audio.
    pub fn start(&self) {
        self.state.active.store(true, Ordering::Relaxed);
    }

    /// Stop capturing audio.
    pub fn stop(&self) {
        self.state.active.store(false, Ordering::Relaxed);
    }

    /// Read available samples into the provided buffer.
    /// Returns the number of samples read.
    pub fn read_available(&mut self, buf: &mut [f32]) -> usize {
        let available = self.consumer.slots();
        let to_read = available.min(buf.len());
        let chunk = self.consumer.read_chunk(to_read);
        match chunk {
            Ok(chunk) => {
                let (a, b) = chunk.as_slices();
                buf[..a.len()].copy_from_slice(a);
                buf[a.len()..a.len() + b.len()].copy_from_slice(b);
                let total = a.len() + b.len();
                chunk.commit_all();
                total
            }
            Err(_) => 0,
        }
    }

    /// Total frames captured since start.
    pub fn frames_captured(&self) -> u64 {
        self.state.frames_captured.load(Ordering::Relaxed)
    }

    /// Number of overruns (buffer full, samples dropped).
    pub fn overruns(&self) -> u64 {
        self.state.overruns.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dummy_input_read() {
        let (mut input, mut producer) = AudioInput::dummy(2, 48000, 1024);
        input.start();

        // Push some samples via the producer
        for i in 0..2048 {
            producer.push(i as f32 * 0.001).unwrap();
        }

        // Read them back
        let mut buf = vec![0.0f32; 4096];
        let read = input.read_available(&mut buf);
        assert_eq!(read, 2048);
        assert!((buf[0]).abs() < 1e-6);
        assert!((buf[1] - 0.001).abs() < 1e-6);
    }

    #[test]
    fn test_dummy_input_not_started() {
        let (mut input, _producer) = AudioInput::dummy(1, 48000, 1024);
        // Not started — state should reflect that
        assert!(!input.state.active.load(Ordering::Relaxed));
        input.start();
        assert!(input.state.active.load(Ordering::Relaxed));
        input.stop();
        assert!(!input.state.active.load(Ordering::Relaxed));
    }
}
