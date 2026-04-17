//! Test data factories for creating real Album/Track instances in tests.
//!
//! These builders produce real `sotf_audio_player::{Album, Track}` types,
//! not test doubles. All test suites should use these instead of creating
//! ad-hoc test data or mirror types.

use sotf_audio_player::{Album, Track};
use std::path::PathBuf;

/// Builder for creating test tracks
pub struct TrackBuilder {
    title: Option<String>,
    artist: Option<String>,
    album_artist: Option<String>,
    path: PathBuf,
    duration_secs: Option<u64>,
    channels: Option<u32>,
    sample_rate: Option<u32>,
    bit_depth: Option<u32>,
    genre: Option<String>,
    track_number: Option<u32>,
    composer: Option<String>,
    is_favorite: bool,
    play_count: usize,
}

impl TrackBuilder {
    pub fn new(title: &str) -> Self {
        Self {
            title: Some(title.to_string()),
            artist: None,
            album_artist: None,
            path: PathBuf::from(format!("/test/{}.flac", title)),
            duration_secs: Some(180),
            channels: Some(2),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            genre: None,
            track_number: None,
            composer: None,
            is_favorite: false,
            play_count: 0,
        }
    }

    pub fn with_artist(mut self, artist: &str) -> Self {
        self.artist = Some(artist.to_string());
        self
    }

    pub fn with_album_artist(mut self, album_artist: &str) -> Self {
        self.album_artist = Some(album_artist.to_string());
        self
    }

    pub fn with_duration(mut self, secs: u64) -> Self {
        self.duration_secs = Some(secs);
        self
    }

    pub fn with_channels(mut self, channels: u32) -> Self {
        self.channels = Some(channels);
        self
    }

    pub fn with_sample_rate(mut self, rate: u32) -> Self {
        self.sample_rate = Some(rate);
        self
    }

    pub fn with_bit_depth(mut self, depth: u32) -> Self {
        self.bit_depth = Some(depth);
        self
    }

    pub fn with_genre(mut self, genre: &str) -> Self {
        self.genre = Some(genre.to_string());
        self
    }

    pub fn with_track_number(mut self, num: u32) -> Self {
        self.track_number = Some(num);
        self
    }

    pub fn with_composer(mut self, composer: &str) -> Self {
        self.composer = Some(composer.to_string());
        self
    }

    pub fn favorite(mut self) -> Self {
        self.is_favorite = true;
        self
    }

    pub fn build(self) -> Track {
        Track {
            path: self.path,
            title: self.title,
            artist: self.artist,
            track_number: self.track_number,
            duration_secs: self.duration_secs,
            channels: self.channels,
            sample_rate: self.sample_rate,
            bit_depth: self.bit_depth,
            replay_gain: None,
            replay_peak: None,
            album_gain: None,
            album_peak: None,
            waveform: None,
            genre: self.genre,
            composer: self.composer,
            disc_number: None,
            conductor: None,
            performer: None,
            isrc: None,
            album_artist: self.album_artist,
            ensemble: None,
            edition: None,
            is_favorite: self.is_favorite,
            play_count: self.play_count,
            source: None,
            uuid: None,
        }
    }
}

/// Builder for creating test albums
pub struct AlbumBuilder {
    title: String,
    year: Option<u32>,
    tracks: Vec<Track>,
    album_art_path: Option<PathBuf>,
    is_favorite: bool,
    play_count: usize,
    dynamic_range: Option<f64>,
}

impl AlbumBuilder {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            year: Some(2024),
            tracks: Vec::new(),
            album_art_path: None,
            is_favorite: false,
            play_count: 0,
            dynamic_range: None,
        }
    }

    pub fn with_year(mut self, year: u32) -> Self {
        self.year = Some(year);
        self
    }

    pub fn add_track(mut self, track: Track) -> Self {
        self.tracks.push(track);
        self
    }

    pub fn with_tracks(mut self, tracks: Vec<Track>) -> Self {
        self.tracks = tracks;
        self
    }

    pub fn with_album_art(mut self, path: &str) -> Self {
        self.album_art_path = Some(PathBuf::from(path));
        self
    }

    pub fn with_dynamic_range(mut self, dr: f64) -> Self {
        self.dynamic_range = Some(dr);
        self
    }

    pub fn favorite(mut self) -> Self {
        self.is_favorite = true;
        self
    }

    pub fn build(self) -> Album {
        Album {
            id: None,
            title: self.title,
            year: self.year,
            tracks: self.tracks,
            album_art_path: self.album_art_path,
            album_art_thumbnail: None,
            play_count: self.play_count,
            edition: None,
            dynamic_range: self.dynamic_range,
            is_favorite: self.is_favorite,
            uuid: None,
        }
    }
}

/// Convenience function for creating test tracks
pub fn track(title: &str) -> TrackBuilder {
    TrackBuilder::new(title)
}

/// Convenience function for creating test albums
pub fn album(title: &str) -> AlbumBuilder {
    AlbumBuilder::new(title)
}

/// Create a simple stereo track
pub fn stereo_track(title: &str, artist: &str) -> Track {
    TrackBuilder::new(title)
        .with_artist(artist)
        .with_channels(2)
        .build()
}

/// Create a simple mono track
pub fn mono_track(title: &str, artist: &str) -> Track {
    TrackBuilder::new(title)
        .with_artist(artist)
        .with_channels(1)
        .build()
}

/// Create a hi-res track
pub fn hires_track(title: &str, artist: &str, sample_rate: u32, bit_depth: u32) -> Track {
    TrackBuilder::new(title)
        .with_artist(artist)
        .with_sample_rate(sample_rate)
        .with_bit_depth(bit_depth)
        .build()
}

/// Create a surround track
pub fn surround_track(title: &str, artist: &str, channels: u32) -> Track {
    TrackBuilder::new(title)
        .with_artist(artist)
        .with_channels(channels)
        .build()
}

/// Create a multi-track album for testing playback sequences
pub fn album_with_tracks(title: &str, artist: &str, track_count: usize) -> Album {
    let tracks: Vec<Track> = (1..=track_count)
        .map(|i| {
            TrackBuilder::new(&format!("{} Track {}", title, i))
                .with_artist(artist)
                .with_album_artist(artist)
                .with_track_number(i as u32)
                .with_duration(180)
                .build()
        })
        .collect();

    AlbumBuilder::new(title).with_tracks(tracks).build()
}
