// ============================================================================
// Standard MIDI File (SMF) Parser — Import .mid files into MidiClip
// ============================================================================
//
// Parses Type 0 and Type 1 Standard MIDI Files into MidiClip events.
// Converts delta-time ticks to absolute sample positions using tempo information.

use crate::message::MidiMessage;
use crate::sequencer::{MidiClip, MidiEvent};
use std::io::Read;
use std::path::Path;

/// Import a Standard MIDI File into a Vec of MidiClips (one per track).
///
/// # Arguments
/// * `path` — Path to the .mid file
/// * `sample_rate` — Target sample rate for converting ticks to samples
///
/// Returns one MidiClip per MIDI track in the file.
pub fn import_midi_file(path: &Path, sample_rate: u32) -> Result<Vec<MidiClip>, String> {
    let data =
        std::fs::read(path).map_err(|e| format!("Failed to read MIDI file: {e}"))?;
    parse_smf(&data, sample_rate)
}

/// Parse SMF data from bytes.
pub fn parse_smf(data: &[u8], sample_rate: u32) -> Result<Vec<MidiClip>, String> {
    let mut pos = 0;

    // Parse header chunk
    let header = parse_header(data, &mut pos)?;
    let ticks_per_beat = header.division as f64;

    let mut clips = Vec::new();

    // Parse track chunks
    for _ in 0..header.num_tracks {
        if pos >= data.len() {
            break;
        }
        let track_events = parse_track(data, &mut pos)?;

        // Convert tick-based events to sample-based MidiClip
        let clip = ticks_to_samples(&track_events, ticks_per_beat, sample_rate);
        clips.push(clip);
    }

    Ok(clips)
}

struct SmfHeader {
    format: u16,
    num_tracks: u16,
    division: u16,
}

struct TrackEvent {
    tick: u64,        // Absolute tick position
    message: MidiMessage,
}

fn parse_header(data: &[u8], pos: &mut usize) -> Result<SmfHeader, String> {
    if data.len() < *pos + 14 {
        return Err("File too short for MIDI header".into());
    }

    // "MThd"
    if &data[*pos..*pos + 4] != b"MThd" {
        return Err("Not a MIDI file (missing MThd)".into());
    }
    *pos += 4;

    let _length = read_u32_be(data, pos);
    let format = read_u16_be(data, pos);
    let num_tracks = read_u16_be(data, pos);
    let division = read_u16_be(data, pos);

    if division & 0x8000 != 0 {
        return Err("SMPTE time division not supported".into());
    }

    Ok(SmfHeader {
        format,
        num_tracks,
        division,
    })
}

fn parse_track(data: &[u8], pos: &mut usize) -> Result<Vec<TrackEvent>, String> {
    if data.len() < *pos + 8 {
        return Err("File too short for track chunk".into());
    }

    // "MTrk"
    if &data[*pos..*pos + 4] != b"MTrk" {
        return Err("Expected MTrk chunk".into());
    }
    *pos += 4;

    let chunk_len = read_u32_be(data, pos) as usize;
    let track_end = *pos + chunk_len;

    let mut events = Vec::new();
    let mut abs_tick: u64 = 0;
    let mut running_status: u8 = 0;

    while *pos < track_end && *pos < data.len() {
        // Read variable-length delta time
        let delta = read_vlq(data, pos)?;
        abs_tick += delta;

        if *pos >= data.len() {
            break;
        }

        let status_byte = data[*pos];

        // Meta event
        if status_byte == 0xFF {
            *pos += 1; // skip 0xFF
            if *pos >= data.len() {
                break;
            }
            let _meta_type = data[*pos];
            *pos += 1;
            let length = read_vlq(data, pos)? as usize;
            *pos += length; // skip meta data
            continue;
        }

        // SysEx event
        if status_byte == 0xF0 || status_byte == 0xF7 {
            *pos += 1;
            let length = read_vlq(data, pos)? as usize;
            *pos += length;
            continue;
        }

        // Channel message
        let (status, data_start) = if status_byte & 0x80 != 0 {
            running_status = status_byte;
            *pos += 1;
            (status_byte, *pos)
        } else {
            // Running status
            (running_status, *pos)
        };

        let msg_type = status & 0xF0;
        let channel = status & 0x0F;

        let message = match msg_type {
            0x80 => {
                // Note Off
                let note = data.get(data_start).copied().unwrap_or(0) & 0x7F;
                let velocity = data.get(data_start + 1).copied().unwrap_or(0) & 0x7F;
                *pos = data_start + 2;
                MidiMessage::NoteOff {
                    channel,
                    note,
                    velocity,
                }
            }
            0x90 => {
                // Note On
                let note = data.get(data_start).copied().unwrap_or(0) & 0x7F;
                let velocity = data.get(data_start + 1).copied().unwrap_or(0) & 0x7F;
                *pos = data_start + 2;
                if velocity == 0 {
                    MidiMessage::NoteOff {
                        channel,
                        note,
                        velocity: 0,
                    }
                } else {
                    MidiMessage::NoteOn {
                        channel,
                        note,
                        velocity,
                    }
                }
            }
            0xA0 => {
                // Polyphonic Aftertouch
                let note = data.get(data_start).copied().unwrap_or(0) & 0x7F;
                let pressure = data.get(data_start + 1).copied().unwrap_or(0) & 0x7F;
                *pos = data_start + 2;
                MidiMessage::PolyphonicAftertouch {
                    channel,
                    note,
                    pressure,
                }
            }
            0xB0 => {
                // Control Change
                let controller = data.get(data_start).copied().unwrap_or(0) & 0x7F;
                let value = data.get(data_start + 1).copied().unwrap_or(0) & 0x7F;
                *pos = data_start + 2;
                MidiMessage::ControlChange {
                    channel,
                    controller,
                    value,
                }
            }
            0xC0 => {
                // Program Change (1 data byte)
                let program = data.get(data_start).copied().unwrap_or(0) & 0x7F;
                *pos = data_start + 1;
                MidiMessage::ProgramChange { channel, program }
            }
            0xD0 => {
                // Channel Aftertouch (1 data byte)
                let pressure = data.get(data_start).copied().unwrap_or(0) & 0x7F;
                *pos = data_start + 1;
                MidiMessage::ChannelAftertouch { channel, pressure }
            }
            0xE0 => {
                // Pitch Bend
                let lsb = data.get(data_start).copied().unwrap_or(0) as u16;
                let msb = data.get(data_start + 1).copied().unwrap_or(0) as u16;
                *pos = data_start + 2;
                MidiMessage::PitchBend {
                    channel,
                    value: (msb << 7) | lsb,
                }
            }
            _ => {
                // Unknown — skip 2 bytes
                *pos = data_start + 2;
                continue;
            }
        };

        events.push(TrackEvent {
            tick: abs_tick,
            message,
        });
    }

    // Ensure we're at the end of the track chunk
    *pos = track_end;

    Ok(events)
}

/// Convert tick-based events to sample-based MidiClip.
/// Assumes 120 BPM default (standard for files without tempo events).
fn ticks_to_samples(events: &[TrackEvent], ticks_per_beat: f64, sample_rate: u32) -> MidiClip {
    // Default tempo: 120 BPM = 500000 microseconds per beat
    let tempo_us_per_beat = 500_000.0;
    let seconds_per_tick = (tempo_us_per_beat / 1_000_000.0) / ticks_per_beat;
    let samples_per_tick = seconds_per_tick * sample_rate as f64;

    let duration_ticks = events.last().map_or(0, |e| e.tick) + 1;
    let duration_samples = (duration_ticks as f64 * samples_per_tick) as u64;

    let mut clip = MidiClip::new(duration_samples.max(1));

    for event in events {
        let time_samples = (event.tick as f64 * samples_per_tick) as u64;
        clip.add_event(MidiEvent {
            time_samples,
            message: event.message.clone(),
        });
    }

    clip.sort();
    clip
}

fn read_u16_be(data: &[u8], pos: &mut usize) -> u16 {
    let val = ((data[*pos] as u16) << 8) | data[*pos + 1] as u16;
    *pos += 2;
    val
}

fn read_u32_be(data: &[u8], pos: &mut usize) -> u32 {
    let val = ((data[*pos] as u32) << 24)
        | ((data[*pos + 1] as u32) << 16)
        | ((data[*pos + 2] as u32) << 8)
        | data[*pos + 3] as u32;
    *pos += 4;
    val
}

fn read_vlq(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut value: u64 = 0;
    for _ in 0..4 {
        if *pos >= data.len() {
            return Err("Unexpected end of data in VLQ".into());
        }
        let byte = data[*pos];
        *pos += 1;
        value = (value << 7) | (byte & 0x7F) as u64;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("VLQ too long".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Type 0 SMF in memory for testing.
    fn build_test_smf() -> Vec<u8> {
        let mut data = Vec::new();

        // Header: MThd, length=6, format=0, tracks=1, division=480
        data.extend_from_slice(b"MThd");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes()); // format 0
        data.extend_from_slice(&1u16.to_be_bytes()); // 1 track
        data.extend_from_slice(&480u16.to_be_bytes()); // 480 ticks/beat

        // Track: MTrk
        let mut track_data = Vec::new();

        // Delta=0, Note On ch0, note 60, vel 100
        track_data.push(0x00); // delta
        track_data.push(0x90); // note on ch0
        track_data.push(60); // note
        track_data.push(100); // velocity

        // Delta=480 (1 beat), Note Off ch0, note 60, vel 0
        track_data.extend_from_slice(&[0x83, 0x60]); // VLQ for 480
        track_data.push(0x80); // note off ch0
        track_data.push(60);
        track_data.push(0);

        // Delta=0, End of Track meta event
        track_data.push(0x00);
        track_data.push(0xFF);
        track_data.push(0x2F);
        track_data.push(0x00);

        data.extend_from_slice(b"MTrk");
        data.extend_from_slice(&(track_data.len() as u32).to_be_bytes());
        data.extend_from_slice(&track_data);

        data
    }

    #[test]
    fn test_parse_smf_basic() {
        let data = build_test_smf();
        let clips = parse_smf(&data, 48000).unwrap();

        assert_eq!(clips.len(), 1);
        let clip = &clips[0];
        assert!(clip.events.len() >= 2, "Should have note on + note off");

        // First event: Note On at tick 0
        assert_eq!(clip.events[0].time_samples, 0);
        assert!(matches!(
            clip.events[0].message,
            MidiMessage::NoteOn { note: 60, velocity: 100, .. }
        ));

        // Second event: Note Off at tick 480 (1 beat at 120 BPM = 0.5 sec = 24000 samples)
        assert_eq!(clip.events[1].time_samples, 24000);
        assert!(matches!(
            clip.events[1].message,
            MidiMessage::NoteOff { note: 60, .. }
        ));
    }

    #[test]
    fn test_parse_smf_invalid() {
        let result = parse_smf(b"not a midi file", 48000);
        assert!(result.is_err());
    }

    #[test]
    fn test_vlq_parsing() {
        // 0x00 = 0
        let mut pos = 0;
        assert_eq!(read_vlq(&[0x00], &mut pos).unwrap(), 0);

        // 0x7F = 127
        pos = 0;
        assert_eq!(read_vlq(&[0x7F], &mut pos).unwrap(), 127);

        // 0x81 0x00 = 128
        pos = 0;
        assert_eq!(read_vlq(&[0x81, 0x00], &mut pos).unwrap(), 128);

        // 0x83 0x60 = 480
        pos = 0;
        assert_eq!(read_vlq(&[0x83, 0x60], &mut pos).unwrap(), 480);
    }
}
