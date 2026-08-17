use super::consts::pre_alloc_capacity_samples;
use super::encrypted::encrypted_record_slots;
use super::encrypted::encrypted_record_total_bytes;
use super::misc::fingerprints_equal;
use super::shared_audio_buffer::SharedAudioBuffer;
use super::shared_audio_buffer::load_initial_cipher;
use std::sync::atomic::{AtomicU64, Ordering};

/// Writer adapter for HAL output.
///
/// Same RT-safety contract as [`HalInputReader`]: cipher loaded once at
/// construction, audio path performs no filesystem I/O.
#[derive(Default)]
pub struct HalOutputWriter {
    pub(super) buffer: Option<SharedAudioBuffer>,
    pub(super) cipher: Option<crate::encryption::AudioCipher>,
    pub(super) ciphertext_buf: Vec<u8>,
    pub(super) encrypted_buf: Vec<f32>,
    key_mismatch_count: AtomicU64,
}

impl HalOutputWriter {
    /// Create a new HAL output writer.
    pub fn new() -> Option<Self> {
        match SharedAudioBuffer::open_default() {
            Ok(buffer) => {
                let pre_alloc = pre_alloc_capacity_samples();
                let ciphertext_bytes =
                    encrypted_record_total_bytes(pre_alloc).unwrap_or(pre_alloc * 8);
                let encrypted_slots = encrypted_record_slots(pre_alloc).unwrap_or(pre_alloc * 2);
                let cipher = load_initial_cipher(&buffer);
                Some(Self {
                    buffer: Some(buffer),
                    cipher,
                    ciphertext_buf: Vec::with_capacity(ciphertext_bytes),
                    encrypted_buf: Vec::with_capacity(encrypted_slots),
                    key_mismatch_count: AtomicU64::new(0),
                })
            }
            Err(_) => None,
        }
    }

    /// Re-open the daemon-owned mapping. This may perform filesystem I/O and
    /// must only be called from a control thread.
    pub fn reconnect(&mut self) -> std::io::Result<()> {
        let replacement = Self::new().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HAL output shared memory is unavailable",
            )
        })?;
        *self = replacement;
        Ok(())
    }

    /// Whether plaintext transport is active or the cached cipher matches the
    /// mapping's current key fingerprint.
    pub fn encryption_key_ready(&self) -> bool {
        let Some(buffer) = self.buffer.as_ref() else {
            return false;
        };
        if !buffer.is_encrypted() {
            return true;
        }
        self.cipher.as_ref().is_some_and(|cipher| {
            fingerprints_equal(cipher.fingerprint(), &buffer.key_fingerprint())
        })
    }

    /// Re-load the session key from disk and replace the cached cipher.
    pub fn reload_cipher(&mut self) -> std::io::Result<()> {
        if let Some(buf) = self.buffer.as_ref() {
            if !buf.is_encrypted() {
                self.cipher = None;
                return Ok(());
            }
            let key = crate::encryption::load_session_key()?;
            let cipher = crate::encryption::AudioCipher::new(&key);
            let expected = buf.key_fingerprint();
            if fingerprints_equal(cipher.fingerprint(), &expected) {
                self.cipher = Some(cipher);
                Ok(())
            } else {
                log::warn!(
                    "[HAL OUTPUT] Session-key reload mismatch: header={}, loaded={}",
                    crate::fingerprint_to_hex(&expected),
                    crate::fingerprint_to_hex(cipher.fingerprint())
                );
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Loaded session key fingerprint does not match shared memory header",
                ))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalOutputWriter not connected to shared memory",
            ))
        }
    }

    /// Check if connected to the HAL driver.
    pub fn is_connected(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.driver_ready())
            .unwrap_or(false)
    }

    /// Write audio samples to the HAL.
    pub fn write(&mut self, buffer: &[f32]) -> usize {
        let is_encrypted = self.buffer.as_ref().is_some_and(|b| b.is_encrypted());

        if is_encrypted {
            let header_fingerprint = self.buffer.as_ref().map(|b| b.key_fingerprint());
            let fingerprint_ok = matches!(
                (&self.cipher, header_fingerprint),
                (Some(c), Some(fp)) if fingerprints_equal(c.fingerprint(), &fp)
            );

            if !fingerprint_ok {
                self.key_mismatch_count.fetch_add(1, Ordering::Relaxed);
                return 0;
            }

            if let Some(cipher) = self.cipher.as_ref()
                && let Some(buf) = &mut self.buffer
            {
                return buf.write_audio_encrypted_into(
                    buffer,
                    cipher,
                    &mut self.ciphertext_buf,
                    &mut self.encrypted_buf,
                );
            }
            return 0;
        }

        if let Some(buf) = &mut self.buffer {
            buf.write_audio(buffer)
        } else {
            0
        }
    }

    /// Number of writes suppressed because the cached session key did not
    /// match the shared-memory fingerprint. Inspect from a control/diagnostic
    /// thread; the audio path only performs a relaxed atomic increment.
    pub fn key_mismatch_count(&self) -> u64 {
        self.key_mismatch_count.load(Ordering::Relaxed)
    }

    /// Number of complete plaintext frames currently queued for HAL playback.
    pub fn available_read_frames(&self) -> usize {
        self.buffer
            .as_ref()
            .map(SharedAudioBuffer::available_read_frames)
            .unwrap_or(0)
    }

    /// Drop queued audio while the transport is quiesced. This must not be
    /// called from an audio callback.
    pub fn flush_audio(&self) {
        if let Some(buffer) = self.buffer.as_ref() {
            buffer.flush_audio();
        }
    }

    /// Get the current HAL format as `(sample_rate, channel_count, buffer_frames)`.
    /// Returns `Err` when disconnected.
    pub fn current_format(&self) -> std::io::Result<(u32, u32, u32)> {
        match self.buffer.as_ref() {
            Some(buf) => Ok((buf.sample_rate(), buf.channel_count(), buf.buffer_frames())),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "HalOutputWriter not connected to shared memory",
            )),
        }
    }

    /// Sample rate (returns 0 when disconnected). Prefer
    /// [`HalOutputWriter::current_format`].
    pub fn sample_rate(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.sample_rate()).unwrap_or(0)
    }

    /// Channel count (returns 0 when disconnected). Prefer
    /// [`HalOutputWriter::current_format`].
    pub fn channel_count(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.channel_count()).unwrap_or(0)
    }

    /// Buffer frame size (returns 0 when disconnected). Prefer
    /// [`HalOutputWriter::current_format`].
    pub fn buffer_frames(&self) -> u32 {
        self.buffer.as_ref().map(|b| b.buffer_frames()).unwrap_or(0)
    }

    /// Set sample rate via the quiesced reconfiguration protocol.
    pub fn set_sample_rate(&mut self, sample_rate: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_sample_rate(sample_rate);
            true
        } else {
            false
        }
    }

    /// Set channel count via the quiesced reconfiguration protocol.
    pub fn set_channel_count(&mut self, channel_count: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_channel_count(channel_count);
            true
        } else {
            false
        }
    }

    /// Set buffer frame size via the quiesced reconfiguration protocol.
    pub fn set_buffer_frames(&mut self, buffer_frames: u32) -> bool {
        if let Some(buffer) = &mut self.buffer {
            buffer.set_buffer_frames(buffer_frames);
            true
        } else {
            false
        }
    }

    /// Set engine ready flag.
    pub fn set_engine_ready(&self, ready: bool) {
        if let Some(buffer) = &self.buffer {
            buffer.set_engine_ready(ready);
        }
    }

    /// Check if configuration has changed (signaled by Swift driver).
    pub fn config_changed(&self) -> bool {
        self.buffer
            .as_ref()
            .map(|b| b.config_changed())
            .unwrap_or(false)
    }

    /// Clear the configuration changed flag.
    pub fn clear_config_changed(&self) {
        if let Some(buffer) = &self.buffer {
            buffer.clear_config_changed();
        }
    }

    /// Signal configuration change to the Swift driver.
    pub fn set_config_changed(&self) {
        if let Some(buffer) = &self.buffer {
            buffer.set_config_changed();
        }
    }
}
