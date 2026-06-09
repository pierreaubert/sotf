use crate::byte_precalc_decimator::{
    BytePrecalcDecimator, select_precalc_taps,
};
use crate::lm_resampler::{LMResampler, compute_decim_and_upsample};
use crate::{DsdRate, FilterType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DsdBitOrder {
    LsbFirst,
    MsbFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmOutputEncoding {
    Pcm24Le,
    Float32Le,
}

impl PcmOutputEncoding {
    pub fn bytes_per_sample(self) -> usize {
        match self {
            Self::Pcm24Le => 3,
            Self::Float32Le => size_of::<f32>(),
        }
    }

    fn write_sample(self, sample: f32, out: &mut Vec<u8>) {
        let sample = sample.clamp(-1.0, 1.0);
        match self {
            Self::Pcm24Le => {
                let scaled = if sample >= 0.0 {
                    sample * 8_388_607.0
                } else {
                    sample * 8_388_608.0
                };
                let value = scaled.round() as i32;
                out.extend_from_slice(&value.to_le_bytes()[..3]);
            }
            Self::Float32Le => {
                out.extend_from_slice(&sample.to_le_bytes())
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DsdPcmOptions {
    pub input_sample_rate: u32,
    pub output_sample_rate: u32,
    pub channels: usize,
    pub filter_type: FilterType,
    pub bit_order: DsdBitOrder,
}

impl DsdPcmOptions {
    pub fn sacd(channels: usize) -> Self {
        Self {
            input_sample_rate: 2_822_400,
            output_sample_rate: 176_400,
            channels,
            filter_type: FilterType::Equiripple,
            bit_order: DsdBitOrder::MsbFirst,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DsdPcmError {
    InvalidChannelCount,
    InvalidInputLength,
    UnsupportedInputRate(u32),
    UnsupportedConversion {
        input_sample_rate: u32,
        output_sample_rate: u32,
        filter_type: FilterType,
    },
}

impl std::fmt::Display for DsdPcmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidChannelCount => {
                f.write_str("invalid DSD channel count")
            }
            Self::InvalidInputLength => {
                f.write_str("DSD input is not channel aligned")
            }
            Self::UnsupportedInputRate(rate) => {
                write!(f, "unsupported DSD input rate: {rate}")
            }
            Self::UnsupportedConversion {
                input_sample_rate,
                output_sample_rate,
                filter_type,
            } => write!(
                f,
                "unsupported DSD-to-PCM conversion: {input_sample_rate} Hz to {output_sample_rate} Hz with {filter_type:?}"
            ),
        }
    }
}

impl std::error::Error for DsdPcmError {}

enum ChannelProcessor {
    Integer(BytePrecalcDecimator),
    Rational(Box<LMResampler>),
}

impl ChannelProcessor {
    fn process(&mut self, bytes: &[u8], out: &mut [f64]) -> usize {
        match self {
            Self::Integer(decimator) => {
                decimator.process_bytes(bytes, out)
            }
            Self::Rational(resampler) => {
                resampler.process_bytes_lm(bytes, out)
            }
        }
    }
}

pub struct DsdPcmConverter {
    opts: DsdPcmOptions,
    decim_ratio: i32,
    upsample_ratio: u32,
    processors: Vec<ChannelProcessor>,
    channel_bytes: Vec<Vec<u8>>,
    pcm_f64: Vec<f64>,
    pcm: Vec<Vec<f32>>,
}

impl DsdPcmConverter {
    pub fn new(opts: DsdPcmOptions) -> Result<Self, DsdPcmError> {
        if opts.channels == 0 {
            return Err(DsdPcmError::InvalidChannelCount);
        }

        let dsd_rate = dsd_rate_multiplier(opts.input_sample_rate)?;
        let (decim_ratio, upsample_ratio) =
            compute_decim_and_upsample(dsd_rate, opts.output_sample_rate);
        let processors = if upsample_ratio > 1 {
            (0..opts.channels)
                .map(|_| {
                    LMResampler::new(
                        upsample_ratio,
                        decim_ratio,
                        opts.output_sample_rate,
                    )
                    .map(|resampler| {
                        ChannelProcessor::Rational(Box::new(resampler))
                    })
                    .map_err(|_| {
                        DsdPcmError::UnsupportedConversion {
                            input_sample_rate: opts.input_sample_rate,
                            output_sample_rate: opts.output_sample_rate,
                            filter_type: opts.filter_type,
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let taps = select_precalc_taps(
                opts.filter_type,
                dsd_rate,
                decim_ratio,
            )
            .ok_or(DsdPcmError::UnsupportedConversion {
                input_sample_rate: opts.input_sample_rate,
                output_sample_rate: opts.output_sample_rate,
                filter_type: opts.filter_type,
            })?;
            (0..opts.channels)
                .map(|_| {
                    BytePrecalcDecimator::new(taps, decim_ratio as u32)
                        .map(ChannelProcessor::Integer)
                        .ok_or(DsdPcmError::UnsupportedConversion {
                            input_sample_rate: opts.input_sample_rate,
                            output_sample_rate: opts.output_sample_rate,
                            filter_type: opts.filter_type,
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(Self {
            opts,
            decim_ratio,
            upsample_ratio,
            processors,
            channel_bytes: vec![Vec::new(); opts.channels],
            pcm_f64: Vec::new(),
            pcm: vec![Vec::new(); opts.channels],
        })
    }

    pub fn input_sample_rate(&self) -> u32 {
        self.opts.input_sample_rate
    }

    pub fn output_sample_rate(&self) -> u32 {
        self.opts.output_sample_rate
    }

    pub fn channels(&self) -> usize {
        self.opts.channels
    }

    pub fn max_output_frames(
        &self,
        interleaved_input_bytes: usize,
    ) -> usize {
        let bytes_per_channel =
            interleaved_input_bytes / self.opts.channels;
        let bits_per_channel = bytes_per_channel * 8;
        let decim = self.decim_ratio.unsigned_abs() as usize;
        let frames = (bits_per_channel * self.upsample_ratio as usize)
            .div_ceil(decim);
        frames + usize::from(self.upsample_ratio > 1) * 16
    }

    pub fn convert_interleaved(
        &mut self,
        input: &[u8],
    ) -> Result<&[Vec<f32>], DsdPcmError> {
        if input.len() % self.opts.channels != 0 {
            return Err(DsdPcmError::InvalidInputLength);
        }

        let bytes_per_channel = input.len() / self.opts.channels;
        for bytes in &mut self.channel_bytes {
            bytes.clear();
            bytes.reserve(bytes_per_channel);
        }

        for frame in input.chunks_exact(self.opts.channels) {
            for (channel, byte) in frame.iter().copied().enumerate() {
                let byte = match self.opts.bit_order {
                    DsdBitOrder::LsbFirst => byte,
                    DsdBitOrder::MsbFirst => byte.reverse_bits(),
                };
                self.channel_bytes[channel].push(byte);
            }
        }

        let max_frames = self.max_output_frames(input.len());
        self.pcm_f64.resize(max_frames, 0.0);
        let mut produced_frames = None;
        let scale = self.upsample_ratio as f64;

        for channel in 0..self.opts.channels {
            self.pcm_f64.fill(0.0);
            let frames = self.processors[channel]
                .process(&self.channel_bytes[channel], &mut self.pcm_f64);
            if let Some(expected) = produced_frames {
                if frames != expected {
                    return Err(DsdPcmError::InvalidInputLength);
                }
            } else {
                produced_frames = Some(frames);
            }

            self.pcm[channel].resize(frames, 0.0);
            for (dst, src) in self.pcm[channel]
                .iter_mut()
                .zip(self.pcm_f64.iter().take(frames))
            {
                *dst = (*src * scale) as f32;
            }
        }

        Ok(&self.pcm)
    }

    pub fn convert_interleaved_to_bytes(
        &mut self,
        input: &[u8],
        encoding: PcmOutputEncoding,
        output: &mut Vec<u8>,
    ) -> Result<usize, DsdPcmError> {
        if input.len() % self.opts.channels != 0 {
            return Err(DsdPcmError::InvalidInputLength);
        }

        let max_frames = self.max_output_frames(input.len());
        self.pcm_f64.resize(max_frames, 0.0);
        let mut produced_frames = None;
        let scale = self.upsample_ratio as f64;
        let reverse_bits = self.opts.bit_order == DsdBitOrder::MsbFirst;
        let channels = self.opts.channels;

        for channel in 0..channels {
            self.pcm_f64.fill(0.0);
            let frames = match &mut self.processors[channel] {
                ChannelProcessor::Integer(decimator) => decimator
                    .process_interleaved_bytes(
                        input,
                        channel,
                        channels,
                        reverse_bits,
                        &mut self.pcm_f64,
                    ),
                ChannelProcessor::Rational(resampler) => resampler
                    .process_interleaved_bytes_lm(
                        input,
                        channel,
                        channels,
                        reverse_bits,
                        &mut self.pcm_f64,
                    ),
            };

            if let Some(expected) = produced_frames {
                if frames != expected {
                    return Err(DsdPcmError::InvalidInputLength);
                }
            } else {
                produced_frames = Some(frames);
            }

            self.pcm[channel].resize(frames, 0.0);
            for (dst, src) in self.pcm[channel]
                .iter_mut()
                .zip(self.pcm_f64.iter().take(frames))
            {
                *dst = (*src * scale) as f32;
            }
        }

        let frames = produced_frames.unwrap_or(0);
        output.clear();
        output.reserve(frames * channels * encoding.bytes_per_sample());
        for frame in 0..frames {
            for channel in 0..channels {
                encoding.write_sample(self.pcm[channel][frame], output);
            }
        }

        Ok(frames)
    }
}

fn dsd_rate_multiplier(sample_rate: u32) -> Result<i32, DsdPcmError> {
    let base = 2_822_400;
    if sample_rate % base != 0 {
        return Err(DsdPcmError::UnsupportedInputRate(sample_rate));
    }
    let multiplier = sample_rate / base;
    let rate = DsdRate::try_from(multiplier)
        .map_err(|_| DsdPcmError::UnsupportedInputRate(sample_rate))?;
    Ok(rate as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sacd_frame_converts_to_176k4_pcm_frames() {
        let mut converter =
            DsdPcmConverter::new(DsdPcmOptions::sacd(2)).unwrap();
        let input = vec![0x69; 2 * 4704];
        let pcm = converter.convert_interleaved(&input).unwrap();
        assert_eq!(pcm.len(), 2);
        assert_eq!(pcm[0].len(), 2352);
        assert_eq!(pcm[1].len(), 2352);
        assert!(pcm[0].iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn rejects_channel_misaligned_input() {
        let mut converter =
            DsdPcmConverter::new(DsdPcmOptions::sacd(2)).unwrap();
        assert_eq!(
            converter.convert_interleaved(&[0, 1, 2]).unwrap_err(),
            DsdPcmError::InvalidInputLength
        );
    }

    #[test]
    fn direct_float_bytes_match_planar_conversion() {
        let input = (0..2 * 4704)
            .map(|idx| (idx as u8).wrapping_mul(37))
            .collect::<Vec<_>>();
        let mut planar_converter =
            DsdPcmConverter::new(DsdPcmOptions::sacd(2)).unwrap();
        let planar = planar_converter.convert_interleaved(&input).unwrap();

        let mut direct_converter =
            DsdPcmConverter::new(DsdPcmOptions::sacd(2)).unwrap();
        let mut bytes = Vec::new();
        let frames = direct_converter
            .convert_interleaved_to_bytes(
                &input,
                PcmOutputEncoding::Float32Le,
                &mut bytes,
            )
            .unwrap();

        assert_eq!(frames, planar[0].len());
        assert_eq!(bytes.len(), frames * 2 * size_of::<f32>());
        for frame in 0..frames {
            for channel in 0..2 {
                let offset = (frame * 2 + channel) * size_of::<f32>();
                assert_eq!(
                    &bytes[offset..offset + size_of::<f32>()],
                    &planar[channel][frame].to_le_bytes()
                );
            }
        }
    }

    #[test]
    fn direct_pcm24_emits_interleaved_bytes() {
        let mut converter =
            DsdPcmConverter::new(DsdPcmOptions::sacd(2)).unwrap();
        let input = vec![0x69; 2 * 4704];
        let mut bytes = Vec::new();
        let frames = converter
            .convert_interleaved_to_bytes(
                &input,
                PcmOutputEncoding::Pcm24Le,
                &mut bytes,
            )
            .unwrap();

        assert_eq!(frames, 2352);
        assert_eq!(bytes.len(), frames * 2 * 3);
    }

    #[test]
    fn direct_float_bytes_match_rational_planar_conversion() {
        let input = (0..2 * 4704)
            .map(|idx| (idx as u8).wrapping_mul(19))
            .collect::<Vec<_>>();
        let opts = DsdPcmOptions {
            output_sample_rate: 96_000,
            ..DsdPcmOptions::sacd(2)
        };
        let mut planar_converter = DsdPcmConverter::new(opts).unwrap();
        let planar = planar_converter.convert_interleaved(&input).unwrap();

        let mut direct_converter = DsdPcmConverter::new(opts).unwrap();
        let mut bytes = Vec::new();
        let frames = direct_converter
            .convert_interleaved_to_bytes(
                &input,
                PcmOutputEncoding::Float32Le,
                &mut bytes,
            )
            .unwrap();

        assert_eq!(frames, planar[0].len());
        assert_eq!(bytes.len(), frames * 2 * size_of::<f32>());
        for frame in 0..frames {
            for channel in 0..2 {
                let offset = (frame * 2 + channel) * size_of::<f32>();
                assert_eq!(
                    &bytes[offset..offset + size_of::<f32>()],
                    &planar[channel][frame].to_le_bytes()
                );
            }
        }
    }
}
