//! Common test utilities for state-behavior equivalence testing.
//!
//! This module provides the framework for testing that TUI and GPUI
//! implementations produce equivalent state changes for the same operations.

pub mod comparable_state;
pub mod gpui_adapter;
pub mod operations;
pub mod tui_adapter;

// Re-export main types
pub use comparable_state::{
    ChannelFilterId, ComparableState, InputModeId, PluginSnapshot, PluginTypeId, ScreenId,
    SortOrderId, StateDiff, ViewModeId, compare_states,
};
pub use gpui_adapter::GpuiAdapter;
pub use operations::{Operation, OperationSequence};
pub use tui_adapter::TuiAdapter;

use sotf_audio_player::Album;
use std::path::PathBuf;

/// Trait for adapting app instances to the common test interface
pub trait AppAdapter {
    /// Extract current state as ComparableState
    fn get_state(&self) -> ComparableState;

    /// Execute an operation and return the new state
    fn execute(&mut self, op: Operation) -> ComparableState {
        self.apply_operation(op);
        self.get_state()
    }

    /// Apply an operation without returning state
    fn apply_operation(&mut self, op: Operation);

    /// Load test albums into the library
    fn load_test_library(&mut self, albums: &[TestAlbum]);

    /// Reset to initial state
    fn reset(&mut self);
}

/// Test album data for seeding libraries
#[derive(Debug, Clone)]
pub struct TestAlbum {
    pub artist: String,
    pub album: String,
    pub year: u32,
    pub tracks: Vec<TestTrack>,
    pub channels: u32,
}

/// Test track data
#[derive(Debug, Clone)]
pub struct TestTrack {
    pub title: String,
    pub duration_secs: f64,
}

impl TestAlbum {
    pub fn new(artist: &str, album: &str, year: u32) -> Self {
        Self {
            artist: artist.to_string(),
            album: album.to_string(),
            year,
            tracks: vec![TestTrack {
                title: "Track 1".to_string(),
                duration_secs: 180.0,
            }],
            channels: 2,
        }
    }

    pub fn with_tracks(mut self, tracks: Vec<(&str, f64)>) -> Self {
        self.tracks = tracks
            .into_iter()
            .map(|(title, duration)| TestTrack {
                title: title.to_string(),
                duration_secs: duration,
            })
            .collect();
        self
    }

    pub fn with_channels(mut self, channels: u32) -> Self {
        self.channels = channels;
        self
    }

    /// Convert to sotf_audio_player::Album
    pub fn to_album(&self) -> Album {
        use sotf_audio_player::Track;

        let tracks: Vec<Track> = self
            .tracks
            .iter()
            .enumerate()
            .map(|(i, t)| Track {
                title: Some(t.title.clone()),
                track_number: Some((i + 1) as u32),
                duration_secs: Some(t.duration_secs as u64),
                path: PathBuf::from(format!(
                    "/test/{}/{}/{:02} - {}.flac",
                    self.artist,
                    self.album,
                    i + 1,
                    t.title
                )),
                channels: Some(self.channels),
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
            })
            .collect();

        Album {
            id: None,
            title: self.album.clone(),
            artist: self.artist.clone(),
            year: Some(self.year),
            tracks,
            album_art_path: None,
            play_count: 0,
        }
    }
}

/// Create a standard test library with diverse albums
pub fn create_test_library() -> Vec<TestAlbum> {
    vec![
        TestAlbum::new("Artist A", "Album One", 2020)
            .with_tracks(vec![
                ("Opening", 200.0),
                ("Main Theme", 240.0),
                ("Finale", 300.0),
            ])
            .with_channels(2),
        TestAlbum::new("Artist A", "Album Two", 2021)
            .with_tracks(vec![
                ("Track 1", 180.0),
                ("Track 2", 200.0),
            ])
            .with_channels(2),
        TestAlbum::new("Artist B", "First Release", 2019)
            .with_tracks(vec![
                ("Intro", 60.0),
                ("Song", 240.0),
                ("Outro", 90.0),
            ])
            .with_channels(2),
        TestAlbum::new("Artist C", "Surround Album", 2022)
            .with_tracks(vec![
                ("Immersive", 300.0),
            ])
            .with_channels(6), // 5.1 surround
        TestAlbum::new("The Beatles", "Abbey Road", 1969)
            .with_tracks(vec![
                ("Come Together", 259.0),
                ("Something", 182.0),
                ("Here Comes The Sun", 185.0),
            ])
            .with_channels(2),
    ]
}

/// Test harness that wraps an AppAdapter for easier testing
pub struct TestHarness<A: AppAdapter> {
    pub adapter: A,
    pub operation_log: Vec<(Operation, ComparableState)>,
}

impl<A: AppAdapter> TestHarness<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            operation_log: Vec::new(),
        }
    }

    /// Execute an operation and log the result
    pub fn execute(&mut self, op: Operation) -> &ComparableState {
        let state = self.adapter.execute(op.clone());
        self.operation_log.push((op, state));
        &self.operation_log.last().unwrap().1
    }

    /// Execute a sequence of operations
    pub fn execute_sequence(&mut self, sequence: &OperationSequence) {
        for op in &sequence.operations {
            self.execute(op.clone());
        }
    }

    /// Get current state
    pub fn state(&self) -> ComparableState {
        self.adapter.get_state()
    }

    /// Reset the adapter and clear logs
    pub fn reset(&mut self) {
        self.adapter.reset();
        self.operation_log.clear();
    }
}

/// Assert that two harnesses are in equivalent states
pub fn assert_equivalent<A1: AppAdapter, A2: AppAdapter>(
    harness1: &TestHarness<A1>,
    harness2: &TestHarness<A2>,
    context: &str,
) {
    let state1 = harness1.state();
    let state2 = harness2.state();
    let diffs = compare_states(&state1, &state2);

    if !diffs.is_empty() {
        let diff_str = diffs
            .iter()
            .map(|d| format!("  - {}", d))
            .collect::<Vec<_>>()
            .join("\n");
        panic!(
            "State mismatch after {context}:\n{diff_str}\n\nExpected (harness1):\n{state1:#?}\n\nActual (harness2):\n{state2:#?}"
        );
    }
}

/// Run a sequence on both harnesses and assert equivalence after each step
pub fn run_equivalence_test<A1: AppAdapter, A2: AppAdapter>(
    harness1: &mut TestHarness<A1>,
    harness2: &mut TestHarness<A2>,
    sequence: &OperationSequence,
) {
    for (i, op) in sequence.operations.iter().enumerate() {
        harness1.execute(op.clone());
        harness2.execute(op.clone());

        let context = format!("{}[{}]: {:?}", sequence.name, i, op);
        assert_equivalent(harness1, harness2, &context);
    }
}
