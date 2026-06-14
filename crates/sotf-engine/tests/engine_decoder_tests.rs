//! Decoder Thread Tests
//!
//! Unit tests for the decoder thread that handles audio file decoding and resampling.

#[path = "engine_decoder_tests/create.rs"]
mod create;
#[path = "engine_decoder_tests/misc.rs"]
mod misc;
#[cfg(test)]
#[path = "engine_decoder_tests/tests.rs"]
mod tests;
