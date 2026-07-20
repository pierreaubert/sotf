// ============================================================================
// AudioSink trait - Abstraction for audio output destinations
// ============================================================================
//
// Allows the playback thread to output to cpal (hardware), PipeWire,
// AirPlay, Chromecast, or any other audio sink without knowing the details.

use std::sync::mpsc::Sender;

use super::ThreadEvent;
pub use crate::{SinkConfig, SinkOpenResult, SinkType};

/// Output destination for processed audio.
///
/// Implementations handle the final step: getting f32 PCM samples to hardware
/// or a network endpoint. The playback thread's main loop handles frame
/// management (channel conversion, flush/drain sequencing, diagnostics) and
/// delegates the actual output to this trait.
///
/// # Real-time safety
/// `write()` must not allocate or acquire contended locks. Pre-allocate
/// all buffers during `open()` / `reconfigure()`.
pub trait AudioSink: Send + 'static {
    /// Open the sink with the given configuration.
    fn open(
        &mut self,
        config: SinkConfig,
        event_tx: Sender<ThreadEvent>,
    ) -> Result<SinkOpenResult, String>;

    /// Write interleaved f32 samples to the sink's buffer.
    ///
    /// Returns the number of samples written. Returns 0 if the buffer is full
    /// (caller should sleep and retry). Must not block for extended periods.
    fn write(&mut self, data: &[f32]) -> Result<usize, String>;

    /// Number of sample slots available for writing without blocking.
    fn available_slots(&self) -> usize;

    /// Total buffer capacity in samples.
    fn capacity(&self) -> usize;

    /// Request a flush (discard buffered audio, e.g. for seek).
    fn flush(&mut self);

    /// Check if the flush has completed (buffer fully drained).
    fn is_flush_complete(&self) -> bool;

    /// Set playback volume (linear scale, typically 0.0 to 1.0+).
    fn set_volume(&mut self, volume: f32);

    /// Set mute state.
    fn set_muted(&mut self, muted: bool);

    /// Reconfigure the sink for a different sample rate or channel count.
    /// This may involve destroying and recreating the output stream.
    fn reconfigure(&mut self, config: SinkConfig) -> Result<SinkOpenResult, String>;

    /// Check if the audio callback has stalled (no samples consumed recently).
    /// Used to detect broken audio devices (e.g. HDMI monitors that accept
    /// the stream but stop calling the callback).
    fn is_stalled(&self) -> bool;

    /// Get the device name (for logging/diagnostics).
    fn device_name(&self) -> &str;

    /// Close the sink and release all resources.
    fn close(&mut self);
}
