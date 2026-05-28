// ============================================================================
// MIDI Sequencer Types — Events, clips, and regions for MIDI sequencing
// ============================================================================

use crate::message::MidiMessage;
use serde::{Deserialize, Serialize};

/// A timestamped MIDI event for sequencing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiEvent {
    /// Time offset in samples from the start of the clip
    pub time_samples: u64,
    /// The MIDI message
    pub message: MidiMessage,
}

impl MidiEvent {
    pub fn note_on(time_samples: u64, channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            time_samples,
            message: MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            },
        }
    }

    pub fn note_off(time_samples: u64, channel: u8, note: u8) -> Self {
        Self {
            time_samples,
            message: MidiMessage::NoteOff {
                channel,
                note,
                velocity: 0,
            },
        }
    }

    pub fn cc(time_samples: u64, channel: u8, controller: u8, value: u8) -> Self {
        Self {
            time_samples,
            message: MidiMessage::ControlChange {
                channel,
                controller,
                value,
            },
        }
    }
}

/// A collection of MIDI events (analogous to an audio Clip).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiClip {
    /// Events sorted by time_samples
    pub events: Vec<MidiEvent>,
    /// Duration in samples (may extend beyond last event for trailing silence)
    pub duration_samples: u64,
}

impl MidiClip {
    pub fn new(duration_samples: u64) -> Self {
        Self {
            events: Vec::new(),
            duration_samples,
        }
    }

    /// Add an event. Events will be sorted by time on `sort()`.
    pub fn add_event(&mut self, event: MidiEvent) {
        self.events.push(event);
    }

    /// Sort events by time (must be called before playback).
    pub fn sort(&mut self) {
        self.events.sort_by_key(|e| e.time_samples);
    }

    /// Get events in the time range [start, start+length) relative to clip start.
    pub fn events_in_range(&self, start_samples: u64, length_samples: u64) -> Vec<&MidiEvent> {
        let end = start_samples + length_samples;
        self.events
            .iter()
            .filter(|e| e.time_samples >= start_samples && e.time_samples < end)
            .collect()
    }
}

/// A MIDI clip placed at a position on the timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiRegion {
    /// The MIDI clip
    pub clip: MidiClip,
    /// Start position on the timeline in samples
    pub position_samples: u64,
}

impl MidiRegion {
    pub fn new(clip: MidiClip, position_samples: u64) -> Self {
        Self {
            clip,
            position_samples,
        }
    }

    /// End position on the timeline in samples.
    pub fn end_samples(&self) -> u64 {
        self.position_samples + self.clip.duration_samples
    }

    /// Check if this region overlaps with a given time range.
    pub fn overlaps(&self, start: u64, length: u64) -> bool {
        let region_end = self.end_samples();
        let range_end = start + length;
        self.position_samples < range_end && region_end > start
    }

    /// Get events from this region that fall within the given timeline range.
    /// Returns events with their time adjusted to be relative to the range start.
    pub fn events_in_timeline_range(
        &self,
        timeline_start: u64,
        length: u64,
    ) -> Vec<(u64, &MidiMessage)> {
        let clip_start = timeline_start.saturating_sub(self.position_samples);
        let clip_end = (timeline_start + length).saturating_sub(self.position_samples);
        let clip_length = clip_end - clip_start;

        self.clip
            .events_in_range(clip_start, clip_length)
            .into_iter()
            .map(|e| {
                let timeline_time = self.position_samples + e.time_samples;
                let relative_time = timeline_time.saturating_sub(timeline_start);
                (relative_time, &e.message)
            })
            .collect()
    }

    /// Iterate over events in the timeline range without allocating.
    pub fn for_each_event_in_timeline_range<F>(&self, timeline_start: u64, length: u64, mut f: F)
    where
        F: FnMut(u64, &MidiMessage),
    {
        let clip_start = timeline_start.saturating_sub(self.position_samples);
        let clip_end = (timeline_start + length).saturating_sub(self.position_samples);
        if clip_end <= clip_start {
            return;
        }
        for e in &self.clip.events {
            if e.time_samples < clip_start {
                continue;
            }
            if e.time_samples >= clip_end {
                break;
            }
            let timeline_time = self.position_samples + e.time_samples;
            let relative_time = timeline_time.saturating_sub(timeline_start);
            f(relative_time, &e.message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_clip_events_in_range() {
        let mut clip = MidiClip::new(48000);
        clip.add_event(MidiEvent::note_on(0, 0, 60, 100));
        clip.add_event(MidiEvent::note_off(24000, 0, 60));
        clip.add_event(MidiEvent::note_on(24000, 0, 64, 80));
        clip.add_event(MidiEvent::note_off(48000, 0, 64));
        clip.sort();

        // First half
        let events = clip.events_in_range(0, 24000);
        assert_eq!(events.len(), 1); // Only note_on at 0

        // Second half
        let events = clip.events_in_range(24000, 24000);
        assert_eq!(events.len(), 2); // note_off(60) + note_on(64) at 24000
    }

    #[test]
    fn test_midi_region_timeline_events() {
        let mut clip = MidiClip::new(48000);
        clip.add_event(MidiEvent::note_on(0, 0, 60, 100));
        clip.add_event(MidiEvent::note_off(24000, 0, 60));
        clip.sort();

        let region = MidiRegion::new(clip, 96000); // Starts at 2 sec

        // Query timeline range [96000, 120000) = first 500ms of the clip
        let events = region.events_in_timeline_range(96000, 24000);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, 0); // note_on at relative time 0
    }

    #[test]
    fn test_midi_region_overlaps() {
        let clip = MidiClip::new(48000);
        let region = MidiRegion::new(clip, 10000);
        // Region: [10000, 58000)
        assert!(region.overlaps(0, 20000));
        assert!(!region.overlaps(60000, 5000));
    }
}
