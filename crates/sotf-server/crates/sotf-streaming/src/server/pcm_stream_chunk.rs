use super::pcm_stream_format::PcmStreamFormat;

#[derive(Clone, Debug)]
pub struct PcmStreamChunk {
    pub samples: Vec<f32>,
    pub num_frames: usize,
    pub format: PcmStreamFormat,
}

impl PcmStreamChunk {
    pub fn new(
        samples: Vec<f32>,
        num_frames: usize,
        channels: u16,
        sample_rate: u32,
    ) -> Result<Self, String> {
        let expected = num_frames
            .checked_mul(channels as usize)
            .ok_or_else(|| "PCM stream chunk frame/channel count overflowed".to_string())?;
        if samples.len() != expected {
            return Err(format!(
                "PCM stream chunk has {} samples, expected {} ({} frames * {} channels)",
                samples.len(),
                expected,
                num_frames,
                channels
            ));
        }
        Ok(Self {
            samples,
            num_frames,
            format: PcmStreamFormat::new(sample_rate, channels),
        })
    }
}
