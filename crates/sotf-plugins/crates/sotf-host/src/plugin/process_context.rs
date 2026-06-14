use super::midi_event::MidiEvent;
use super::note_expression_event::NoteExpressionEvent;
use super::transport_info::TransportInfo;

/// Processing context passed to plugins.
#[derive(Clone, Copy)]
pub struct ProcessContext<'a> {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of frames in this processing block
    pub num_frames: usize,
    /// Transport and musical-time metadata at block start.
    pub transport: TransportInfo,
    /// MIDI events scheduled within this processing block.
    pub midi_events: &'a [MidiEvent],
    /// Per-note expression events scheduled within this processing block.
    pub note_expression_events: &'a [NoteExpressionEvent],
}

impl<'a> ProcessContext<'a> {
    /// Create a processing context with default transport and no MIDI events.
    pub fn new(sample_rate: u32, num_frames: usize) -> Self {
        Self {
            sample_rate,
            num_frames,
            transport: TransportInfo::at_sample(0, sample_rate),
            midi_events: &[],
            note_expression_events: &[],
        }
    }

    /// Return a copy with absolute sample position populated.
    pub fn with_sample_position(mut self, sample_position: u64) -> Self {
        let prev = self.transport;
        self.transport = TransportInfo::at_sample(sample_position, self.sample_rate)
            .with_tempo(prev.bpm, self.sample_rate)
            .with_time_signature(
                prev.time_signature.numerator,
                prev.time_signature.denominator,
            )
            .with_loop_range(prev.loop_range);
        self.transport.playing = prev.playing;
        self.transport.recording = prev.recording;
        self
    }

    /// Return a copy with transport metadata.
    pub const fn with_transport(mut self, transport: TransportInfo) -> Self {
        self.transport = transport;
        self
    }

    /// Return a copy with borrowed MIDI events.
    pub const fn with_midi_events<'b>(self, midi_events: &'b [MidiEvent]) -> ProcessContext<'b> {
        ProcessContext {
            sample_rate: self.sample_rate,
            num_frames: self.num_frames,
            transport: self.transport,
            midi_events,
            note_expression_events: &[],
        }
    }

    /// Return a copy with borrowed MIDI and per-note expression events.
    pub const fn with_events<'b>(
        self,
        midi_events: &'b [MidiEvent],
        note_expression_events: &'b [NoteExpressionEvent],
    ) -> ProcessContext<'b> {
        ProcessContext {
            sample_rate: self.sample_rate,
            num_frames: self.num_frames,
            transport: self.transport,
            midi_events,
            note_expression_events,
        }
    }
}
