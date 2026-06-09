#![forbid(unsafe_code)]

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

pub const CODEC_ID_DSD: AudioCodecId = AudioCodecId::new(FourCc::new(*b"DSD "));
pub const DEFAULT_DSD64_PCM_RATE: u32 = 176_400;

#[derive(Debug, Error)]
#[error("DSD-to-PCM decode failed: {0}")]
pub struct DsdDecodeError(String);

pub fn register_decoders(registry: &mut CodecRegistry) {
    registry.register_audio_decoder::<DsdPcmAudioDecoder>();
}

pub struct DsdPcmAudioDecoder {
    params: AudioCodecParameters,
    converter: DsdPcmConverter,
    buffer: AudioBuffer<f32>,
}

impl DsdPcmAudioDecoder {
    pub fn try_new(params: &AudioCodecParameters) -> SymphoniaResult<Self> {
        if params.codec != CODEC_ID_DSD {
            return unsupported_error("dsd: codec is not DSD");
        }

        let channels = params
            .channels
            .clone()
            .ok_or(SymphoniaError::DecodeError("dsd: missing channel count"))?;
        let channel_count = channels.count();
        if channel_count == 0 {
            return decode_error("dsd: channel count is zero");
        }

        let input_sample_rate = params.sample_rate.unwrap_or(2_822_400);
        let output_sample_rate = default_output_sample_rate(input_sample_rate);
        let converter = DsdPcmConverter::new(DsdPcmOptions {
            input_sample_rate,
            output_sample_rate,
            channels: channel_count,
            ..DsdPcmOptions::sacd(channel_count)
        })
        .map_err(|err| DsdDecodeError(err.to_string()))
        .map_err(|_| SymphoniaError::DecodeError("dsd: decoder initialization failed"))?;

        let capacity = converter.max_output_frames(
            usize::try_from(params.max_frames_per_packet.unwrap_or(4704)).unwrap_or(4704)
                * channel_count,
        );
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
            converter,
            buffer,
        })
    }

    fn decode_inner(&mut self, packet: &PacketRef<'_>) -> SymphoniaResult<()> {
        let pcm = self
            .converter
            .convert_interleaved(packet.data)
            .map_err(|_| SymphoniaError::DecodeError("dsd: packet decode failed"))?;
        let frames = pcm.first().map(Vec::len).unwrap_or(0);
        self.buffer.grow_capacity(frames);
        self.buffer.resize_uninit(frames);

        for (channel, source) in pcm.iter().enumerate() {
            let Some(plane) = self.buffer.plane_mut(channel) else {
                self.buffer.clear();
                return decode_error("dsd: decoded channel plane is missing");
            };
            plane.copy_from_slice(source);
        }

        Ok(())
    }
}

impl AudioDecoder for DsdPcmAudioDecoder {
    fn reset(&mut self) {
        self.buffer.clear();
        if let Ok(converter) = DsdPcmConverter::new(DsdPcmOptions {
            input_sample_rate: self.converter.input_sample_rate(),
            output_sample_rate: self.converter.output_sample_rate(),
            channels: self.converter.channels(),
            ..DsdPcmOptions::sacd(self.converter.channels())
        }) {
            self.converter = converter;
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

impl RegisterableAudioDecoder for DsdPcmAudioDecoder {
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
            CODEC_ID_DSD,
            "dsd",
            "Direct Stream Digital (DSD) to PCM"
        )]
    }
}

fn default_output_sample_rate(input_sample_rate: u32) -> u32 {
    match input_sample_rate {
        2_822_400 => DEFAULT_DSD64_PCM_RATE,
        5_644_800 => 176_400,
        11_289_600 => 176_400,
        22_579_200 => 352_800,
        _ => DEFAULT_DSD64_PCM_RATE,
    }
}

#[cfg(test)]
mod tests {
    use symphonia_core::audio::{Audio, Channels};
    use symphonia_core::codecs::audio::AudioDecoderOptions;
    use symphonia_core::codecs::registry::CodecRegistry;
    use symphonia_core::packet::Packet;
    use symphonia_core::units::{Duration, Timestamp};

    use super::*;

    #[test]
    fn registry_constructs_dsd_decoder() {
        let mut registry = CodecRegistry::new();
        register_decoders(&mut registry);

        let mut params = AudioCodecParameters::new();
        params
            .for_codec(CODEC_ID_DSD)
            .with_sample_rate(2_822_400)
            .with_bits_per_sample(1)
            .with_bits_per_coded_sample(1)
            .with_channels(Channels::Discrete(2u16))
            .with_max_frames_per_packet(4704);

        let decoder = registry
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .unwrap();
        assert_eq!(decoder.codec_params().codec, CODEC_ID_DSD);
        assert_eq!(decoder.codec_params().sample_rate, Some(176_400));
        assert!(matches!(
            decoder.codec_params().sample_format,
            Some(SampleFormat::F32)
        ));
    }

    #[test]
    fn decodes_sacd_frame_to_f32_pcm() {
        let mut params = AudioCodecParameters::new();
        params
            .for_codec(CODEC_ID_DSD)
            .with_sample_rate(2_822_400)
            .with_bits_per_sample(1)
            .with_bits_per_coded_sample(1)
            .with_channels(Channels::Discrete(2u16))
            .with_max_frames_per_packet(4704);
        let mut decoder = DsdPcmAudioDecoder::try_new(&params).unwrap();
        let data = vec![0x69; 2 * 4704];
        let packet = Packet::new(1, Timestamp::ZERO, Duration::new(1), data);
        let decoded = decoder.decode_ref(&packet.as_packet_ref()).unwrap();
        let GenericAudioBufferRef::F32(buffer) = decoded else {
            panic!("expected f32 buffer");
        };
        assert_eq!(buffer.spec().rate(), 176_400);
        assert_eq!(buffer.frames(), 2352);
        assert_eq!(buffer.spec().channels().count(), 2);
    }
}
