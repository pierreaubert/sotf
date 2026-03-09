//! Decoder Thread Tests
//!
//! Unit tests for the decoder thread that handles audio file decoding and resampling.

use hound::{WavSpec, WavWriter};
use sotf_audio::engine::{DecoderCommand, DecoderMessage, DecoderThread, ThreadEvent};
use std::path::PathBuf;
use std::sync::mpsc::{channel, sync_channel};
use std::time::Duration;
use tempfile::NamedTempFile;

/// Helper to create a DecoderThread with a recycle channel
fn create_decoder(
    message_tx: std::sync::mpsc::SyncSender<DecoderMessage>,
    event_tx: std::sync::mpsc::Sender<ThreadEvent>,
    sample_rate: u32,
    frame_size: usize,
) -> (DecoderThread, std::sync::mpsc::Sender<Vec<f32>>) {
    let (recycle_tx, recycle_rx) = channel();
    let decoder = DecoderThread::new(message_tx, event_tx, sample_rate, frame_size, recycle_rx)
        .expect("Failed to create decoder thread");
    (decoder, recycle_tx)
}

/// Helper to create a test WAV file
fn create_test_wav(duration_secs: f32, sample_rate: u32) -> NamedTempFile {
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let temp_file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
    let mut writer = WavWriter::create(temp_file.path(), spec).unwrap();

    // Generate a simple 440Hz tone
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin();
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude).unwrap(); // Left
        writer.write_sample(amplitude).unwrap(); // Right
    }

    writer.finalize().unwrap();
    temp_file
}

#[test]
fn test_decoder_thread_creation() {
    let (message_tx, _message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);
    let _ = decoder;

    // Shutdown is automatic via Drop
}

#[test]
fn test_decoder_load_and_decode() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, event_rx) = channel();

    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);

    // Create a test WAV file
    let temp_file = create_test_wav(0.1, 48000); // 100ms
    let path = temp_file.path().to_path_buf();

    // Send play command
    decoder.send_command(DecoderCommand::Play(path)).unwrap();

    // Wait for frames
    let mut frame_count = 0;
    let mut got_eos = false;

    let timeout = Duration::from_secs(5);
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Ok(msg) = message_rx.recv_timeout(Duration::from_millis(100)) {
            match msg {
                DecoderMessage::Frame(frame) => {
                    frame_count += 1;
                    assert_eq!(frame.sample_rate, 48000);
                    assert_eq!(frame.num_channels, 2);
                    assert!(frame.num_frames > 0);
                }
                DecoderMessage::EndOfStream => {
                    got_eos = true;
                    break;
                }
                DecoderMessage::Flush => {}
            }
        }

        // Also check for events
        while let Ok(event) = event_rx.try_recv() {
            if let ThreadEvent::DecoderEndOfStream = event {
                got_eos = true;
            }
        }

        if got_eos {
            break;
        }
    }

    assert!(frame_count > 0, "No frames received from decoder");
    assert!(got_eos, "Did not receive end of stream");
}

#[test]
fn test_decoder_pause_resume() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);

    // Create a longer test file
    let temp_file = create_test_wav(5.0, 48000); // 5 seconds
    let path = temp_file.path().to_path_buf();

    decoder.send_command(DecoderCommand::Play(path)).unwrap();

    // Receive some frames
    std::thread::sleep(Duration::from_millis(100));
    let frames_before_pause = message_rx.try_iter().count();

    // Pause
    decoder.send_command(DecoderCommand::Pause).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Clear any buffered frames
    let _ = message_rx.try_iter().count();

    // Wait a bit - should not receive new frames while paused
    std::thread::sleep(Duration::from_millis(200));
    let frames_during_pause = message_rx.try_iter().count();

    // Resume
    decoder.send_command(DecoderCommand::Resume).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let frames_after_resume = message_rx.try_iter().count();

    assert!(
        frames_before_pause > 0,
        "Should receive frames before pause"
    );
    assert_eq!(
        frames_during_pause, 0,
        "Should not receive frames while paused"
    );
    assert!(
        frames_after_resume > 0,
        "Should receive frames after resume"
    );
}

#[test]
fn test_decoder_seek() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);

    // Create a test file
    let temp_file = create_test_wav(2.0, 48000); // 2 seconds
    let path = temp_file.path().to_path_buf();

    decoder.send_command(DecoderCommand::Play(path)).unwrap();

    // Let it play a bit and drain initial frames
    std::thread::sleep(Duration::from_millis(100));
    let _ = message_rx.try_iter().count(); // Clear any frames

    // Seek to 1 second
    decoder.send_command(DecoderCommand::Seek(1.0)).unwrap();

    // Wait for a Flush message with proper timeout
    let timeout = Duration::from_secs(2);
    let start = std::time::Instant::now();
    let mut has_flush = false;

    while start.elapsed() < timeout && !has_flush {
        match message_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(DecoderMessage::Flush) => {
                has_flush = true;
            }
            Ok(_) => {
                // Ignore other messages (frames, etc.)
            }
            Err(_) => {
                // Timeout on this iteration, keep trying
            }
        }
    }

    assert!(has_flush, "Should receive flush message after seek");
}

#[test]
fn test_decoder_resampling() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    // Create decoder with 48kHz target
    let target_sr = 48000;
    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, target_sr, 512);

    // Create a 44.1kHz file (needs resampling)
    let source_sr = 44100;
    let temp_file = create_test_wav(0.1, source_sr);
    let path = temp_file.path().to_path_buf();

    decoder.send_command(DecoderCommand::Play(path)).unwrap();

    // Receive frames and verify they're at target sample rate
    std::thread::sleep(Duration::from_millis(200));

    let messages: Vec<_> = message_rx.try_iter().collect();
    let frames: Vec<_> = messages
        .iter()
        .filter_map(|msg| {
            if let DecoderMessage::Frame(frame) = msg {
                Some(frame)
            } else {
                None
            }
        })
        .collect();

    assert!(!frames.is_empty(), "Should receive frames after resampling");

    // All frames should be at target sample rate
    for frame in frames {
        assert_eq!(
            frame.sample_rate, target_sr,
            "Frame should be resampled to target rate"
        );
    }
}

#[test]
fn test_decoder_stop() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);

    let temp_file = create_test_wav(1.0, 48000);
    let path = temp_file.path().to_path_buf();

    decoder.send_command(DecoderCommand::Play(path)).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Stop
    decoder.send_command(DecoderCommand::Stop).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // Clear buffer
    let _ = message_rx.try_iter().count();

    // Should not receive more frames
    std::thread::sleep(Duration::from_millis(200));
    let frames_after_stop = message_rx.try_iter().count();

    assert_eq!(frames_after_stop, 0, "Should not receive frames after stop");
}

#[test]
fn test_decoder_invalid_file() {
    let (message_tx, _message_rx) = sync_channel(100);
    let (event_tx, event_rx) = channel();

    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);

    // Try to play non-existent file
    let invalid_path = PathBuf::from("/nonexistent/file.wav");
    decoder
        .send_command(DecoderCommand::Play(invalid_path))
        .unwrap();

    // Should receive an error event
    let timeout = Duration::from_secs(2);
    let start = std::time::Instant::now();
    let mut got_error = false;

    while start.elapsed() < timeout {
        if let Ok(event) = event_rx.recv_timeout(Duration::from_millis(100))
            && let ThreadEvent::DecoderError(_) = event {
                got_error = true;
                break;
            }
    }

    assert!(got_error, "Should receive error event for invalid file");
}

#[test]
fn test_decoder_shutdown() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    let (mut decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);

    let temp_file = create_test_wav(1.0, 48000);
    let path = temp_file.path().to_path_buf();

    decoder.send_command(DecoderCommand::Play(path)).unwrap();
    std::thread::sleep(Duration::from_millis(50));

    // Shutdown
    decoder.shutdown();

    // Thread should stop, channel should be disconnected
    std::thread::sleep(Duration::from_millis(100));

    // Trying to receive should fail or timeout
    // Drain any remaining frames
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(1);
    let mut got_eos_or_error = false;

    while start.elapsed() < timeout {
        match message_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(DecoderMessage::EndOfStream) => {
                got_eos_or_error = true;
                break;
            }
            Ok(DecoderMessage::Frame(_)) => {
                // Ignore frames
            }
            Ok(DecoderMessage::Flush) => {
                // Ignore flush
            }
            Err(_) => {
                // Channel disconnected or timeout
                got_eos_or_error = true;
                break;
            }
        }
    }

    assert!(got_eos_or_error, "Should receive EOS or channel disconnect");
}

#[test]
fn test_decoder_multiple_files() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, 512);

    // Play first file
    let temp_file1 = create_test_wav(0.1, 48000);
    decoder
        .send_command(DecoderCommand::Play(temp_file1.path().to_path_buf()))
        .unwrap();

    // Wait for end of stream
    let timeout = Duration::from_secs(2);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(DecoderMessage::EndOfStream) = message_rx.recv_timeout(Duration::from_millis(50))
        {
            break;
        }
    }

    // Play second file
    let temp_file2 = create_test_wav(0.1, 48000);
    decoder
        .send_command(DecoderCommand::Play(temp_file2.path().to_path_buf()))
        .unwrap();

    // Should receive frames from second file
    std::thread::sleep(Duration::from_millis(200));
    let messages: Vec<_> = message_rx.try_iter().collect();
    let has_frames = messages
        .iter()
        .any(|msg| matches!(msg, DecoderMessage::Frame(_)));

    assert!(has_frames, "Should receive frames from second file");
}

#[test]
fn test_decoder_frame_size_consistency() {
    let (message_tx, message_rx) = sync_channel(100);
    let (event_tx, _event_rx) = channel();

    let frame_size = 512;
    let (decoder, _recycle_tx) = create_decoder(message_tx, event_tx, 48000, frame_size);

    let temp_file = create_test_wav(0.5, 48000);
    decoder
        .send_command(DecoderCommand::Play(temp_file.path().to_path_buf()))
        .unwrap();

    std::thread::sleep(Duration::from_millis(300));

    let frames: Vec<_> = message_rx
        .try_iter()
        .filter_map(|msg| {
            if let DecoderMessage::Frame(frame) = msg {
                Some(frame)
            } else {
                None
            }
        })
        .collect();

    assert!(!frames.is_empty(), "Should receive frames");

    // Most frames should be the target frame size (last frame might be smaller)
    let expected_size_count = frames.iter().filter(|f| f.num_frames == frame_size).count();

    // At least 80% of frames should be the expected size
    let ratio = expected_size_count as f32 / frames.len() as f32;
    assert!(
        ratio > 0.8,
        "Most frames should match frame_size ({}%), got {}/{}",
        ratio * 100.0,
        expected_size_count,
        frames.len()
    );
}
