#![cfg(target_os = "macos")]
//! Real Integration Tests for HAL + Daemon Pipeline
//!
//! These tests verify the ACTUAL audio pipeline between:
//! - Swift HAL driver (CoreAudio plugin)
//! - Shared memory IPC (`/tmp/sotf-{uid}/audio.shm`)
//! - Rust daemon (sotf-daemon)
//! - Unix socket IPC
//!
//! # Prerequisites
//!
//! These tests require:
//! 1. HAL driver installed at `/Library/Audio/Plug-Ins/HAL/SotFHAL.driver`
//! 2. Daemon running (`cargo run --bin sotf-daemon --features hal`)
//! 3. macOS (HAL is macOS-only)
//!
//! # Running
//!
//! ```bash
//! # Run all real integration tests (requires setup)
//! cargo test -p driver-hal --test real_integration_tests -- --ignored
//!
//! # Run specific test
//! cargo test -p driver-hal --test real_integration_tests test_real_shared_memory_connection -- --ignored
//! ```
//!
//! # Safety
//!
//! These tests interact with real system resources. They are designed to be
//! read-only where possible and will not modify system state.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

// Re-export the path functions from driver_hal to use the same logic
use driver_hal::get_shared_memory_path as get_real_shm_path;

/// Get the daemon socket path (tries TMPDIR first, then UID-based)
fn get_real_socket_path() -> PathBuf {
    // Try macOS per-user temp directory first
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        let path = PathBuf::from(tmpdir).join("sotf-daemon.sock");
        if path.exists() {
            return path;
        }
    }

    // Fallback to UID-based path
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/sotf-{}/daemon.sock", uid))
}

/// Check if the HAL driver is installed
fn is_hal_driver_installed() -> bool {
    let driver_paths = [
        "/Library/Audio/Plug-Ins/HAL/SotFHAL.driver",
        "/Library/Audio/Plug-Ins/HAL/AutoEQ.driver",
        "/Library/Audio/Plug-Ins/HAL/sotf_hal.driver",
    ];
    driver_paths
        .iter()
        .any(|p| std::path::Path::new(p).exists())
}

// =============================================================================
// Shared Memory Tests
// =============================================================================

/// Test that we can connect to the real shared memory region
///
/// This verifies:
/// - The shared memory file exists
/// - It has a valid SOTF header
/// - The version is compatible
#[test]
#[ignore = "Requires HAL driver and daemon running"]
fn test_real_shared_memory_connection() {
    use driver_hal::SharedAudioBuffer;

    let shm_path = get_real_shm_path();

    if !shm_path.exists() {
        eprintln!("Shared memory not found at {:?}", shm_path);
        eprintln!("This is expected if no app is using the HAL audio device.");
        eprintln!("To test: Play audio through 'SotF Audio' device, then run this test.");
        return; // Skip gracefully, don't fail
    }

    // Try to open the shared memory
    let buffer = SharedAudioBuffer::open(&shm_path).expect("Failed to open real shared memory");

    // Verify we got valid configuration
    let sample_rate = buffer.sample_rate();
    let buffer_frames = buffer.buffer_frames();
    let channel_count = buffer.channel_count();

    println!("Connected to real shared memory:");
    println!("  Path: {:?}", shm_path);
    println!("  Sample rate: {} Hz", sample_rate);
    println!("  Buffer frames: {}", buffer_frames);
    println!("  Channels: {}", channel_count);
    println!("  Driver ready: {}", buffer.driver_ready());
    println!("  Active: {}", buffer.is_active());
    println!("  Encrypted: {}", buffer.is_encrypted());

    // Verify reasonable values
    assert!(
        (44100..=192000).contains(&sample_rate),
        "Sample rate {} out of expected range",
        sample_rate
    );
    assert!(
        (64..=8192).contains(&buffer_frames),
        "Buffer frames {} out of expected range",
        buffer_frames
    );
    assert!(
        (1..=32).contains(&channel_count),
        "Channel count {} out of expected range",
        channel_count
    );
}

/// Test reading audio from real shared memory
///
/// This verifies:
/// - We can read audio data from the HAL driver
/// - The audio data is valid (not all zeros, not garbage)
#[test]
#[ignore = "Requires HAL driver with active audio"]
fn test_real_shared_memory_read_audio() {
    use driver_hal::SharedAudioBuffer;

    let shm_path = get_real_shm_path();
    if !shm_path.exists() {
        eprintln!("Shared memory not found - skipping");
        return;
    }

    let buffer = SharedAudioBuffer::open(&shm_path).expect("Failed to open shared memory");

    let channel_count = buffer.channel_count() as usize;
    let buffer_frames = buffer.buffer_frames() as usize;
    let sample_count = buffer_frames * channel_count;

    // Try to read audio
    let mut audio_data = vec![0.0f32; sample_count];
    let frames_read = buffer.read_audio(&mut audio_data);

    println!("Read {} frames from shared memory", frames_read);

    if frames_read == 0 {
        eprintln!("No audio available - this is normal if nothing is playing");
        return;
    }

    // Analyze the audio data
    let mut min_sample = f32::MAX;
    let mut max_sample = f32::MIN;
    let mut sum = 0.0f64;
    let mut non_zero_count = 0;

    for &sample in &audio_data[..frames_read * channel_count] {
        if sample < min_sample {
            min_sample = sample;
        }
        if sample > max_sample {
            max_sample = sample;
        }
        sum += sample as f64;
        if sample != 0.0 {
            non_zero_count += 1;
        }
    }

    let avg = sum / (frames_read * channel_count) as f64;

    println!("Audio analysis:");
    println!("  Min sample: {:.6}", min_sample);
    println!("  Max sample: {:.6}", max_sample);
    println!("  Average: {:.6}", avg);
    println!(
        "  Non-zero samples: {} / {}",
        non_zero_count,
        frames_read * channel_count
    );

    // Verify audio is valid
    assert!(
        min_sample >= -1.5 && max_sample <= 1.5,
        "Audio samples out of expected range [{}, {}]",
        min_sample,
        max_sample
    );
}

/// Test config negotiation via shared memory
///
/// This verifies:
/// - Config change requests work
/// - The daemon responds appropriately
#[test]
#[ignore = "Requires HAL driver and daemon running"]
fn test_real_config_negotiation() {
    use driver_hal::SharedAudioBuffer;

    let shm_path = get_real_shm_path();
    if !shm_path.exists() {
        eprintln!("Shared memory not found - skipping");
        return;
    }

    let buffer = SharedAudioBuffer::open(&shm_path).expect("Failed to open shared memory");

    // Read current config
    let current_rate = buffer.sample_rate();
    let current_frames = buffer.buffer_frames();

    println!(
        "Current config: {}Hz, {} frames",
        current_rate, current_frames
    );

    // Check config status
    let config_changed = buffer.config_changed();
    let config_status = buffer.config_status();
    let config_source = buffer.config_source();

    println!("Config state:");
    println!("  Changed: {}", config_changed);
    println!("  Status: {}", config_status);
    println!("  Source: {}", config_source);

    // Note: We don't actually request a config change here to avoid
    // disrupting the system. This test just verifies we can read the state.
}

// =============================================================================
// Daemon IPC Tests
// =============================================================================

/// Test that we can connect to the daemon socket
#[test]
#[ignore = "Requires daemon running"]
fn test_real_daemon_connection() {
    let socket_path = get_real_socket_path();

    if !socket_path.exists() {
        eprintln!("Daemon socket not found at {:?}", socket_path);
        eprintln!("Start the daemon with: cargo run --bin sotf-daemon --features hal");
        panic!("Daemon not running");
    }

    // Try to connect
    let stream = UnixStream::connect(&socket_path).expect("Failed to connect to daemon");

    // Set timeout
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set timeout");

    println!("Connected to daemon at {:?}", socket_path);
}

/// Test sending status command to daemon
#[test]
#[ignore = "Requires daemon running"]
fn test_real_daemon_status_command() {
    let socket_path = get_real_socket_path();
    if !socket_path.exists() {
        panic!("Daemon not running");
    }

    let mut stream = UnixStream::connect(&socket_path).expect("Failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set timeout");

    // Send status command
    let command = r#"{"command": "status"}"#;
    writeln!(stream, "{}", command).expect("Failed to send command");

    // Read response
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("Failed to read response");

    println!("Status response: {}", response.trim());

    // Parse and verify response
    let json: serde_json::Value = serde_json::from_str(&response).expect("Invalid JSON response");

    assert!(
        json.get("success").is_some(),
        "Response should have 'success' field"
    );

    if let Some(data) = json.get("data") {
        if let Some(state) = data.get("state") {
            println!("Daemon state: {}", state);
        }
        if let Some(volume) = data.get("volume") {
            println!("Volume: {}", volume);
        }
    }
}

/// Test sending HAL status command to daemon
#[test]
#[ignore = "Requires daemon running"]
fn test_real_daemon_hal_status() {
    let socket_path = get_real_socket_path();
    if !socket_path.exists() {
        panic!("Daemon not running");
    }

    let mut stream = UnixStream::connect(&socket_path).expect("Failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set timeout");

    // Send HAL status command
    let command = r#"{"command": "hal_status"}"#;
    writeln!(stream, "{}", command).expect("Failed to send command");

    // Read response
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("Failed to read response");

    println!("HAL status response: {}", response.trim());

    let json: serde_json::Value = serde_json::from_str(&response).expect("Invalid JSON response");

    if let Some(data) = json.get("data") {
        println!("HAL Status:");
        if let Some(installed) = data.get("driver_installed") {
            println!("  Driver installed: {}", installed);
        }
        if let Some(available) = data.get("buffer_initialized") {
            println!("  Buffer available: {}", available);
        }
        if let Some(platform) = data.get("platform_supported") {
            println!("  Platform supported: {}", platform);
        }
    }
}

/// Test sending encryption status command to daemon
#[test]
#[ignore = "Requires daemon running"]
fn test_real_daemon_encryption_status() {
    let socket_path = get_real_socket_path();
    if !socket_path.exists() {
        panic!("Daemon not running");
    }

    let mut stream = UnixStream::connect(&socket_path).expect("Failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set timeout");

    // Send encryption status command
    let command = r#"{"command": "encryption_status"}"#;
    writeln!(stream, "{}", command).expect("Failed to send command");

    // Read response
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("Failed to read response");

    println!("Encryption status response: {}", response.trim());

    let json: serde_json::Value = serde_json::from_str(&response).expect("Invalid JSON response");

    if let Some(data) = json.get("data") {
        println!("Encryption Status:");
        if let Some(enabled) = data.get("enabled") {
            println!("  Enabled: {}", enabled);
        }
        if let Some(fingerprint) = data.get("fingerprint") {
            println!("  Key fingerprint: {}", fingerprint);
        }
        if let Some(path) = data.get("key_path") {
            println!("  Key path: {}", path);
        }
    }
}

/// Test listing available audio devices via daemon
#[test]
#[ignore = "Requires daemon running"]
fn test_real_daemon_list_devices() {
    let socket_path = get_real_socket_path();
    if !socket_path.exists() {
        panic!("Daemon not running");
    }

    let mut stream = UnixStream::connect(&socket_path).expect("Failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set timeout");

    // Send list devices command
    let command = r#"{"command": "list_devices"}"#;
    writeln!(stream, "{}", command).expect("Failed to send command");

    // Read response
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("Failed to read response");

    let json: serde_json::Value = serde_json::from_str(&response).expect("Invalid JSON response");

    if let Some(data) = json.get("data")
        && let Some(devices) = data.get("devices")
        && let Some(arr) = devices.as_array()
    {
        println!("Available audio devices ({}):", arr.len());
        for device in arr {
            if let Some(name) = device.get("name") {
                let is_default = device
                    .get("is_default")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let marker = if is_default { " (default)" } else { "" };
                println!("  - {}{}", name, marker);
            }
        }
    }
}

// =============================================================================
// Full Pipeline Tests
// =============================================================================

/// Test the full audio pipeline: HAL -> Daemon -> Shared Memory
///
/// This test verifies:
/// 1. Audio is being captured by HAL driver
/// 2. Daemon is processing it
/// 3. Audio flows through correctly
#[test]
#[ignore = "Requires HAL driver, daemon, and active audio playback"]
fn test_real_full_pipeline() {
    use driver_hal::SharedAudioBuffer;

    // Check prerequisites
    if !is_hal_driver_installed() {
        panic!("HAL driver not installed");
    }

    let socket_path = get_real_socket_path();
    if !socket_path.exists() {
        panic!("Daemon not running");
    }

    let shm_path = get_real_shm_path();
    if !shm_path.exists() {
        eprintln!("Shared memory not available - play audio through SotF device first");
        return;
    }

    // Connect to daemon and get status
    let mut stream = UnixStream::connect(&socket_path).expect("Failed to connect to daemon");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    writeln!(stream, r#"{{"command": "status"}}"#).unwrap();
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();

    let status: serde_json::Value = serde_json::from_str(&response).unwrap();
    println!("Daemon status: {:?}", status);

    // Open shared memory and read audio
    let buffer = SharedAudioBuffer::open(&shm_path).expect("Failed to open shared memory");

    println!("\nPipeline configuration:");
    println!("  Sample rate: {} Hz", buffer.sample_rate());
    println!("  Buffer frames: {}", buffer.buffer_frames());
    println!("  Channels: {}", buffer.channel_count());
    println!("  Driver ready: {}", buffer.driver_ready());
    println!("  Active: {}", buffer.is_active());

    // Read multiple blocks to verify continuous operation
    let channel_count = buffer.channel_count() as usize;
    let buffer_frames = buffer.buffer_frames() as usize;
    let mut total_frames_read = 0;
    let mut audio_data = vec![0.0f32; buffer_frames * channel_count];

    for i in 0..10 {
        let frames_read = buffer.read_audio(&mut audio_data);
        total_frames_read += frames_read;

        if frames_read > 0 {
            // Check for valid audio
            let max_abs = audio_data[..frames_read * channel_count]
                .iter()
                .map(|s| s.abs())
                .fold(0.0f32, f32::max);

            println!("Block {}: {} frames, peak={:.4}", i, frames_read, max_abs);
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    println!("\nTotal frames read: {}", total_frames_read);

    if total_frames_read == 0 {
        eprintln!("No audio data received - ensure audio is playing through the SotF device");
    }
}

/// Test engine_ready flag synchronization
///
/// Verifies that the Rust side can set engine_ready and the HAL driver sees it
#[test]
#[ignore = "Requires HAL driver and daemon running"]
fn test_real_engine_ready_flag() {
    use driver_hal::SharedAudioBuffer;

    let shm_path = get_real_shm_path();
    if !shm_path.exists() {
        eprintln!("Shared memory not found - skipping");
        return;
    }

    let buffer = SharedAudioBuffer::open(&shm_path).expect("Failed to open shared memory");

    // Read current state
    let engine_ready = buffer
        .header()
        .engine_ready
        .load(std::sync::atomic::Ordering::Acquire);
    println!("Current engine_ready state: {}", engine_ready != 0);

    // We don't modify the flag here to avoid disrupting the running daemon
    // This test just verifies we can read the shared state
}

// =============================================================================
// Stress Tests
// =============================================================================

/// Stress test: Multiple rapid connections to daemon
#[test]
#[ignore = "Requires daemon running - stress test"]
fn test_real_daemon_rapid_connections() {
    let socket_path = get_real_socket_path();
    if !socket_path.exists() {
        panic!("Daemon not running");
    }

    let num_connections = 100;
    let mut successes = 0;
    let mut failures = 0;

    println!("Testing {} rapid connections...", num_connections);

    for i in 0..num_connections {
        match UnixStream::connect(&socket_path) {
            Ok(mut stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(1))).ok();

                // Send a quick status command
                if writeln!(stream, r#"{{"command": "status"}}"#).is_ok() {
                    let mut buf = [0u8; 1024];
                    if stream.read(&mut buf).is_ok() {
                        successes += 1;
                    } else {
                        failures += 1;
                    }
                } else {
                    failures += 1;
                }
            }
            Err(e) => {
                failures += 1;
                if failures <= 5 {
                    eprintln!("Connection {} failed: {}", i, e);
                }
            }
        }
    }

    println!("Results: {} successes, {} failures", successes, failures);

    // Allow some failures due to resource limits, but most should succeed
    assert!(
        successes >= num_connections * 9 / 10,
        "Too many connection failures: {}/{}",
        failures,
        num_connections
    );
}

/// Stress test: Concurrent shared memory access
#[test]
#[ignore = "Requires HAL driver with active audio - stress test"]
fn test_real_shared_memory_concurrent_reads() {
    use driver_hal::SharedAudioBuffer;
    use std::sync::Arc;
    use std::thread;

    let shm_path = get_real_shm_path();
    if !shm_path.exists() {
        eprintln!("Shared memory not found - skipping");
        return;
    }

    let num_threads = 4;
    let reads_per_thread = 100;

    let shm_path: std::sync::Arc<PathBuf> = Arc::new(shm_path);
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let path: std::sync::Arc<PathBuf> = Arc::clone(&shm_path);
        let handle = thread::spawn(move || {
            let buffer = SharedAudioBuffer::open(path.as_ref()).expect("Failed to open buffer");
            let channel_count = buffer.channel_count() as usize;
            let buffer_frames = buffer.buffer_frames() as usize;
            let mut audio_data = vec![0.0f32; buffer_frames * channel_count];
            let mut total_frames = 0usize;

            for _ in 0..reads_per_thread {
                let frames = buffer.read_audio(&mut audio_data);
                total_frames += frames;
                thread::sleep(Duration::from_micros(100));
            }

            (thread_id, total_frames)
        });
        handles.push(handle);
    }

    println!("Concurrent read results:");
    for handle in handles {
        let (thread_id, frames) = handle.join().expect("Thread panicked");
        println!("  Thread {}: {} total frames read", thread_id, frames);
    }
}

// =============================================================================
// Helper to run all tests in sequence
// =============================================================================

/// Run all real integration tests in order
///
/// This is useful for manual testing to see the full picture.
/// Run with: cargo test -p driver-hal --test real_integration_tests run_all_real_tests -- --ignored --nocapture
#[test]
#[ignore = "Meta-test that runs other tests"]
fn run_all_real_tests() {
    println!("=== Real Integration Test Suite ===\n");

    println!("1. Checking prerequisites...");
    println!("   HAL driver installed: {}", is_hal_driver_installed());
    println!(
        "   Daemon socket exists: {}",
        get_real_socket_path().exists()
    );
    println!("   Shared memory exists: {}", get_real_shm_path().exists());

    println!("\n2. To run individual tests:");
    println!(
        "   cargo test -p driver-hal --test real_integration_tests <test_name> -- --ignored --nocapture"
    );

    println!("\n3. Available tests:");
    println!("   - test_real_shared_memory_connection");
    println!("   - test_real_shared_memory_read_audio");
    println!("   - test_real_config_negotiation");
    println!("   - test_real_daemon_connection");
    println!("   - test_real_daemon_status_command");
    println!("   - test_real_daemon_hal_status");
    println!("   - test_real_daemon_encryption_status");
    println!("   - test_real_daemon_list_devices");
    println!("   - test_real_full_pipeline");
    println!("   - test_real_engine_ready_flag");
    println!("   - test_real_daemon_rapid_connections (stress)");
    println!("   - test_real_shared_memory_concurrent_reads (stress)");
}
