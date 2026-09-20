//! Shared fixtures for the `dev-api` feature across UI shells.

use crate::{Album, Track};

/// Source metadata for exercising the production file-import path in UI QA.
pub fn write_headphone_provenance_fixture(path: &std::path::Path) -> Result<(), String> {
    let curve = autoeq::read::read_curve_from_csv(&path.to_path_buf())
        .map_err(|error| error.to_string())?;
    let mut record = autoeq_measurements::MeasurementRecord::from_source_path(
        curve,
        autoeq_measurements::MeasurementOrigin::Csv,
        path,
    )
    .map_err(|error| error.to_string())?;
    for (key, value) in [
        ("model", "Reference headphone, revision B"),
        ("rig", "IEC 60318-4 fixture"),
        ("sample", "Unit 2, left ear"),
        ("compensation", "Uncompensated"),
    ] {
        record
            .provenance
            .acquisition
            .extensions
            .insert(key.into(), value.into());
    }
    autoeq_measurements::write_sidecar(path, &record).map_err(|error| error.to_string())?;
    Ok(())
}

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
            album.tracks[0].path = album.tracks[0]
                .path
                .with_file_name(format!("home-fixture-{}.flac", index + 1));
            album.tracks[0].artist = Some(format!("Fixture Artist {}", index % 3 + 1));
            album
        })
        .collect()
}
