pub mod core;
pub mod dsd;
pub mod error;
pub mod formats;
#[cfg(feature = "iamf")]
pub mod iamf;
pub mod pcm_reader;
pub mod source;
pub mod stream;

// Re-export the main API
pub use core::SourceMetadataReceiver;
pub use core::{
    AudioDecoder, AudioSpec, DecodedAudio, create_decoder, create_decoder_from_source,
    create_decoder_from_source_with_dsd_mode,
    create_decoder_from_source_with_dsd_mode_and_metadata, create_decoder_with_dsd_mode,
    probe_file,
};
pub use dsd::{DffPcmDecoder, DsfPcmDecoder};
pub use error::{AudioDecoderError, AudioDecoderResult};
pub use formats::{AudioFormat, DsdDecodeCapability};
pub use pcm_reader::PcmDecoder;
pub use source::{AudioSource, ServiceId};
pub use stream::{AudioStream, StreamConfig, StreamEvent, StreamPosition, StreamState};
