//! Utility functions for the audio HAL driver
//!
//! This module contains helper functions for Core Audio integration,
//! error handling, and common operations.

use crate::{AudioDriverError, Result};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use coreaudio_sys::*;

/// Convert an OSStatus code to a Rust Result
pub fn os_status_to_result(status: OSStatus) -> Result<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(AudioDriverError::CoreAudio(status).into())
    }
}

/// Convert an OSStatus to a human-readable string
pub fn os_status_to_string(status: OSStatus) -> &'static str {
    match status {
        0 => "No Error",
        _ => "Audio Error",
    }
}

/// Create an AudioStreamBasicDescription for a given format
pub fn create_asbd(
    sample_rate: f64,
    channels: u32,
    bits_per_channel: u32,
    is_float: bool,
) -> AudioStreamBasicDescription {
    let format_id = if is_float {
        kAudioFormatLinearPCM
    } else {
        kAudioFormatLinearPCM
    };

    let format_flags = if is_float {
        kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked | kAudioFormatFlagIsNonInterleaved
    } else {
        kAudioFormatFlagIsSignedInteger
            | kAudioFormatFlagIsPacked
            | kAudioFormatFlagIsNonInterleaved
    };

    let bytes_per_frame = if is_float && bits_per_channel == 32 {
        4
    } else if !is_float && bits_per_channel == 16 {
        2
    } else {
        bits_per_channel / 8
    };

    AudioStreamBasicDescription {
        mSampleRate: sample_rate,
        mFormatID: format_id,
        mFormatFlags: format_flags,
        mBytesPerPacket: bytes_per_frame,
        mFramesPerPacket: 1,
        mBytesPerFrame: bytes_per_frame,
        mChannelsPerFrame: channels,
        mBitsPerChannel: bits_per_channel,
        mReserved: 0,
    }
}

/// Create a stereo float32 ASBD for the given sample rate
pub fn create_stereo_f32_asbd(sample_rate: f64) -> AudioStreamBasicDescription {
    create_asbd(sample_rate, 2, 32, true)
}

/// Generate unique AudioObjectID values
pub struct AudioObjectIDGenerator {
    next_id: AudioObjectID,
}

impl AudioObjectIDGenerator {
    pub fn new() -> Self {
        Self {
            // Start from a high number to avoid conflicts with system objects
            next_id: 1000,
        }
    }

    pub fn next_id(&mut self) -> AudioObjectID {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl Default for AudioObjectIDGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to copy data to a buffer safely
pub fn copy_to_buffer<T: Copy>(data: &[T], buffer: &mut [u8]) -> Result<u32> {
    let data_size = std::mem::size_of_val(data);

    if buffer.len() < data_size {
        return Err(AudioDriverError::Buffer(format!(
            "Buffer too small: {} < {}",
            buffer.len(),
            data_size
        ))
        .into());
    }

    unsafe {
        std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, buffer.as_mut_ptr(), data_size);
    }

    Ok(data_size as u32)
}

/// Helper to copy a single value to a buffer
pub fn copy_value_to_buffer<T: Copy>(value: &T, buffer: &mut [u8]) -> Result<u32> {
    let data_size = std::mem::size_of::<T>();

    if buffer.len() < data_size {
        return Err(AudioDriverError::Buffer(format!(
            "Buffer too small: {} < {}",
            buffer.len(),
            data_size
        ))
        .into());
    }

    unsafe {
        std::ptr::copy_nonoverlapping(
            value as *const T as *const u8,
            buffer.as_mut_ptr(),
            data_size,
        );
    }

    Ok(data_size as u32)
}

/// Helper to read a value from a buffer
pub fn read_value_from_buffer<T: Copy>(buffer: &[u8]) -> Result<T> {
    let data_size = std::mem::size_of::<T>();

    if buffer.len() < data_size {
        return Err(AudioDriverError::Buffer(format!(
            "Buffer too small: {} < {}",
            buffer.len(),
            data_size
        ))
        .into());
    }

    unsafe { Ok(std::ptr::read(buffer.as_ptr() as *const T)) }
}

/// Convert sample rate from Hz to a friendly string
pub fn sample_rate_to_string(sample_rate: f64) -> String {
    match sample_rate as u32 {
        44100 => "44.1 kHz".to_string(),
        48000 => "48 kHz".to_string(),
        88200 => "88.2 kHz".to_string(),
        96000 => "96 kHz".to_string(),
        176400 => "176.4 kHz".to_string(),
        192000 => "192 kHz".to_string(),
        _ => format!("{} Hz", sample_rate),
    }
}

/// Thread-safe atomic counter for generating unique IDs
use std::sync::atomic::{AtomicU32, Ordering};

static ID_COUNTER: AtomicU32 = AtomicU32::new(1000);

/// Generate a unique ID atomically
pub fn generate_unique_id() -> u32 {
    ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Check if an audio format is supported
pub fn is_format_supported(asbd: &AudioStreamBasicDescription) -> bool {
    // Check for supported sample rates
    let sample_rate = asbd.mSampleRate as u32;
    let supported_rates = [44100, 48000, 88200, 96000, 176400, 192000];

    if !supported_rates.contains(&sample_rate) {
        return false;
    }

    // Check format ID
    if asbd.mFormatID != kAudioFormatLinearPCM {
        return false;
    }

    // Check channel count (support 1-8 channels)
    if asbd.mChannelsPerFrame == 0 || asbd.mChannelsPerFrame > 8 {
        return false;
    }

    // Check bit depth
    match asbd.mBitsPerChannel {
        16 | 24 | 32 => true,
        _ => false,
    }
}

/// Convert dB to linear gain
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Convert linear gain to dB
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        -120.0 // Represent silence as -120dB
    } else {
        20.0 * linear.log10()
    }
}

/// Create a CFString from a Rust string and copy its bytes to a buffer
pub fn copy_cfstring_to_buffer(string: &str, buffer: &mut [u8]) -> Result<u32> {
    let cf_string = CFString::new(string);
    let cf_ref = cf_string.as_concrete_TypeRef();

    // Copy the CFStringRef pointer itself (not the string data)
    let ptr_size = std::mem::size_of::<CFStringRef>();

    if buffer.len() < ptr_size {
        return Err(AudioDriverError::Buffer(format!(
            "Buffer too small for CFStringRef: {} < {}",
            buffer.len(),
            ptr_size
        ))
        .into());
    }

    unsafe {
        std::ptr::copy_nonoverlapping(
            &cf_ref as *const CFStringRef as *const u8,
            buffer.as_mut_ptr(),
            ptr_size,
        );
    }

    // Important: We're giving ownership to Core Audio, so don't release it here
    // Core Audio will call CFRelease when done
    std::mem::forget(cf_string);

    Ok(ptr_size as u32)
}

/// Get the size needed to store a CFStringRef
pub fn cfstring_ref_size() -> u32 {
    std::mem::size_of::<CFStringRef>() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_status_conversion() {
        assert!(os_status_to_result(0).is_ok()); // kAudioHardwareNoError
        assert!(os_status_to_result(-1).is_err()); // kAudioHardwareUnspecifiedError
    }

    #[test]
    fn test_asbd_creation() {
        let asbd = create_stereo_f32_asbd(48000.0);
        assert_eq!(asbd.mSampleRate, 48000.0);
        assert_eq!(asbd.mChannelsPerFrame, 2);
        assert_eq!(asbd.mBitsPerChannel, 32);
        assert!(is_format_supported(&asbd));
    }

    #[test]
    fn test_id_generation() {
        let mut gen = AudioObjectIDGenerator::new();
        let id1 = gen.next_id();
        let id2 = gen.next_id();
        assert_ne!(id1, id2);
        assert!(id1 >= 1000);
        assert!(id2 > id1);
    }

    #[test]
    fn test_buffer_operations() {
        let value: u32 = 42;
        let mut buffer = vec![0u8; 8];

        let written = copy_value_to_buffer(&value, &mut buffer).unwrap();
        assert_eq!(written, 4);

        let read_value: u32 = read_value_from_buffer(&buffer).unwrap();
        assert_eq!(read_value, value);
    }

    #[test]
    fn test_db_conversion() {
        assert_eq!(db_to_linear(0.0), 1.0);
        assert_eq!(db_to_linear(-6.0), 0.5011872);
        assert_eq!(linear_to_db(1.0), 0.0);
        assert!((linear_to_db(0.5) - (-6.0)).abs() < 0.1);
    }
}
