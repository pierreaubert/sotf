//! Library statistics computation.
//!
//! Computes aggregate statistics over a library's albums and tracks.
//! Shared between all app frontends (GPUI, TUI, etc.)

use std::collections::{HashMap, HashSet};

use crate::Album;

/// Cached library statistics to avoid recomputing on every render frame.
/// These stats are expensive to compute (O(n) over all albums/tracks) and should
/// only be invalidated when the library actually changes.
#[derive(Debug, Clone, Default)]
pub struct LibraryStats {
    /// Number of unique artists (case-insensitive)
    pub artists_count: usize,
    /// Number of unique composers (case-insensitive)
    pub composers_count: usize,
    /// Total track count across all albums
    pub total_tracks: usize,
    /// Number of unique genres (case-insensitive)
    pub genres_count: usize,
    /// Count of albums per genre (for selection UI)
    pub genre_counts: HashMap<String, usize>,
    /// Count of albums per year (for selection UI)
    pub year_counts: HashMap<i32, usize>,
    /// Count of albums per decade (for selection UI) - (start_year, end_year, count)
    pub decade_counts: Vec<(i32, i32, usize)>,
    /// Count of albums per artist (for selection UI)
    pub artist_counts: HashMap<String, usize>,
    /// Count of albums per artist first letter (for selection UI)
    pub artist_letter_counts: HashMap<char, usize>,
    /// Count of albums per composer (for selection UI)
    pub composer_counts: HashMap<String, usize>,
    /// Count of albums per composer first letter (for selection UI)
    pub composer_letter_counts: HashMap<char, usize>,
    /// Count of albums per first letter of album name (for selection UI)
    pub album_letter_counts: HashMap<char, usize>,
    /// Count of albums per track count range (for selection UI) - (min, max, count)
    pub track_range_counts: Vec<(usize, usize, usize)>,
    /// Minimum year across all albums (0 if none have year)
    pub min_year: i32,
    /// Maximum year across all albums (0 if none have year)
    pub max_year: i32,
    /// Number of mono albums (1 channel)
    pub mono_count: usize,
    /// Number of stereo albums (2 channels)
    pub stereo_count: usize,
    /// Number of surround albums (5.0/5.1 - 5 or 6 channels)
    pub surround_count: usize,
    /// Number of 7.1 albums (8 channels)
    pub surround71_count: usize,
    /// Number of albums with more than 8 channels
    pub surround_plus_count: usize,
    /// Whether stats are valid (false = need recomputation)
    pub valid: bool,
}

impl LibraryStats {
    /// Compute library statistics from a slice of albums.
    pub fn compute(albums: &[Album]) -> Self {
        let mut artists: HashSet<String> = HashSet::new();
        let mut composers: HashSet<String> = HashSet::new();
        let mut genres: HashSet<String> = HashSet::new();
        let mut genre_counts: HashMap<String, usize> = HashMap::new();
        let mut year_counts: HashMap<i32, usize> = HashMap::new();
        let mut artist_counts: HashMap<String, usize> = HashMap::new();
        let mut artist_letter_counts: HashMap<char, usize> = HashMap::new();
        let mut composer_counts: HashMap<String, usize> = HashMap::new();
        let mut composer_letter_counts: HashMap<char, usize> = HashMap::new();
        let mut album_letter_counts: HashMap<char, usize> = HashMap::new();
        let mut track_count_distribution: HashMap<usize, usize> = HashMap::new();
        let mut total_tracks = 0usize;
        let mut min_year = i32::MAX;
        let mut max_year = 0i32;
        let mut mono_count = 0usize;
        let mut stereo_count = 0usize;
        let mut surround_count = 0usize;
        let mut surround71_count = 0usize;
        let mut surround_plus_count = 0usize;

        for album in albums {
            // Count channels
            if let Some(channels) = album.uniform_channel_count() {
                match channels {
                    1 => mono_count += 1,
                    2 => stereo_count += 1,
                    5 | 6 => surround_count += 1,
                    8 => surround71_count += 1,
                    n if n > 8 => surround_plus_count += 1,
                    _ => {} // 3, 4, 7 channels - rare
                }
            }

            // Track year range and count per year
            if let Some(y) = album.year {
                let y = y as i32;
                if y > 0 {
                    if y < min_year {
                        min_year = y;
                    }
                    if y > max_year {
                        max_year = y;
                    }
                    *year_counts.entry(y).or_insert(0) += 1;
                }
            }

            // Count albums per first letter
            if let Some(first_char) = album.title.chars().next() {
                let letter = first_char.to_ascii_uppercase();
                let key = if letter.is_ascii_alphabetic() {
                    letter
                } else {
                    '#'
                };
                *album_letter_counts.entry(key).or_insert(0) += 1;
            }

            // Count track distribution
            let track_count = album.tracks.len();
            *track_count_distribution.entry(track_count).or_insert(0) += 1;

            // Get album artist for artist counts
            let album_artist = album.artist();
            if !album_artist.is_empty() {
                *artist_counts.entry(album_artist.to_string()).or_insert(0) += 1;
                if let Some(first_char) = album_artist.chars().next() {
                    let letter = first_char.to_ascii_uppercase();
                    let key = if letter.is_ascii_alphabetic() {
                        letter
                    } else {
                        '#'
                    };
                    *artist_letter_counts.entry(key).or_insert(0) += 1;
                }
            }

            // Get album genre and composer from first track
            if let Some(first_track) = album.tracks.first() {
                if let Some(genre) = &first_track.genre {
                    if !genre.is_empty() {
                        *genre_counts.entry(genre.clone()).or_insert(0) += 1;
                    }
                }
                if let Some(composer) = &first_track.composer {
                    if !composer.is_empty() {
                        *composer_counts.entry(composer.clone()).or_insert(0) += 1;
                        if let Some(first_char) = composer.chars().next() {
                            let letter = first_char.to_ascii_uppercase();
                            let key = if letter.is_ascii_alphabetic() {
                                letter
                            } else {
                                '#'
                            };
                            *composer_letter_counts.entry(key).or_insert(0) += 1;
                        }
                    }
                }
            }

            // Count unique artists, composers, genres, tracks
            for track in &album.tracks {
                total_tracks += 1;
                if let Some(artist) = &track.artist {
                    if !artist.is_empty() {
                        artists.insert(artist.to_lowercase());
                    }
                }
                if let Some(composer) = &track.composer {
                    if !composer.is_empty() {
                        composers.insert(composer.to_lowercase());
                    }
                }
                if let Some(genre) = &track.genre {
                    if !genre.is_empty() {
                        genres.insert(genre.to_lowercase());
                    }
                }
            }
        }

        if min_year == i32::MAX {
            min_year = 0;
        }

        let track_range_counts = Self::build_track_ranges(&track_count_distribution);
        let decade_counts = Self::build_decade_counts(&year_counts);

        LibraryStats {
            artists_count: artists.len(),
            composers_count: composers.len(),
            total_tracks,
            genres_count: genres.len(),
            genre_counts,
            year_counts,
            decade_counts,
            artist_counts,
            artist_letter_counts,
            composer_counts,
            composer_letter_counts,
            album_letter_counts,
            track_range_counts,
            min_year,
            max_year,
            mono_count,
            stereo_count,
            surround_count,
            surround71_count,
            surround_plus_count,
            valid: true,
        }
    }

    /// Build decade counts from year counts.
    fn build_decade_counts(year_counts: &HashMap<i32, usize>) -> Vec<(i32, i32, usize)> {
        let mut decade_map: HashMap<i32, usize> = HashMap::new();
        for (year, count) in year_counts {
            let decade_start = (*year / 10) * 10;
            *decade_map.entry(decade_start).or_insert(0) += count;
        }

        let mut decades: Vec<(i32, i32, usize)> = decade_map
            .into_iter()
            .map(|(start, count)| (start, start + 9, count))
            .collect();

        // Sort by decade descending (most recent first)
        decades.sort_by(|a, b| b.0.cmp(&a.0));
        decades
    }

    /// Build track count ranges from distribution.
    fn build_track_ranges(
        distribution: &HashMap<usize, usize>,
    ) -> Vec<(usize, usize, usize)> {
        let ranges = [
            (1, 5),
            (6, 10),
            (11, 15),
            (16, 20),
            (21, 30),
            (31, 50),
            (51, usize::MAX),
        ];

        ranges
            .iter()
            .filter_map(|(min, max)| {
                let count: usize = distribution
                    .iter()
                    .filter(|(tracks, _)| **tracks >= *min && **tracks <= *max)
                    .map(|(_, count)| count)
                    .sum();
                if count > 0 {
                    Some((*min, *max, count))
                } else {
                    None
                }
            })
            .collect()
    }
}
