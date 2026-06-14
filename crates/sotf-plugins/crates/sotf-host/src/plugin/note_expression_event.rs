use super::types::NoteExpressionKind;

/// A per-note expression event timestamped relative to the start of a block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteExpressionEvent {
    /// Sample offset within the current block.
    pub sample_offset: usize,
    /// Host-assigned note ID when available.
    pub note_id: i32,
    /// MIDI channel for MPE-style expression.
    pub channel: u8,
    /// MIDI note number.
    pub note: u8,
    /// Expression semantic.
    pub expression: NoteExpressionKind,
    /// Normalized or unit-specific expression value supplied by the host.
    pub value: f64,
}

impl NoteExpressionEvent {
    /// Create a block-relative per-note expression event.
    pub const fn new(
        sample_offset: usize,
        note_id: i32,
        channel: u8,
        note: u8,
        expression: NoteExpressionKind,
        value: f64,
    ) -> Self {
        Self {
            sample_offset,
            note_id,
            channel,
            note,
            expression,
            value,
        }
    }
}
