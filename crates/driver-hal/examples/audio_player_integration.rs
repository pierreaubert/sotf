//! Example: Integration with audio player
//!
//! This example demonstrates how src-audio would integrate with the HAL driver:
//! 1. Read audio from HAL input buffer (from macOS apps)
//! 2. Process through plugin chain (simulated here)
//! 3. Write back to HAL output buffer (loopback)
//! 4. Also output to physical device (not shown here, handled by src-audio)

use sotf_hal::{HalAudioHandle, HalInputReader, HalOutputWriter};
use std::time::Duration;

fn main() {
    env_logger::init();

    println!("=== HAL Driver + Audio Player Integration Example ===\n");

    // Initialize the global buffer (normally done by HAL driver on startup)
    sotf_hal::audio_buffer::init_global_buffer(
        500,   // 500ms buffer
        48000, // 48kHz
        2,     // stereo
    );

    println!("✅ Audio buffer initialized (48kHz, stereo, 500ms capacity)\n");

    // Create handles for audio I/O
    let mut input_reader = HalInputReader::new().expect("Failed to create input reader");
    let mut output_writer = HalOutputWriter::new().expect("Failed to create output writer");

    println!("✅ Created input reader and output writer\n");

    // Get configuration
    let config = input_reader.config();
    println!("📊 Buffer configuration:");
    println!("   Sample rate: {} Hz", config.sample_rate);
    println!("   Channels: {}", config.channels);
    println!();

    // Simulate audio player processing loop
    println!("🎵 Starting simulated audio processing loop...\n");

    simulate_hal_producing_audio(&mut input_reader);

    for iteration in 0..10 {
        println!("--- Iteration {} ---", iteration + 1);

        // Check input buffer status
        let available = input_reader.available();
        println!("  Input buffer: {} samples available", available);

        if available > 0 {
            // Read audio from HAL (from macOS apps)
            let frame_size = 512;
            let mut input_buffer = vec![0.0f32; frame_size];
            let read = input_reader.read(&mut input_buffer);

            println!("  📥 Read {} samples from HAL input", read);

            // Simulate processing (e.g., EQ, upmixer, etc.)
            let processed = simulate_processing(&input_buffer[..read]);

            // Write back to HAL (loopback)
            let written = output_writer.write(&processed);
            println!("  📤 Wrote {} samples to HAL output (loopback)", written);

            // In real implementation, also output to physical device here
            // (using cpal or existing playback thread)
        } else {
            println!("  ⏸️  No audio data available from HAL");
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    println!("\n✅ Example completed");
}

/// Simulate HAL driver receiving audio from macOS apps
fn simulate_hal_producing_audio(reader: &mut HalInputReader) {
    println!("🎤 Simulating macOS app sending audio to HAL device...");

    // In real scenario, Core Audio would call HAL driver's I/O callback
    // For this example, we manually write to the input buffer

    let buffer = sotf_hal::audio_buffer::get_global_buffer().expect("Failed to get global buffer");

    let mut producer = buffer.input_producer();

    // Generate some test audio (sine wave)
    let sample_rate = 48000.0;
    let frequency = 440.0; // A4 note
    let duration_samples = 2048;

    let mut samples = Vec::with_capacity(duration_samples);
    for i in 0..duration_samples {
        let t = i as f32 / sample_rate;
        let sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.3;
        samples.push(sample);
    }

    let written = producer.write(&samples);
    println!("  ✅ Wrote {} test samples to input buffer\n", written);
}

/// Simulate audio processing (EQ, upmixer, etc.)
fn simulate_processing(input: &[f32]) -> Vec<f32> {
    // In real implementation, this would be:
    // - Plugin chain processing (EQ, compressor, limiter, etc.)
    // - Upmixing (stereo → 5.1)
    // - Loudness compensation
    // - Any other effects

    // For now, just apply a simple gain
    let gain = 0.8;
    input.iter().map(|&s| s * gain).collect()
}

/// Example using the combined handle
#[allow(dead_code)]
fn example_with_combined_handle() {
    sotf_hal::audio_buffer::init_global_buffer(500, 48000, 2);

    let mut handle = HalAudioHandle::new().expect("Failed to create audio handle");

    let mut input_buffer = vec![0.0f32; 512];
    let mut output_buffer = vec![0.0f32; 512];

    loop {
        // Read from HAL
        let read = handle.read_input(&mut input_buffer);

        if read > 0 {
            // Process audio
            for i in 0..read {
                output_buffer[i] = input_buffer[i] * 0.8; // Simple gain
            }

            // Write back to HAL (loopback)
            handle.write_output(&output_buffer[..read]);

            // Check buffer stats
            let input_stats = handle.input_stats();
            let output_stats = handle.output_stats();

            println!(
                "Input: {} available, Output: {} available",
                input_stats.available, output_stats.available
            );
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}
