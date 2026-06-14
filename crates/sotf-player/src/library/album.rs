use super::misc::clean_album_title;
use super::misc::create_probe;
use super::normalize::normalize_album_key;
use super::parse::apply_riff_info_metadata;
use super::track::Track;
use super::types::AlbumChannelType;
use super::types::TrackMetadata;
use std::path::{Path, PathBuf};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::common::Limit;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug, Clone, Default)]
pub struct Album {
    pub id: Option<i64>,
    pub title: String,
    pub year: Option<u32>,
    pub tracks: Vec<Track>,
    pub album_art_path: Option<PathBuf>,
    /// JPEG thumbnail of album art (160x160 for high-DPI displays)
    pub album_art_thumbnail: Option<Vec<u8>>,
    pub play_count: usize,
    pub edition: Option<String>,
    pub dynamic_range: Option<f64>,
    pub is_favorite: bool,
    /// Stable UUID v5 for cross-instance identity (P2P sync, federation).
    pub uuid: Option<String>,
}

impl Album {
    /// Determine the channel configuration of this album
    pub fn channel_type(&self) -> Option<AlbumChannelType> {
        if self.tracks.is_empty() {
            return None;
        }

        // Get channel counts from all tracks that have the info
        let channel_counts: Vec<u32> = self.tracks.iter().filter_map(|t| t.channels).collect();

        if channel_counts.is_empty() {
            return None;
        }

        // Check if all tracks have the same channel count
        let first_channels = channel_counts[0];
        let all_same = channel_counts.iter().all(|&c| c == first_channels);

        if all_same {
            if first_channels == 2 {
                Some(AlbumChannelType::Stereo)
            } else {
                Some(AlbumChannelType::Multichannel(first_channels))
            }
        } else {
            Some(AlbumChannelType::Mixed)
        }
    }

    /// Get the channel count if all tracks have the same number of channels
    pub fn uniform_channel_count(&self) -> Option<u32> {
        match self.channel_type()? {
            AlbumChannelType::Stereo => Some(2),
            AlbumChannelType::Multichannel(n) => Some(n),
            AlbumChannelType::Mixed => None,
        }
    }

    /// Get the sample rate of this album (from the first track that has it)
    pub fn sample_rate(&self) -> Option<u32> {
        self.tracks.iter().find_map(|t| t.sample_rate)
    }
}

impl Album {
    /// Get the artist(s) for this album by scanning all tracks.
    /// Returns the album_artist if consistent across tracks, otherwise
    /// returns "Various Artists" for compilations.
    /// Prefers album_artist, falls back to track artist.
    pub fn artist(&self) -> String {
        use std::collections::BTreeSet;

        // First try to collect album_artists
        let album_artists: BTreeSet<String> = self
            .tracks
            .iter()
            .filter_map(|t| t.album_artist.clone())
            .collect();

        // If we have a consistent album_artist, use it
        if album_artists.len() == 1 {
            return album_artists.into_iter().next().unwrap();
        }

        // Otherwise fall back to track artists
        let track_artists: BTreeSet<String> = self
            .tracks
            .iter()
            .filter_map(|t| t.artist.clone())
            .collect();

        if track_artists.is_empty() {
            "Unknown Artist".to_string()
        } else if track_artists.len() == 1 {
            track_artists.into_iter().next().unwrap()
        } else {
            // Multiple artists - this is a compilation
            "Various Artists".to_string()
        }
    }

    pub fn display_name(&self) -> String {
        let artist = self.artist();
        if let Some(year) = self.year {
            format!("{} - {} ({})", artist, self.title, year)
        } else {
            format!("{} - {}", artist, self.title)
        }
    }

    pub fn sort_tracks(&mut self) {
        self.tracks.sort_by_key(|t| t.track_number);
    }
}

pub(super) fn extract_metadata(path: &Path) -> Result<TrackMetadata, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(&ext.to_string_lossy());
    }

    let format_opts = FormatOptions::default();
    // Use larger limits to avoid crashes with files that have large metadata or embedded artwork
    let metadata_opts = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(10 * 1024 * 1024)) // 10 MB for metadata
        .limit_visual_bytes(Limit::Maximum(20 * 1024 * 1024)); // 20 MB for embedded artwork

    // Use custom probe with registered formats for better format detection
    let probe = create_probe();
    let mut format = probe.probe(&hint, mss, format_opts, metadata_opts)?;

    let mut metadata = TrackMetadata::default();

    // Extract duration, channel count, sample rate, and bit depth from the format
    if let Some(track) = format.default_track(TrackType::Audio) {
        // Get duration
        if let Some(time_base) = track.time_base
            && let Some(n_frames) = track.num_frames
        {
            let duration =
                time_base.calc_time(symphonia::core::units::Timestamp::new(n_frames as i64));
            metadata.duration_secs = duration.map(|time| time.as_secs().max(0) as u64);
        }

        if let Some(CodecParameters::Audio(codec_params)) = &track.codec_params {
            // Get channel count
            if let Some(channels) = codec_params.channels.as_ref() {
                metadata.channels = Some(channels.count() as u32);
            }

            // Get sample rate
            if let Some(sample_rate) = codec_params.sample_rate {
                metadata.sample_rate = Some(sample_rate);
            }

            // Get bit depth (bits per sample)
            if let Some(bits_per_sample) = codec_params.bits_per_sample {
                metadata.bit_depth = Some(bits_per_sample);
            }
        }
    }

    // Extract metadata tags
    use symphonia::core::meta::{RawValue, StandardTag, Tag};

    let non_empty_string = |value: &str| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };

    let raw_value = |value: &RawValue| non_empty_string(&value.to_string());

    let mut process_tag = |tag: &Tag| match &tag.std {
        Some(StandardTag::TrackTitle(value)) => {
            metadata.title = non_empty_string(value);
        }
        Some(StandardTag::Artist(value)) => {
            metadata.artist = non_empty_string(value);
        }
        Some(StandardTag::Album(value)) => {
            metadata.album = non_empty_string(value);
        }
        Some(StandardTag::TrackNumber(num)) => {
            metadata.track_number = Some(*num as u32);
        }
        Some(StandardTag::RecordingDate(value)) | Some(StandardTag::ReleaseDate(value)) => {
            if let Some(year_str) = value.split('-').next()
                && let Ok(year) = year_str.parse()
            {
                metadata.year = Some(year);
            }
        }
        Some(StandardTag::RecordingYear(year)) | Some(StandardTag::ReleaseYear(year)) => {
            metadata.year = Some(*year as u32);
        }
        Some(StandardTag::Genre(value)) => {
            metadata.genre = non_empty_string(value);
        }
        Some(StandardTag::Composer(value)) => {
            metadata.composer = non_empty_string(value);
        }
        Some(StandardTag::DiscNumber(num)) => {
            metadata.disc_number = Some(*num as u32);
        }
        Some(StandardTag::Conductor(value)) => {
            metadata.conductor = non_empty_string(value);
        }
        Some(StandardTag::Performer(value)) => {
            metadata.performer = non_empty_string(value);
        }
        Some(StandardTag::IdentIsrc(value)) => {
            metadata.isrc = non_empty_string(value);
        }
        Some(StandardTag::AlbumArtist(value)) => {
            metadata.album_artist = non_empty_string(value);
        }
        Some(StandardTag::Ensemble(value)) => {
            metadata.ensemble = non_empty_string(value);
        }
        _ => {
            let raw_key = tag.raw.key.to_ascii_lowercase();
            match raw_key.as_str() {
                "title" => metadata.title = raw_value(&tag.raw.value),
                "artist" => metadata.artist = raw_value(&tag.raw.value),
                "album" => metadata.album = raw_value(&tag.raw.value),
                "genre" => metadata.genre = raw_value(&tag.raw.value),
                "composer" => metadata.composer = raw_value(&tag.raw.value),
                "conductor" => metadata.conductor = raw_value(&tag.raw.value),
                "performer" => metadata.performer = raw_value(&tag.raw.value),
                "isrc" => metadata.isrc = raw_value(&tag.raw.value),
                "albumartist" | "album_artist" | "album artist" => {
                    metadata.album_artist = raw_value(&tag.raw.value);
                }
                "ensemble" => metadata.ensemble = raw_value(&tag.raw.value),
                "track" | "tracknumber" | "track_number" => {
                    if let Some(value) = raw_value(&tag.raw.value) {
                        let track_num = value.split('/').next().unwrap_or(&value);
                        if let Ok(num) = track_num.trim().parse() {
                            metadata.track_number = Some(num);
                        }
                    }
                }
                "disc" | "discnumber" | "disc_number" => {
                    if let Some(value) = raw_value(&tag.raw.value) {
                        let disc_num = value.split('/').next().unwrap_or(&value);
                        if let Ok(num) = disc_num.trim().parse() {
                            metadata.disc_number = Some(num);
                        }
                    }
                }
                "date" | "year" | "releasedate" | "release_date" => {
                    if let Some(value) = raw_value(&tag.raw.value)
                        && let Some(year_str) = value.split('-').next()
                        && let Ok(year) = year_str.parse()
                    {
                        metadata.year = Some(year);
                    }
                }
                _ => {}
            }
        }
    };

    // Helper to process tags from a metadata revision
    let mut process_tags = |metadata_rev: &symphonia::core::meta::MetadataRevision| {
        for tag in &metadata_rev.media.tags {
            process_tag(tag);
        }
        for track in &metadata_rev.per_track {
            for tag in &track.metadata.tags {
                process_tag(tag);
            }
        }
    };

    // Check format metadata, including metadata discovered during probing.
    if let Some(metadata_rev) = format.metadata().current() {
        process_tags(metadata_rev);
    }

    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("wav") || ext.eq_ignore_ascii_case("wave"))
    {
        apply_riff_info_metadata(path, &mut metadata);
    }

    // Try to detect edition from directory name
    if let Some(parent) = path.parent() {
        if let Some(dir_name) = parent.file_name().map(|n| n.to_string_lossy()) {
            let dir_str = dir_name.as_ref();
            let mut edition = None;

            // Look for (...)
            if let Some(start) = dir_str.rfind('(') {
                if let Some(end) = dir_str.rfind(')') {
                    if end > start {
                        edition = Some(dir_str[start + 1..end].trim().to_string());
                    }
                }
            }
            // Look for [...] - prioritizing brackets if present
            if let Some(start) = dir_str.rfind('[') {
                if let Some(end) = dir_str.rfind(']') {
                    if end > start {
                        edition = Some(dir_str[start + 1..end].trim().to_string());
                    }
                }
            }

            if let Some(ed) = edition {
                // Only use if it looks like an edition info (heuristic)
                metadata.edition = Some(ed);
            }
        }
    }

    Ok(metadata)
}

pub(super) fn directory_album_key(album: &Album) -> String {
    if let Some(id) = album.id {
        return format!("id:{id}");
    }

    if let Some(uuid) = &album.uuid {
        return format!("uuid:{uuid}");
    }

    let first_parent = album
        .tracks
        .iter()
        .find_map(|track| track.path.parent())
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_default();
    let edition = album.edition.as_deref().unwrap_or_default();

    format!(
        "fallback:{}|{}|{}|{}",
        normalize_album_key(&album.title),
        normalize_album_key(edition),
        normalize_album_key(&album.artist()),
        first_parent,
    )
}

pub(super) fn folder_album_key(album: &Album) -> Option<String> {
    album
        .tracks
        .iter()
        .find_map(|track| track.path.parent())
        .map(|parent| format!("__folder__|{}", parent.to_string_lossy()))
}

/// Helper function to group and merge albums
///
/// For albums with the same title (regardless of artist):
/// - Merge all tracks from all albums (for multi-disc albums)
/// - Deduplicate tracks by title + disc + track number (for duplicate editions)
/// - Keep the album metadata (year, art) from the album with highest DR
/// - Prefer albums with known artists over "Unknown Artist" or "Various Artists"
pub fn group_and_merge_albums(albums: Vec<&Album>) -> Vec<Album> {
    // Group by (normalized_title, normalized_artist) to keep different artists separate.
    // Albums with unknown/various artists are merged into a matching known-artist group
    // if one exists for the same title.

    let is_unknown_artist = |artist: &str| -> bool {
        let lower = artist.to_lowercase();
        lower.contains("unknown") || lower.contains("various")
    };

    // First pass: group by (title, artist)
    let mut groups: std::collections::HashMap<(String, String), Vec<&Album>> =
        std::collections::HashMap::new();

    for album in &albums {
        let title = album.title.trim();
        let normalized_title = clean_album_title(title).trim().to_lowercase();
        let artist = album.artist();
        let normalized_artist = normalize_album_key(&artist);
        groups
            .entry((normalized_title, normalized_artist))
            .or_default()
            .push(album);
    }

    // Second pass: merge unknown-artist groups into known-artist groups with the same title
    let unknown_keys: Vec<(String, String)> = groups
        .keys()
        .filter(|(title, artist_key)| {
            // Check if all albums in this group have unknown artists
            groups[&(title.clone(), artist_key.clone())]
                .iter()
                .all(|a| is_unknown_artist(&a.artist()))
        })
        .cloned()
        .collect();

    for (title, unknown_artist_key) in unknown_keys {
        // Find a known-artist group with the same title
        let known_key = groups
            .keys()
            .find(|(t, ak)| {
                *t == title
                    && *ak != unknown_artist_key
                    && groups[&(t.clone(), ak.clone())]
                        .iter()
                        .any(|a| !is_unknown_artist(&a.artist()))
            })
            .cloned();

        if let Some(target_key) = known_key {
            if let Some(unknown_albums) = groups.remove(&(title, unknown_artist_key)) {
                groups.get_mut(&target_key).unwrap().extend(unknown_albums);
            }
        }
    }

    // Merge albums and deduplicate tracks
    let mut merged_albums: Vec<Album> = Vec::new();
    for group in groups.values() {
        if group.is_empty() {
            continue;
        }

        // Helper to check if an artist is a "known" artist (not Unknown/Various)
        let is_known_artist = |artist: &str| -> bool {
            let lower = artist.to_lowercase();
            !lower.contains("unknown") && !lower.contains("various")
        };

        // Select the best album for metadata:
        // 1. Prefer albums with known artists over "Unknown Artist" / "Various Artists"
        // 2. Among albums with same artist quality, prefer highest dynamic range
        let best_album = group
            .iter()
            .max_by(|a, b| {
                let a_known = is_known_artist(&a.artist());
                let b_known = is_known_artist(&b.artist());

                // First compare by artist quality (known > unknown)
                match (a_known, b_known) {
                    (true, false) => return std::cmp::Ordering::Greater,
                    (false, true) => return std::cmp::Ordering::Less,
                    _ => {}
                }

                // Then by dynamic range
                let dr_a = a.dynamic_range.unwrap_or(0.0);
                let dr_b = b.dynamic_range.unwrap_or(0.0);
                dr_a.partial_cmp(&dr_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(&group[0]);

        let mut album = (*best_album).clone();

        // Clean up the title if there are multiple editions
        if group.len() > 1 {
            album.title = clean_album_title(&album.title);

            // Collect all tracks from all albums in the group
            let mut all_tracks = Vec::new();
            for g_album in group {
                all_tracks.extend(g_album.tracks.clone());
            }
            album.tracks = all_tracks;

            // Normalize album_artist on merged tracks to the best album's artist
            // so that Album::artist() doesn't return "Various Artists" due to
            // unknown-artist tracks mixed with known-artist tracks.
            let best_artist = best_album.artist();
            if is_known_artist(&best_artist) {
                for track in &mut album.tracks {
                    if let Some(ref aa) = track.album_artist {
                        if !is_known_artist(aa) {
                            track.album_artist = Some(best_artist.clone());
                        }
                    }
                    if let Some(ref a) = track.artist {
                        if !is_known_artist(a) {
                            track.artist = Some(best_artist.clone());
                        }
                    }
                }
            }
        }

        // Deduplicate tracks by (title + disc + track number) only when track numbers exist.
        // Files without track numbers can legitimately share a title, so keep path identity.
        let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        album.tracks.retain(|track| {
            let title_key = track.title.as_ref().map(|t| t.trim().to_lowercase());
            let key = if track.track_number.is_none() {
                format!("path:{}", track.path.display())
            } else {
                let disc = track.disc_number.unwrap_or(1);
                let track_num = track.track_number.unwrap_or_default();
                format!(
                    "tag:{}|{}|{}",
                    title_key.unwrap_or_default(),
                    disc,
                    track_num
                )
            };
            seen_keys.insert(key)
        });

        // Sort tracks by disc and track number
        album.tracks.sort_by(|a, b| {
            a.disc_number
                .unwrap_or(1)
                .cmp(&b.disc_number.unwrap_or(1))
                .then_with(|| {
                    a.track_number
                        .unwrap_or(0)
                        .cmp(&b.track_number.unwrap_or(0))
                })
        });

        merged_albums.push(album);
    }

    merged_albums
}
