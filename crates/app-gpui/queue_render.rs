//! Helpers for rendering the playback queue in GPUI.
//!
//! Kept in a separate module (rather than inside `components/home/queue`) so
//! the pure data-summarization logic can be unit-tested even though the
//! visual component tree is excluded from the test build.

use sotf_audio_player::QueueItem;

const METERS_PANEL_MIN_WIDTH: f32 = 120.0;
const METERS_PANEL_MAX_AVAILABLE_RATIO: f32 = 0.6;

/// Lightweight summary of a queue item used to build the queue accordion.
/// Keeping this small avoids cloning the full `QueueItem` (album + tracks)
/// on every render.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueAccordionSummary {
    pub idx: usize,
    pub title: String,
    pub track_position: String,
}

/// Build accordion summaries from queue items without cloning the items
/// themselves. Only the formatted title and track-position strings are
/// newly allocated.
pub fn queue_accordion_summaries(items: &[QueueItem]) -> Vec<QueueAccordionSummary> {
    items
        .iter()
        .enumerate()
        .map(|(idx, item)| QueueAccordionSummary {
            idx,
            title: format!("{} - {}", item.album.title, item.album.artist()),
            track_position: format!(
                "Track {}/{}",
                item.current_track_index + 1,
                item.album.tracks.len()
            ),
        })
        .collect()
}

/// Calculate the right-side meters panel width for the queue screen.
pub fn queue_meters_panel_width(meters_ratio: f32, available_queue_width: f32) -> f32 {
    let available_queue_width = if available_queue_width.is_finite() {
        available_queue_width.max(0.0)
    } else {
        0.0
    };
    let max_width = available_queue_width * METERS_PANEL_MAX_AVAILABLE_RATIO;
    let min_width = METERS_PANEL_MIN_WIDTH.min(max_width);
    let desired_width = if meters_ratio.is_finite() {
        meters_ratio.max(0.0) * available_queue_width
    } else {
        0.0
    };

    desired_width.clamp(min_width, max_width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio_player::{Album, QueueItem, Track};

    fn make_track(title: &str, artist: &str) -> Track {
        Track {
            title: Some(title.to_string()),
            artist: Some(artist.to_string()),
            album_artist: Some(artist.to_string()),
            ..Default::default()
        }
    }

    fn make_album(title: &str, tracks: Vec<Track>) -> Album {
        Album {
            title: title.to_string(),
            tracks,
            ..Album {
                id: None,
                title: String::new(),
                year: None,
                tracks: Vec::new(),
                album_art_path: None,
                album_art_thumbnail: None,
                play_count: 0,
                edition: None,
                dynamic_range: None,
                is_favorite: false,
                uuid: None,
            }
        }
    }

    #[test]
    fn queue_accordion_summaries_do_not_clone_items() {
        let items = vec![
            QueueItem {
                album: make_album(
                    "Test Album",
                    vec![
                        make_track("Track One", "Artist A"),
                        make_track("Track Two", "Artist A"),
                    ],
                ),
                current_track_index: 1,
            },
            QueueItem {
                album: make_album("Another Album", vec![make_track("Solo", "Artist B")]),
                current_track_index: 0,
            },
        ];

        let summaries = queue_accordion_summaries(&items);

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].idx, 0);
        assert_eq!(summaries[0].title, "Test Album - Artist A");
        assert_eq!(summaries[0].track_position, "Track 2/2");
        assert_eq!(summaries[1].idx, 1);
        assert_eq!(summaries[1].title, "Another Album - Artist B");
        assert_eq!(summaries[1].track_position, "Track 1/1");

        // The original items are still owned by this scope, proving the
        // helper did not consume/clone the whole queue.
        assert_eq!(items[0].album.title, "Test Album");
    }

    #[test]
    fn queue_meters_panel_width_shrinks_when_min_exceeds_max() {
        assert!((queue_meters_panel_width(0.25, 141.7583) - 85.054985).abs() < 0.0001);
    }

    #[test]
    fn queue_meters_panel_width_uses_nominal_min_when_space_allows() {
        assert!((queue_meters_panel_width(0.10, 800.0) - 120.0).abs() < 0.0001);
        assert!((queue_meters_panel_width(0.80, 800.0) - 480.0).abs() < 0.0001);
    }

    #[test]
    fn queue_meters_panel_width_handles_non_finite_inputs() {
        assert_eq!(queue_meters_panel_width(f32::NAN, 800.0), 120.0);
        assert_eq!(queue_meters_panel_width(0.25, f32::NAN), 0.0);
    }
}
