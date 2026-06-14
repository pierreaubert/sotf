use super::consts::STREAM_DATA_SIZE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PcmStreamFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl PcmStreamFormat {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }
}

pub(super) fn build_wav_stream_header_f32(format: PcmStreamFormat) -> [u8; 44] {
    let bits_per_sample: u16 = 32;
    let block_align = format.channels * (bits_per_sample / 8);
    let byte_rate = format.sample_rate * u32::from(block_align);
    let riff_size = STREAM_DATA_SIZE.saturating_add(36);

    let mut header = [0u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&riff_size.to_le_bytes());
    header[8..12].copy_from_slice(b"WAVE");
    header[12..16].copy_from_slice(b"fmt ");
    header[16..20].copy_from_slice(&16u32.to_le_bytes());
    header[20..22].copy_from_slice(&3u16.to_le_bytes()); // IEEE float
    header[22..24].copy_from_slice(&format.channels.to_le_bytes());
    header[24..28].copy_from_slice(&format.sample_rate.to_le_bytes());
    header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
    header[32..34].copy_from_slice(&block_align.to_le_bytes());
    header[34..36].copy_from_slice(&bits_per_sample.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&STREAM_DATA_SIZE.to_le_bytes());
    header
}
