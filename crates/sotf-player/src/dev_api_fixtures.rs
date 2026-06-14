//! Shared fixtures for the `dev-api` feature across UI shells.

use crate::{Album, Track};

/// Returns a deterministic album fixture used by dev API endpoints.
pub fn metadata_fixture_album() -> Album {
    let track_path = std::env::temp_dir()
        .join("sotf-dev-driver")
        .join("metadata-scenario")
        .join("scenario-track.flac");
    Album {
        id: Some(7),
        title: "Scenario Album".to_string(),
        year: Some(1999),
        tracks: vec![Track {
            path: track_path,
            title: Some("Scenario Track".to_string()),
            artist: Some("Scenario Artist".to_string()),
            album_artist: Some("Scenario Artist".to_string()),
            track_number: Some(1),
            sample_rate: Some(44_100),
            channels: Some(2),
            ..Default::default()
        }],
        ..Default::default()
    }
}
