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
    pub genre_counts: std::collections::HashMap<String, usize>,
    /// Count of albums per year (for selection UI)
    pub year_counts: std::collections::HashMap<i32, usize>,
    /// Count of albums per decade (for selection UI) - key is (start_year, end_year)
    pub decade_counts: Vec<(i32, i32, usize)>,
    /// Count of albums per artist (for selection UI)
    pub artist_counts: std::collections::HashMap<String, usize>,
    /// Count of albums per artist first letter (for selection UI)
    pub artist_letter_counts: std::collections::HashMap<char, usize>,
    /// Count of albums per composer (for selection UI)
    pub composer_counts: std::collections::HashMap<String, usize>,
    /// Count of albums per composer first letter (for selection UI)
    pub composer_letter_counts: std::collections::HashMap<char, usize>,
    /// Count of albums per first letter of album name (for selection UI)
    pub album_letter_counts: std::collections::HashMap<char, usize>,
    /// Count of albums per track count range (for selection UI)
    pub track_range_counts: Vec<(usize, usize, usize)>, // (min, max, count)
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
