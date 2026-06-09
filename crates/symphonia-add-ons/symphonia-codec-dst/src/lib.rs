#![forbid(unsafe_code)]

use dst_decoder::decoder::DstDecoder;
use rdsd2pcm::{DsdPcmConverter, DsdPcmOptions};
use symphonia_core::audio::sample::SampleFormat;
use symphonia_core::audio::{
    AsGenericAudioBufferRef, AudioBuffer, AudioMut, AudioSpec, GenericAudioBufferRef,
};
use symphonia_core::codecs::CodecInfo;
use symphonia_core::codecs::audio::{
    AudioCodecId, AudioCodecParameters, AudioDecoder, AudioDecoderOptions, FinalizeResult,
};
use symphonia_core::codecs::registry::{
    CodecRegistry, RegisterableAudioDecoder, SupportedAudioCodec,
};
use symphonia_core::common::FourCc;
use symphonia_core::errors::{
    Error as SymphoniaError, Result as SymphoniaResult, decode_error, unsupported_error,
};
use symphonia_core::packet::PacketRef;
use symphonia_core::support_audio_codec;
use thiserror::Error;

pub const CODEC_ID_DST: AudioCodecId = AudioCodecId::new(FourCc::new(*b"DST "));
pub const DEFAULT_DST_PCM_RATE: u32 = 176_400;

#[derive(Debug, Error)]
#[error("DST-to-DSD decode failed: {0}")]
pub struct DstDecodeError(String);

/// Decoder for DST frames into packed channel-interleaved DSD bytes.
pub struct DstDsdDecoder {
    inner: DstDecoder,
    channel_count: usize,
    sample_rate: usize,
}

impl DstDsdDecoder {
    pub fn new(channel_count: usize, sample_rate: usize) -> Result<Self, DstDecodeError> {
        let inner = DstDecoder::new(channel_count, sample_rate)
            .map_err(|err| DstDecodeError(err.to_string()))?;
        Ok(Self {
            inner,
            channel_count,
            sample_rate,
        })
    }

    pub fn channel_count(&self) -> usize {
        self.channel_count
    }

    pub fn sample_rate(&self) -> usize {
        self.sample_rate
    }

    pub fn dsd_frame_bytes(&self) -> usize {
        self.inner.dsd_frame_bytes()
    }

    pub fn decode_frame(
        &mut self,
        encoded: &[u8],
        decoded: &mut [u8],
    ) -> Result<usize, DstDecodeError> {
        self.inner
            .decode_frame(encoded, decoded)
            .map_err(|err| DstDecodeError(err.to_string()))
    }
}

pub fn register_decoders(registry: &mut CodecRegistry) {
    registry.register_audio_decoder::<DstAudioDecoder>();
}

/// Symphonia audio decoder for SACD DST packets.
///
/// The decoder first expands DST to packed DSD bytes, then converts those DSD
/// bytes to planar `f32` PCM using rdsd2pcm. Use [`DstDsdDecoder`] directly if
/// a caller needs the preserved DSD byte stream instead of PCM.
pub struct DstAudioDecoder {
    params: AudioCodecParameters,
    decoder: DstDsdDecoder,
    decoded: Vec<u8>,
    pcm_converter: DsdPcmConverter,
    buffer: AudioBuffer<f32>,
}

impl DstAudioDecoder {
    pub fn try_new(params: &AudioCodecParameters) -> SymphoniaResult<Self> {
        if params.codec != CODEC_ID_DST {
            return unsupported_error("dst: codec is not DST");
        }

        let channels = params
            .channels
            .clone()
            .ok_or(SymphoniaError::DecodeError("dst: missing channel count"))?;
        let channel_count = channels.count();
        if channel_count == 0 {
            return decode_error("dst: channel count is zero");
        }

        let sample_rate = params.sample_rate.unwrap_or(2_822_400) as usize;
        let decoder = DstDsdDecoder::new(channel_count, sample_rate)
            .map_err(|_| SymphoniaError::DecodeError("dst: decoder initialization failed"))?;
        let decoded = vec![0u8; decoder.dsd_frame_bytes()];
        let output_sample_rate = default_output_sample_rate(sample_rate as u32);
        let pcm_converter = DsdPcmConverter::new(DsdPcmOptions {
            input_sample_rate: sample_rate as u32,
            output_sample_rate,
            channels: channel_count,
            ..DsdPcmOptions::sacd(channel_count)
        })
        .map_err(|_| SymphoniaError::DecodeError("dst: DSD-to-PCM initialization failed"))?;
        let capacity = pcm_converter.max_output_frames(decoded.len());
        let buffer = AudioBuffer::new(AudioSpec::new(output_sample_rate, channels), capacity);

        let mut params = params.clone();
        params.sample_rate = Some(output_sample_rate);
        params.sample_format = Some(SampleFormat::F32);
        params.bits_per_sample = Some(32);
        params.bits_per_coded_sample = Some(1);
        params.max_frames_per_packet = Some(capacity as u64);
        params.frames_per_block = Some(capacity as u64);

        Ok(Self {
            params,
            decoder,
            decoded,
            pcm_converter,
            buffer,
        })
    }

    fn decode_inner(&mut self, packet: &PacketRef<'_>) -> SymphoniaResult<()> {
        let written = self
            .decoder
            .decode_frame(packet.data, &mut self.decoded)
            .map_err(|_| SymphoniaError::DecodeError("dst: frame decode failed"))?;
        if written % self.decoder.channel_count() != 0 {
            self.buffer.clear();
            return decode_error("dst: decoded frame is not channel aligned");
        }
        let pcm = self
            .pcm_converter
            .convert_interleaved(&self.decoded[..written])
            .map_err(|_| SymphoniaError::DecodeError("dst: DSD-to-PCM conversion failed"))?;

        let frames_per_channel = pcm.first().map(Vec::len).unwrap_or(0);
        self.buffer.grow_capacity(frames_per_channel);
        self.buffer.resize_uninit(frames_per_channel);

        for (channel, source) in pcm.iter().enumerate() {
            let Some(plane) = self.buffer.plane_mut(channel) else {
                self.buffer.clear();
                return decode_error("dst: decoded PCM channel plane is missing");
            };
            plane.copy_from_slice(source);
        }

        Ok(())
    }
}

impl AudioDecoder for DstAudioDecoder {
    fn reset(&mut self) {
        self.buffer.clear();
        if let Ok(decoder) =
            DstDsdDecoder::new(self.decoder.channel_count(), self.decoder.sample_rate())
        {
            self.decoder = decoder;
        }
        if let Ok(converter) = DsdPcmConverter::new(DsdPcmOptions {
            input_sample_rate: self.decoder.sample_rate() as u32,
            output_sample_rate: self.params.sample_rate.unwrap_or(DEFAULT_DST_PCM_RATE),
            channels: self.decoder.channel_count(),
            ..DsdPcmOptions::sacd(self.decoder.channel_count())
        }) {
            self.pcm_converter = converter;
        }
    }

    fn codec_info(&self) -> &CodecInfo {
        &Self::supported_codecs()
            .iter()
            .find(|desc| desc.id == self.params.codec)
            .unwrap()
            .info
    }

    fn codec_params(&self) -> &AudioCodecParameters {
        &self.params
    }

    fn decode_ref(&mut self, packet: &PacketRef<'_>) -> SymphoniaResult<GenericAudioBufferRef<'_>> {
        self.decode_inner(packet)?;
        Ok(self.buffer.as_generic_audio_buffer_ref())
    }

    fn finalize(&mut self) -> FinalizeResult {
        Default::default()
    }

    fn last_decoded(&self) -> GenericAudioBufferRef<'_> {
        self.buffer.as_generic_audio_buffer_ref()
    }
}

impl RegisterableAudioDecoder for DstAudioDecoder {
    fn try_registry_new(
        params: &AudioCodecParameters,
        _opts: &AudioDecoderOptions,
    ) -> SymphoniaResult<Box<dyn AudioDecoder>>
    where
        Self: Sized,
    {
        Ok(Box::new(Self::try_new(params)?))
    }

    fn supported_codecs() -> &'static [SupportedAudioCodec] {
        &[support_audio_codec!(
            CODEC_ID_DST,
            "dst",
            "Direct Stream Transfer (DST) to PCM"
        )]
    }
}

fn default_output_sample_rate(input_sample_rate: u32) -> u32 {
    match input_sample_rate {
        2_822_400 => DEFAULT_DST_PCM_RATE,
        5_644_800 => 176_400,
        11_289_600 => 176_400,
        22_579_200 => 352_800,
        _ => DEFAULT_DST_PCM_RATE,
    }
}

#[cfg(test)]
mod tests {
    use symphonia_core::audio::Channels;
    use symphonia_core::codecs::audio::AudioDecoderOptions;
    use symphonia_core::codecs::registry::CodecRegistry;

    use super::*;

    #[test]
    fn registry_constructs_dst_decoder() {
        let mut registry = CodecRegistry::new();
        register_decoders(&mut registry);

        let mut params = AudioCodecParameters::new();
        params
            .for_codec(CODEC_ID_DST)
            .with_sample_rate(2_822_400)
            .with_bits_per_sample(1)
            .with_bits_per_coded_sample(1)
            .with_channels(Channels::Discrete(2u16));

        let decoder = registry
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .unwrap();
        assert_eq!(decoder.codec_params().codec, CODEC_ID_DST);
        assert!(matches!(
            decoder.codec_params().sample_format,
            Some(SampleFormat::F32)
        ));
        assert_eq!(decoder.codec_params().sample_rate, Some(176_400));
        assert_eq!(decoder.codec_info().short_name, "dst");
    }
}
