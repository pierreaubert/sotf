//! Explicit acceptance of captured takes, independent of capture quality.
use crate::recording_types::{ChannelRecording, ChannelRecordingState};
use std::collections::HashSet;

type TakeKey = (usize, usize, usize, Option<(Option<usize>, usize)>);

/// Session-local review decisions. Capture owners invalidate a take before
/// replacing its result; changing session inputs clears the whole review.
#[derive(Debug, Clone, Default)]
pub struct TakeReviewState {
    accepted: HashSet<TakeKey>,
    pub saved_to: Option<String>,
    saved_inputs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingSaveStatus {
    NotSaved,
    Current,
    Previous,
}

impl TakeReviewState {
    /// Record only a successfully written session and its submitted identity.
    pub fn mark_saved(&mut self, path: String, inputs: serde_json::Value) {
        self.saved_to = Some(path);
        self.saved_inputs = Some(inputs);
    }

    pub fn save_status(&self, inputs: &serde_json::Value) -> RecordingSaveStatus {
        if self.saved_to.is_none() {
            RecordingSaveStatus::NotSaved
        } else if self.saved_inputs.as_ref() == Some(inputs) {
            RecordingSaveStatus::Current
        } else {
            RecordingSaveStatus::Previous
        }
    }

    fn key(take: &ChannelRecording) -> TakeKey {
        (
            take.channel_index,
            take.mic_index,
            take.mic_position_index,
            take.imported_source
                .as_ref()
                .map(|source| (source.driver_index, source.measurement_index)),
        )
    }

    pub fn clear(&mut self) {
        self.accepted.clear();
        self.saved_to = None;
        self.saved_inputs = None;
    }

    pub fn invalidate(&mut self, take: &ChannelRecording) {
        self.accepted.remove(&Self::key(take));
        self.saved_to = None;
        self.saved_inputs = None;
    }

    pub fn accept(&mut self, take: &mut ChannelRecording) -> bool {
        if take.result.is_none()
            || !matches!(
                take.state,
                ChannelRecordingState::Done | ChannelRecordingState::ReviewNeeded
            )
        {
            return false;
        }
        take.state = ChannelRecordingState::Done;
        if self.accepted.insert(Self::key(take)) {
            self.saved_to = None;
            self.saved_inputs = None;
        }
        true
    }

    pub fn is_accepted(&self, take: &ChannelRecording) -> bool {
        take.state == ChannelRecordingState::Done
            && take.result.is_some()
            && self.accepted.contains(&Self::key(take))
    }

    pub fn all_accepted(&self, takes: &[ChannelRecording]) -> bool {
        !takes.is_empty() && takes.iter().all(|take| self.is_accepted(take))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(mic: usize, position: usize) -> ChannelRecording {
        let mut take = ChannelRecording::with_mic_position(0, "L".into(), mic, position);
        take.state = ChannelRecordingState::Done;
        take.result = Some(
            serde_json::from_value(serde_json::json!({
                "channel": 0, "frequencies": [100.0], "magnitude_db": [0.0], "phase_deg": [0.0]
            }))
            .unwrap(),
        );
        take
    }

    #[test]
    fn successful_save_tracks_submitted_identity_and_later_edits() {
        let mut review = TakeReviewState::default();
        let original = serde_json::json!({"name": "Room", "width": 4});
        let edited = serde_json::json!({"name": "Room", "width": 5});
        assert_eq!(review.save_status(&original), RecordingSaveStatus::NotSaved);
        review.mark_saved("session.json".into(), original.clone());
        assert_eq!(review.save_status(&original), RecordingSaveStatus::Current);
        assert_eq!(review.save_status(&edited), RecordingSaveStatus::Previous);
        assert_eq!(review.saved_to.as_deref(), Some("session.json"));
        review.mark_saved("session.json".into(), edited.clone());
        assert_eq!(review.save_status(&edited), RecordingSaveStatus::Current);
        review.clear();
        assert_eq!(review.save_status(&edited), RecordingSaveStatus::NotSaved);
    }

    #[test]
    fn acceptance_is_explicit_and_distinguishes_microphones_and_positions() {
        let mut review = TakeReviewState::default();
        let mut takes = [captured(0, 0), captured(1, 0), captured(0, 1)];
        assert!(!review.all_accepted(&takes));
        assert!(review.accept(&mut takes[0]));
        assert!(!review.is_accepted(&takes[1]));
        assert!(!review.is_accepted(&takes[2]));
        assert!(review.accept(&mut takes[1]));
        takes[2].state = ChannelRecordingState::ReviewNeeded;
        assert!(review.accept(&mut takes[2]));
        assert!(review.all_accepted(&takes));
        review.saved_to = Some("session.json".into());
        review.invalidate(&takes[1]);
        assert!(!review.all_accepted(&takes));
        assert!(review.is_accepted(&takes[0]));
        assert!(review.is_accepted(&takes[2]));
        assert!(review.saved_to.is_none());
        assert!(review.accept(&mut takes[1]));
        review.clear();
        assert!(!review.all_accepted(&takes));
    }

    #[test]
    fn missing_failed_and_in_progress_takes_cannot_be_accepted() {
        let mut review = TakeReviewState::default();
        assert!(!review.all_accepted(&[]));
        let mut take = captured(0, 0);
        for state in [
            ChannelRecordingState::Empty,
            ChannelRecordingState::Error,
            ChannelRecordingState::Recording,
        ] {
            take.state = state;
            assert!(!review.accept(&mut take));
        }
        take.state = ChannelRecordingState::Done;
        take.result = None;
        assert!(!review.accept(&mut take));
    }
}
