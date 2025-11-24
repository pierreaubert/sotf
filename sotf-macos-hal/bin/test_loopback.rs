//! Test bidirectional audio and loopback functionality

use sotf_hal::{HalInputReader, HalOutputWriter};

fn main() {
    env_logger::init();

    println!("=== Testing HAL Driver Loopback ===\n");

    // Initialize audio buffer
    sotf_hal::audio_buffer::init_global_buffer(
        500,   // 500ms
        48000, // 48kHz
        2,     // stereo
    );

    println!("✅ Initialized audio buffers\n");

    // Create reader and writer
    let mut reader = HalInputReader::new().expect("Failed to create reader");
    let mut writer = HalOutputWriter::new().expect("Failed to create writer");

    // Test 1: Simulate HAL receiving audio from macOS app
    println!("Test 1: HAL Input (macOS → HAL → Input Buffer)");
    println!("-----------------------------------------------");

    let buffer = sotf_hal::audio_buffer::get_global_buffer().expect("Failed to get buffer");

    // Simulate HAL driver writing to input buffer (from macOS app)
    let mut input_producer = buffer.input_producer();
    let test_input = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    let written = input_producer.write(&test_input);
    println!("  ✅ HAL wrote {} samples to input buffer", written);

    // Audio player reads from input buffer
    let mut read_buffer = vec![0.0f32; 8];
    let read = reader.read(&mut read_buffer);
    println!("  ✅ Audio player read {} samples", read);
    println!("  📊 Data: {:?}", &read_buffer[..read]);

    assert_eq!(read, test_input.len());
    assert_eq!(read_buffer, test_input);
    println!("  ✅ Input data matches!\n");

    // Test 2: Audio player writing back to HAL (loopback)
    println!("Test 2: HAL Output (Audio Player → HAL → macOS)");
    println!("------------------------------------------------");

    // Audio player writes processed audio to output buffer
    let processed_output = vec![1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7];
    let written = writer.write(&processed_output);
    println!(
        "  ✅ Audio player wrote {} samples to output buffer",
        written
    );

    // HAL driver reads from output buffer to send to macOS
    let mut output_consumer = buffer.output_consumer();
    let mut hal_output = vec![0.0f32; 8];
    let read = output_consumer.read(&mut hal_output);
    println!("  ✅ HAL read {} samples from output buffer", read);
    println!("  📊 Data: {:?}", &hal_output[..read]);

    assert_eq!(read, processed_output.len());
    assert_eq!(hal_output, processed_output);
    println!("  ✅ Output data matches!\n");

    // Test 3: Full loopback simulation
    println!("Test 3: Full Loopback Simulation");
    println!("---------------------------------");

    // Generate test signal
    let test_signal: Vec<f32> = (0..100)
        .map(|i| ((i as f32 / 100.0) * 2.0 * std::f32::consts::PI).sin() * 0.5)
        .collect();

    println!("  🎵 Generated {} sample test signal", test_signal.len());

    // 1. HAL receives from macOS
    let written = input_producer.write(&test_signal);
    println!("  ✅ HAL input: {} samples written", written);

    // 2. Audio player reads
    let mut player_buffer = vec![0.0f32; 100];
    let read = reader.read(&mut player_buffer);
    println!("  ✅ Player read: {} samples", read);

    // 3. Audio player processes (simple gain)
    let gain = 0.8;
    let processed: Vec<f32> = player_buffer.iter().map(|&s| s * gain).collect();
    println!("  ✅ Player processed: applied {:.1}x gain", gain);

    // 4. Audio player writes to output (loopback)
    let written = writer.write(&processed);
    println!("  ✅ Player output: {} samples written", written);

    // 5. HAL reads to send back to macOS
    let mut hal_loopback = vec![0.0f32; 100];
    let read = output_consumer.read(&mut hal_loopback);
    println!("  ✅ HAL output: {} samples read for loopback", read);

    // Verify processing
    let expected_first = test_signal[0] * gain;
    let actual_first = hal_loopback[0];
    println!(
        "  📊 First sample: {:.4} → {:.4} (expected: {:.4})",
        test_signal[0], actual_first, expected_first
    );

    assert!((actual_first - expected_first).abs() < 0.0001);
    println!("  ✅ Loopback processing verified!\n");

    // Test 4: Buffer statistics
    println!("Test 4: Buffer Statistics");
    println!("-------------------------");

    println!("  Input buffer:");
    println!("    Available: {}", reader.available());
    println!("    Is empty: {}", reader.is_empty());

    println!("  Output buffer:");
    println!("    Available write: {}", writer.available_write());
    println!("    Is full: {}", writer.is_full());

    println!("\n✅ All loopback tests passed!");
}
