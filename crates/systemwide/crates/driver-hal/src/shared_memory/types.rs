use super::shared_audio_buffer::SharedAudioBuffer;

#[derive(Debug, Clone, Copy)]
pub(super) struct EncryptedRecordHeader {
    pub(super) sample_count: usize,
    pub(super) frame_counter: u64,
    pub(super) ciphertext_len: usize,
    pub(super) total_bytes: usize,
    pub(super) slot_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EncryptedRecordRead {
    Empty,
    InvalidHeader,
    OutputTooSmall { sample_count: usize },
    Corrupt { frame_counter: u64 },
    Read { sample_count: usize },
}

#[allow(
    clippy::too_many_arguments,
    reason = "internal hot-path helper: refactoring into a struct would not improve readability"
)]
pub(super) fn read_encrypted_with_staging(
    shared: &SharedAudioBuffer,
    output: &mut [f32],
    cipher: &crate::encryption::AudioCipher,
    encrypted_samples_buf: &mut Vec<f32>,
    ciphertext_buf: &mut Vec<u8>,
    decrypted_record_buf: &mut Vec<f32>,
    pending_decrypted_samples: &mut Vec<f32>,
    pending_sample_offset: &mut usize,
) -> usize {
    let channel_count = shared.channel_count() as usize;
    if channel_count == 0 {
        output.fill(0.0);
        return 0;
    }

    let mut copied_samples = 0;

    if *pending_sample_offset < pending_decrypted_samples.len() {
        let pending_available = pending_decrypted_samples.len() - *pending_sample_offset;
        let to_copy = pending_available.min(output.len());
        output[..to_copy].copy_from_slice(
            &pending_decrypted_samples[*pending_sample_offset..*pending_sample_offset + to_copy],
        );
        *pending_sample_offset += to_copy;
        copied_samples += to_copy;

        if *pending_sample_offset >= pending_decrypted_samples.len() {
            pending_decrypted_samples.clear();
            *pending_sample_offset = 0;
        }
    }

    while copied_samples < output.len() {
        match shared.read_next_encrypted_record_into(
            decrypted_record_buf,
            cipher,
            encrypted_samples_buf,
            ciphertext_buf,
        ) {
            EncryptedRecordRead::Read { sample_count } => {
                let remaining = output.len() - copied_samples;
                let to_copy = sample_count.min(remaining);
                output[copied_samples..copied_samples + to_copy]
                    .copy_from_slice(&decrypted_record_buf[..to_copy]);
                copied_samples += to_copy;

                if to_copy < sample_count {
                    let pending_count = sample_count - to_copy;
                    if pending_decrypted_samples.capacity() < pending_count {
                        output[copied_samples..].fill(0.0);
                        return copied_samples / channel_count;
                    }
                    pending_decrypted_samples.clear();
                    pending_decrypted_samples
                        .extend_from_slice(&decrypted_record_buf[to_copy..sample_count]);
                    *pending_sample_offset = 0;
                    break;
                }
            }
            EncryptedRecordRead::OutputTooSmall { sample_count } => {
                // Production readers reserve the protocol maximum in `new`.
                // Never grow here: a malformed record or hand-constructed
                // undersized reader must fail silent instead of allocating on
                // the audio thread.
                if decrypted_record_buf.capacity() < sample_count {
                    if copied_samples == 0 {
                        output.fill(0.0);
                    } else {
                        output[copied_samples..].fill(0.0);
                    }
                    return copied_samples / channel_count;
                }
                decrypted_record_buf.resize(sample_count, 0.0);
            }
            EncryptedRecordRead::Corrupt { .. } | EncryptedRecordRead::InvalidHeader => {
                if copied_samples == 0 {
                    output.fill(0.0);
                    return 0;
                }
                break;
            }
            EncryptedRecordRead::Empty => break,
        }
    }

    if copied_samples < output.len() {
        output[copied_samples..].fill(0.0);
    }

    copied_samples / channel_count
}
