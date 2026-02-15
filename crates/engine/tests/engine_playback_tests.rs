//! Playback Thread Tests
//!
//! Unit tests for the playback thread that handles audio output to hardware.
//! All tests require BlackHole virtual audio device to avoid playing sound on real devices.

mod common;

use sotf_audio::engine::{
    AudioFrame, PlaybackCommand, PlaybackThread, ProcessingMessage, ThreadEvent,
};
use std::sync::mpsc::channel;
use std::time::Duration;

#[test]
fn test_playback_thread_creation() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    // Create playback thread with BlackHole device
    let result = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    );

    assert!(
        result.is_ok(),
        "Failed to create playback thread with BlackHole: {:?}",
        result.err()
    );
}

#[test]
fn test_playback_send_commands() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Test sending volume command
    assert!(
        playback
            .send_command(PlaybackCommand::SetVolume(0.5))
            .is_ok()
    );

    // Test sending mute command
    assert!(playback.send_command(PlaybackCommand::Mute(true)).is_ok());
    assert!(playback.send_command(PlaybackCommand::Mute(false)).is_ok());

    // Test sending stop command
    assert!(playback.send_command(PlaybackCommand::Stop).is_ok());
}

#[test]
fn test_playback_volume_commands() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Test various volume levels
    let volumes = [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0];

    for &vol in &volumes {
        let result = playback.send_command(PlaybackCommand::SetVolume(vol));
        assert!(result.is_ok(), "Failed to set volume to {}", vol);
    }
}

#[test]
fn test_playback_shutdown() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let mut playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    playback.shutdown();

    // After shutdown, commands should fail
    std::thread::sleep(Duration::from_millis(100));

    let result = playback.send_command(PlaybackCommand::SetVolume(0.5));
    assert!(result.is_err(), "Commands should fail after shutdown");
}

#[test]
fn test_playback_receives_frames() {
    let (message_tx, message_rx) = channel();
    let (event_tx, event_rx) = channel();

    let _playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Send some audio frames
    let frame = AudioFrame::silent(512, 2, 48000);

    for _ in 0..10 {
        message_tx
            .send(ProcessingMessage::Frame(frame.clone()))
            .ok();
    }

    std::thread::sleep(Duration::from_millis(200));

    // Check for underrun events (should not occur with sufficient frames)
    let events: Vec<_> = event_rx.try_iter().collect();
    let underruns = events
        .iter()
        .filter(|e| matches!(e, ThreadEvent::PlaybackUnderrun))
        .count();

    // With 10 frames queued, should not underrun immediately
    assert_eq!(underruns, 0, "Should not underrun with buffered frames");
}

/// Note: This test is skipped when using virtual audio devices like BlackHole
/// because virtual devices don't have real-time timing constraints and may not
/// trigger underrun events the same way real hardware does.
#[test]
#[ignore = "Underrun detection requires real audio hardware with timing constraints"]
fn test_playback_detects_underrun() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, event_rx) = channel();

    let _playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Don't send any frames - playback should underrun
    std::thread::sleep(Duration::from_millis(1000));

    // Check for events
    let events: Vec<_> = event_rx.try_iter().collect();

    // Check if we got any errors
    if let Some(ThreadEvent::ProcessingError(e)) = events
        .iter()
        .find(|e| matches!(e, ThreadEvent::ProcessingError(_)))
    {
        panic!("Audio thread error during test: {}", e);
    }

    let underruns = events
        .iter()
        .filter(|e| matches!(e, ThreadEvent::PlaybackUnderrun))
        .count();

    // Should detect at least one underrun when no data is provided
    assert!(
        underruns > 0,
        "Should detect underrun when no frames are provided (got 0 events)"
    );
}

#[test]
fn test_playback_handles_eos() {
    let (message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let _playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Send some frames then EOS
    for _ in 0..5 {
        let frame = AudioFrame::silent(512, 2, 48000);
        message_tx.send(ProcessingMessage::Frame(frame)).ok();
    }

    message_tx.send(ProcessingMessage::EndOfStream).ok();

    // Should handle gracefully
    std::thread::sleep(Duration::from_millis(200));
    // If we get here without panic, test passed
}

#[test]
fn test_playback_handles_flush() {
    let (message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let _playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Send frames, then flush
    for _ in 0..10 {
        let frame = AudioFrame::silent(512, 2, 48000);
        message_tx.send(ProcessingMessage::Frame(frame)).ok();
    }

    message_tx.send(ProcessingMessage::Flush).ok();

    std::thread::sleep(Duration::from_millis(100));

    // Send more frames after flush
    for _ in 0..5 {
        let frame = AudioFrame::silent(512, 2, 48000);
        message_tx.send(ProcessingMessage::Frame(frame)).ok();
    }

    std::thread::sleep(Duration::from_millis(200));
    // Should handle flush without panic
}

#[test]
fn test_playback_channel_update() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Try updating channel count
    let result = playback.send_command(PlaybackCommand::UpdateChannels(5));

    // Command should be accepted (even if hardware doesn't support it)
    assert!(result.is_ok(), "Should accept channel update command");

    std::thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_playback_rapid_volume_changes() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Rapidly change volume
    for i in 0..100 {
        let vol = (i as f32 / 100.0).sin().abs();
        playback.send_command(PlaybackCommand::SetVolume(vol)).ok();
    }

    std::thread::sleep(Duration::from_millis(100));
    // Should handle rapid changes without issue
}

#[test]
fn test_playback_rapid_mute_toggle() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Rapidly toggle mute
    for i in 0..50 {
        playback
            .send_command(PlaybackCommand::Mute(i % 2 == 0))
            .ok();
    }

    std::thread::sleep(Duration::from_millis(100));
    // Should handle rapid mute toggles
}

#[test]
fn test_playback_mixed_commands() {
    let (message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Mix audio frames and commands
    for i in 0..20 {
        if i % 3 == 0 {
            let vol = i as f32 / 20.0;
            playback.send_command(PlaybackCommand::SetVolume(vol)).ok();
        } else if i % 3 == 1 {
            playback
                .send_command(PlaybackCommand::Mute(i % 2 == 0))
                .ok();
        } else {
            let frame = AudioFrame::silent(512, 2, 48000);
            message_tx.send(ProcessingMessage::Frame(frame)).ok();
        }

        std::thread::sleep(Duration::from_millis(10));
    }

    std::thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_playback_different_sample_rates() {
    // Ensure BlackHole is available before running tests
    let _device = common::require_blackhole_device();

    let sample_rates = [44100, 48000, 88200, 96000];

    for &sr in &sample_rates {
        let (_message_tx, message_rx) = channel();
        let (event_tx, _event_rx) = channel();

        let result = PlaybackThread::new(
            message_rx,
            event_tx,
            sr,
            2,
            common::blackhole_device_option(),
            channel::<Vec<f32>>().0,
        );

        match result {
            Ok(_) => {
                // Success - BlackHole supports this rate
            }
            Err(e) => {
                // May fail if BlackHole doesn't support this rate
                assert!(
                    e.contains("audio")
                        || e.contains("device")
                        || e.contains("rate")
                        || e.contains("host"),
                    "Error should be audio-related for {} Hz: {}",
                    sr,
                    e
                );
            }
        }
    }
}

#[test]
fn test_playback_different_channel_counts() {
    // Ensure BlackHole is available before running tests
    let _device = common::require_blackhole_device();

    let channel_counts = [1, 2, 4, 5, 6, 8];

    for &channels in &channel_counts {
        let (_message_tx, message_rx) = channel();
        let (event_tx, _event_rx) = channel();

        let result = PlaybackThread::new(
            message_rx,
            event_tx,
            48000,
            channels,
            common::blackhole_device_option(),
            channel::<Vec<f32>>().0,
        );

        match result {
            Ok(_) => {
                // Success - BlackHole supports this channel count
            }
            Err(e) => {
                // May fail if BlackHole doesn't support this channel count
                assert!(
                    e.contains("audio")
                        || e.contains("device")
                        || e.contains("channel")
                        || e.contains("host"),
                    "Error should be audio-related for {} channels: {}",
                    channels,
                    e
                );
            }
        }
    }
}

#[test]
fn test_playback_drop_cleanup() {
    let (_message_tx, message_rx) = channel();
    let (event_tx, _event_rx) = channel();

    let playback = PlaybackThread::new(
        message_rx,
        event_tx,
        48000,
        2,
        common::blackhole_device_option(),
        channel::<Vec<f32>>().0,
    )
    .expect("Failed to create playback thread with BlackHole");

    // Let Drop handle cleanup
    drop(playback);

    std::thread::sleep(Duration::from_millis(100));
    // Should clean up without panic
}
