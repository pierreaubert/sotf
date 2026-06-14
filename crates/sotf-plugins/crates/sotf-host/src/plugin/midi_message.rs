/// A raw MIDI message scheduled within a processing block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiMessage {
    /// Inline MIDI bytes.
    pub data: [u8; 3],
    /// Number of valid bytes in `data`.
    pub len: u8,
}

impl MidiMessage {
    /// Create a MIDI message from up to three raw bytes.
    pub const fn new(data: [u8; 3], len: u8) -> Self {
        Self { data, len }
    }

    /// Create a Note On message.
    pub const fn note_on(channel: u8, note: u8, velocity: u8) -> Self {
        Self::new([0x90 | (channel & 0x0f), note, velocity], 3)
    }

    /// Create a Note Off message.
    pub const fn note_off(channel: u8, note: u8, velocity: u8) -> Self {
        Self::new([0x80 | (channel & 0x0f), note, velocity], 3)
    }

    /// Create a Control Change message.
    pub const fn control_change(channel: u8, controller: u8, value: u8) -> Self {
        Self::new([0xb0 | (channel & 0x0f), controller, value], 3)
    }

    /// Borrow the valid MIDI bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len.min(3) as usize]
    }
}
