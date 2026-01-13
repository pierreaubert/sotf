pub mod core;
pub mod error;
pub mod formats;
pub mod stream;

// Re-export the main API
pub use core::{AudioDecoder, AudioSpec, DecodedAudio, create_decoder, probe_file};
pub use error::{AudioDecoderError, AudioDecoderResult};
pub use formats::AudioFormat;
pub use stream::{AudioStream, StreamConfig, StreamEvent, StreamPosition, StreamState};
