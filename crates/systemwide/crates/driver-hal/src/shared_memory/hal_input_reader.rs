use super::consts::pre_alloc_capacity_samples;
use super::encrypted::encrypted_record_slots;
use super::encrypted::encrypted_record_total_bytes;
use super::misc::get_shared_memory_path;
use super::shared_audio_buffer::SharedAudioBuffer;
use super::shared_audio_buffer::load_initial_cipher;
use super::types::read_encrypted_with_staging;

/// Reader adapter for HAL input.
///
/// The cipher is loaded once at construction. If the daemon rotates the
/// session key while we're running, the read path detects the fingerprint
/// mismatch and returns silence (RT-safe) until [`HalInputReader::reload_cipher`]
/// is invoked from a non-RT control thread.
#[derive(Default)]
pub struct HalInputReader {
    pub(super) buffer: Option<SharedAudioBuffer>,
    pub(super) cipher: Option<crate::encryption::AudioCipher>,
    pub(super) encrypted_samples_buf: Vec<f32>,
    pub(super) ciphertext_buf: Vec<u8>,
    pub(super) decrypted_record_buf: Vec<f32>,
    pub(super) pending_decrypted_samples: Vec<f32>,
    pub(super) pending_sample_offset: usize,
}

impl HalInputReader {
    /// Create a new HAL input reader.
    ///
    /// Pre-allocates staging buffers sized for the worst-case HAL geometry
    /// so the audio path will never reallocate. If encryption is enabled at
    /// construction time, the session key is loaded once here (off the
    /// audio thread).
    pub fn new() -> Option<Self> {
        let path = get_shared_memory_path();
        log::info!("[HAL INPUT] Attempting to open SharedMemory at: {:?}", path);

        match SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                log::info!(
                    "[HAL INPUT] SharedMemory opened: sample_rate={}, buffer_frames={}, channels={}, driver_ready={}, active={}",
                    buffer.sample_rate(),
                    buffer.buffer_frames(),
                    buffer.channel_count(),
                    buffer.driver_ready(),
                    buffer.is_active()
                );

                let pre_alloc = pre_alloc_capacity_samples();
                let encrypted_slots = encrypted_record_slots(pre_alloc).unwrap_or(pre_alloc * 2);
                let ciphertext_bytes =
                    encrypted_record_total_bytes(pre_alloc).unwrap_or(pre_alloc * 8);

                let cipher = load_initial_cipher(&buffer);

                Some(Self {
                    buffer: Some(buffer),
                    cipher,
                    encrypted_samples_buf: Vec::with_capacity(encrypted_slots),
                    ciphertext_buf: Vec::with_capacity(ciphertext_bytes),
                    decrypted_record_buf: Vec::with_capacity(pre_alloc),
                    pending_decrypted_samples: Vec::with_capacity(pre_alloc),
                    pending_sample_offset: 0,
                })
            }
            Err(e) => {
                log::error!("[HAL INPUT] Failed to open SharedMemory: {}", e);
                None
            }
        }
    }

    /// Re-load the session key from disk and replace the cached cipher.
    ///
    /// Must be called from a non-RT thread. Audio reads return silence
    /// while the cached cipher's fingerprint disagrees with the header's
    /// — call this to recover.
    pub fn reload_cipher(&mut self) -> std::io::Result<()> {
        if let Some(buf) = self.buffer.as_ref() {
            let key = crate::encryption::load_session_key()?;
            let cipher = crate::encryption::AudioCipher::new(&key);
            if cipher.fingerprint() == &buf.key_fingerprint() {
                self.cipher = Some(cipher);
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Loaded session key fingerprint does not match shared memory header",
                ))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalInputReader not connected to shared memory",
            ))
        }
    }

    /// Returns true when encrypted shared memory is active but this reader has
    /// no matching cached cipher.
    pub fn needs_cipher_reload(&self) -> bool {
        let Some(buf) = self.buffer.as_ref() else {
            return false;
        };
        if !buf.is_encrypted() {
            return false;
        }
        let header_fingerprint = buf.key_fingerprint();
        !self
            .cipher
            .as_ref()
            .map(|cipher| cipher.fingerprint() == &header_fingerprint)
            .unwrap_or(false)
    }

    /// Check if connected to the HAL driver.
    pub fn is_connected(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.driver_ready())
            .unwrap_or(false)
    }

    /// Read audio samples from the HAL.
    ///
    /// Real-time safe: no filesystem I/O, no allocations, no per-call
    /// formatting. If encryption is on and the cached cipher's fingerprint
    /// no longer matches the header, returns silence.
    pub fn read(&mut self, buffer: &mut [f32]) -> usize {
        if let Some(buf) = &self.buffer {
            if buf.is_encrypted() {
                let header_fingerprint = buf.key_fingerprint();
                let fingerprint_ok = self
                    .cipher
                    .as_ref()
                    .map(|c| c.fingerprint() == &header_fingerprint)
                    .unwrap_or(false);

                if !fingerprint_ok {
                    // RT-safe: silence until a control thread calls
                    // `reload_cipher`. No disk I/O on the audio path.
                    buffer.fill(0.0);
                    return 0;
                }

                if let Some(cipher) = &self.cipher {
                    return read_encrypted_with_staging(
                        buf,
                        buffer,
                        cipher,
                        &mut self.encrypted_samples_buf,
                        &mut self.ciphertext_buf,
                        &mut self.decrypted_record_buf,
                        &mut self.pending_decrypted_samples,
                        &mut self.pending_sample_offset,
                    );
                }
                buffer.fill(0.0);
                return 0;
            }

            buf.read_audio(buffer)
        } else {
            0
        }
    }

    /// Get the current HAL format as `(sample_rate, channel_count, buffer_frames)`.
    ///
    /// Returns `Err` when the reader is not connected to shared memory.
    /// Prefer this over the legacy `sample_rate()`/`channel_count()`
    /// accessors which returned 0 (or formerly 48000/2) on disconnect.
    pub fn current_format(&self) -> std::io::Result<(u32, u32, u32)> {
        match self.buffer.as_ref() {
            Some(buf) => Ok((buf.sample_rate(), buf.channel_count(), buf.buffer_frames())),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalInputReader not connected to shared memory",
            )),
        }
    }

    /// Sample rate (returns 0 when disconnected). Prefer
    /// [`HalInputReader::current_format`].
    pub fn sample_rate(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(0)
    }

    /// Channel count (returns 0 when disconnected). Prefer
    /// [`HalInputReader::current_format`].
    pub fn channel_count(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.channel_count()).unwrap_or(0)
    }

    /// Get available frames to read.
    pub fn available_read_frames(&self) -> usize {
        let shared_frames = self
            .buffer
            .as_ref()
            .map(|b| b.available_read_frames())
            .unwrap_or(0);
        let pending_samples = self
            .pending_decrypted_samples
            .len()
            .saturating_sub(self.pending_sample_offset);
        let channels = self.channel_count() as usize;
        shared_frames + pending_samples.checked_div(channels).unwrap_or(0)
    }
}
