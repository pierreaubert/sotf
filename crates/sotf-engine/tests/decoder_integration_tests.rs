//! Decoder Integration Tests
//!
//! Tests for the audio decoder module including:
//! - File format support (WAV, FLAC, MP3, etc.)
//! - Audio specification parsing
//! - Decoding and sample conversion
//! - Seeking functionality
//! - Error handling

#[path = "decoder_integration_tests/create.rs"]
mod create;
#[cfg(test)]
#[path = "decoder_integration_tests/tests.rs"]
mod tests;
