//! Playback event store tests (from app/state/playback_events.rs)

use sotf_audio_player_gpui::{MAX_EVENTS, PlaybackEvent, PlaybackEventStore, TrackChangeTrigger};
use std::path::PathBuf;

#[test]
fn test_event_recording() {
    let mut store = PlaybackEventStore::new();

    store.record_event(PlaybackEvent::Started {
        queue_index: 0,
        track_path: Some(PathBuf::from("/music/track.flac")),
    });
    store.record_event(PlaybackEvent::VolumeChanged { from: 0.7, to: 0.5 });
    store.record_event(PlaybackEvent::Paused);

    assert_eq!(store.len(), 3);
}

#[test]
fn test_replay() {
    let mut store = PlaybackEventStore::new();

    store.record_event(PlaybackEvent::Started {
        queue_index: 0,
        track_path: None,
    });
    store.record_event(PlaybackEvent::VolumeChanged { from: 0.7, to: 0.5 });
    store.record_event(PlaybackEvent::Paused);
    store.record_event(PlaybackEvent::Resumed);

    let snapshot = store.current_snapshot();
    assert!(snapshot.is_playing);
    assert_eq!(snapshot.queue_index, Some(0));
    assert_eq!(snapshot.volume, 0.5);

    // Replay to before resume
    let snapshot_at_2 = store.replay_to(2);
    assert!(!snapshot_at_2.is_playing);
}

#[test]
fn test_max_events() {
    let mut store = PlaybackEventStore::new();

    for i in 0..1500 {
        store.record_event(PlaybackEvent::PositionUpdated { position: i as f64 });
        store.last_position_update = None;
    }

    assert!(store.len() <= MAX_EVENTS);
}

#[test]
fn test_summary() {
    let mut store = PlaybackEventStore::new();

    store.record_event(PlaybackEvent::Started {
        queue_index: 0,
        track_path: None,
    });
    store.record_event(PlaybackEvent::Paused);
    store.record_event(PlaybackEvent::Resumed);
    store.record_event(PlaybackEvent::Seeked {
        from_position: 0.0,
        to_position: 30.0,
    });
    store.record_event(PlaybackEvent::TrackChanged {
        from_index: Some(0),
        to_index: 1,
        trigger: TrackChangeTrigger::NextTrack,
    });

    let summary = store.summary();
    assert_eq!(summary.play_count, 1);
    assert_eq!(summary.pause_count, 1);
    assert_eq!(summary.seek_count, 1);
    assert_eq!(summary.track_changes, 1);
}
