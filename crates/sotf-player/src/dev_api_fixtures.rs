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

/// A deterministic, varied library large enough to exercise collapsed and
/// expanded Home shelves without depending on the host's music collection.
pub fn home_fixture_albums() -> Vec<Album> {
    (0..16)
        .map(|index| {
            let mut album = metadata_fixture_album();
            album.id = Some(100 + index);
            album.title = format!("Home Fixture Album {:02}", index + 1);
            album.year = Some(2020 + (index % 5) as u32);
            album.is_favorite = index % 2 == 0;
            album.play_count = (16 - index) as usize;
            album.tracks[0].title = Some(format!("Home Fixture Track {:02}", index + 1));
            album.tracks[0].artist = Some(format!("Fixture Artist {}", index % 3 + 1));
            album
        })
        .collect()
}
