//! MIDI Clock transport and tick scheduling helpers.

/// MIDI Clock pulses per quarter note, per the MIDI spec.
pub const MIDI_CLOCK_PPQ: u32 = 24;
pub const MIDI_CLOCK_TICK: u8 = 0xF8;
pub const MIDI_CLOCK_START: u8 = 0xFA;
pub const MIDI_CLOCK_CONTINUE: u8 = 0xFB;
pub const MIDI_CLOCK_STOP: u8 = 0xFC;

/// Return the clock tick interval in samples for `bpm` and `ppq`.
pub fn clock_tick_interval_samples(bpm: f64, sample_rate: u32, ppq: u32) -> Option<f64> {
    if !bpm.is_finite() || bpm <= 0.0 || sample_rate == 0 || ppq == 0 {
        return None;
    }
    Some((60.0 * sample_rate as f64) / (bpm * ppq as f64))
}

/// Compute sample offsets in a processing block where MIDI Clock ticks should be emitted.
///
/// `block_start_sample` is absolute transport time in samples; returned offsets are
/// relative to the start of the block.
pub fn schedule_clock_ticks_for_block(
    bpm: f64,
    sample_rate: u32,
    ppq: u32,
    block_start_sample: u64,
    block_frames: usize,
) -> Vec<usize> {
    let Some(interval) = clock_tick_interval_samples(bpm, sample_rate, ppq) else {
        return Vec::new();
    };
    if block_frames == 0 {
        return Vec::new();
    }

    let block_start = block_start_sample as f64;
    let block_end = block_start + block_frames as f64;
    let mut tick = (block_start / interval).ceil() * interval;
    let mut offsets = Vec::new();

    while tick < block_end {
        let offset = (tick - block_start).round();
        if offset >= 0.0 && offset < block_frames as f64 {
            offsets.push(offset as usize);
        }
        tick += interval;
    }

    offsets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_interval_uses_bpm_ppq_and_sample_rate() {
        assert_eq!(
            clock_tick_interval_samples(120.0, 48_000, MIDI_CLOCK_PPQ),
            Some(1_000.0)
        );
        assert!(clock_tick_interval_samples(0.0, 48_000, MIDI_CLOCK_PPQ).is_none());
        assert!(clock_tick_interval_samples(120.0, 48_000, 0).is_none());
    }

    #[test]
    fn clock_scheduler_returns_block_relative_tick_offsets() {
        let offsets = schedule_clock_ticks_for_block(120.0, 48_000, MIDI_CLOCK_PPQ, 0, 2_100);
        assert_eq!(offsets, vec![0, 1_000, 2_000]);

        let offsets = schedule_clock_ticks_for_block(120.0, 48_000, MIDI_CLOCK_PPQ, 2_100, 1_000);
        assert_eq!(offsets, vec![900]);
    }
}
