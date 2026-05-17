// ============================================================================
// MidiTrack — A MIDI track with instrument plugin and sequenced regions
// ============================================================================

use super::track::Track;
use crate::engine::PluginConfig;
use sotf_audio_player_midi::MidiMessage;
use sotf_audio_player_midi::sequencer::MidiRegion;
use sotf_plugins::{DawHost, ProcessContext};

/// A MIDI note/controller event scheduled within a rendered audio block.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteEvent {
    pub sample_offset: u32,
    pub channel: u8,
    pub kind: NoteEventKind,
}

/// MIDI event kinds consumed by timeline instrument plugins.
#[derive(Debug, Clone, PartialEq)]
pub enum NoteEventKind {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    ControlChange { controller: u8, value: u8 },
    PitchBend { value: i16 },
}

/// Instrument plugin interface for timeline MIDI tracks.
pub trait InstrumentPlugin: Send {
    fn info(&self) -> sotf_plugins::PluginInfo;
    fn output_channels(&self) -> usize;
    fn parameters(&self) -> Vec<sotf_plugins::parameters::Parameter>;
    fn set_parameter(
        &mut self,
        id: sotf_plugins::parameters::ParameterId,
        value: sotf_plugins::parameters::ParameterValue,
    ) -> sotf_plugins::PluginResult<()>;
    fn get_parameter(
        &self,
        id: &sotf_plugins::parameters::ParameterId,
    ) -> Option<sotf_plugins::parameters::ParameterValue>;
    fn process_events(
        &mut self,
        events: &[NoteEvent],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> sotf_plugins::PluginResult<usize>;
    fn reset(&mut self);
}

/// A MIDI track on the timeline.
///
/// Contains MIDI regions, an instrument plugin (synthesizer) that generates audio,
/// and an optional effect chain (DawHost) for post-processing.
pub struct MidiTrack {
    /// Track name
    pub name: String,
    /// MIDI regions on this track
    pub regions: Vec<MidiRegion>,
    /// Instrument plugin that converts MIDI events to audio
    pub instrument: Box<dyn InstrumentPlugin>,
    /// Serializable instrument config used to rebuild `instrument`.
    pub instrument_config: Option<PluginConfig>,
    /// Post-instrument effect chain
    pub chain: DawHost,
    /// Serializable plugin configs used to rebuild `chain`.
    pub plugin_configs: Vec<PluginConfig>,
    /// Track volume (linear)
    pub volume: f32,
    /// Track pan (-1.0 to 1.0)
    pub pan: f32,
    /// Muted
    pub muted: bool,
    /// Soloed
    pub solo: bool,
    /// Pre-allocated event buffer for the current block
    event_buf: Vec<NoteEvent>,
    /// Pre-allocated instrument output buffer
    instrument_buf: Vec<f32>,
    /// Pre-allocated chain output buffer
    chain_output: Vec<f32>,
}

impl MidiTrack {
    pub fn new(
        name: impl Into<String>,
        instrument: Box<dyn InstrumentPlugin>,
        sample_rate: u32,
    ) -> Self {
        let out_ch = instrument.output_channels();
        Self {
            name: name.into(),
            regions: Vec::new(),
            instrument,
            instrument_config: None,
            chain: DawHost::new(out_ch, sample_rate),
            plugin_configs: Vec::new(),
            volume: 1.0,
            pan: 0.0,
            muted: false,
            solo: false,
            event_buf: Vec::new(),
            instrument_buf: Vec::new(),
            chain_output: Vec::new(),
        }
    }

    /// Add a MIDI region to this track.
    pub fn add_region(&mut self, region: MidiRegion) {
        self.regions.push(region);
    }

    /// Build the effect chain.
    pub fn build(&mut self) -> Result<(), String> {
        self.chain.build()
    }

    /// Output channels of this track (from the instrument).
    pub fn output_channels(&self) -> usize {
        self.instrument.output_channels()
    }

    /// Render one block of audio at the given timeline position.
    pub fn render_block(
        &mut self,
        position_samples: u64,
        num_frames: usize,
        sample_rate: u32,
        output: &mut [f32],
    ) -> Result<usize, String> {
        let out_ch = self.instrument.output_channels();
        let total_samples = num_frames * out_ch;

        // Collect MIDI events from all overlapping regions
        self.event_buf.clear();
        for region in &self.regions {
            if !region.overlaps(position_samples, num_frames as u64) {
                continue;
            }
            let events = region.events_in_timeline_range(position_samples, num_frames as u64);
            for (relative_time, msg) in events {
                let kind = match msg {
                    MidiMessage::NoteOn { note, velocity, .. } => NoteEventKind::NoteOn {
                        note: *note,
                        velocity: *velocity,
                    },
                    MidiMessage::NoteOff { note, .. } => NoteEventKind::NoteOff { note: *note },
                    MidiMessage::ControlChange {
                        controller, value, ..
                    } => NoteEventKind::ControlChange {
                        controller: *controller,
                        value: *value,
                    },
                    MidiMessage::PitchBend { value, .. } => NoteEventKind::PitchBend {
                        value: *value as i16 - 8192,
                    },
                    _ => continue,
                };
                self.event_buf.push(NoteEvent {
                    sample_offset: relative_time as u32,
                    channel: 0,
                    kind,
                });
            }
        }

        // Stable ordering at the same sample offset preserves MIDI clip/region order.
        self.event_buf.sort_by_key(|e| e.sample_offset);

        // Process through instrument
        if self.instrument_buf.len() < total_samples {
            self.instrument_buf.resize(total_samples, 0.0);
        }
        self.instrument_buf[..total_samples].fill(0.0);

        let context = ProcessContext {
            sample_rate,
            num_frames,
        };
        let frames = self.instrument.process_events(
            &self.event_buf,
            &mut self.instrument_buf[..total_samples],
            &context,
        )?;

        // Process through effect chain
        if self.chain.plugin_count() > 0 {
            let chain_out_ch = self.chain.output_channels();
            let chain_out_len = frames * chain_out_ch;
            if self.chain_output.len() < chain_out_len {
                self.chain_output.resize(chain_out_len, 0.0);
            }
            let out_frames = self.chain.process(
                &self.instrument_buf[..frames * out_ch],
                &mut self.chain_output[..chain_out_len],
            )?;
            let copy_len = (out_frames * chain_out_ch).min(output.len());
            output[..copy_len].copy_from_slice(&self.chain_output[..copy_len]);
            Track::apply_volume_pan(&mut output[..copy_len], chain_out_ch, self.volume, self.pan);
            Ok(out_frames)
        } else {
            let copy_len = (frames * out_ch).min(output.len());
            output[..copy_len].copy_from_slice(&self.instrument_buf[..copy_len]);
            Track::apply_volume_pan(&mut output[..copy_len], out_ch, self.volume, self.pan);
            Ok(frames)
        }
    }
}

// ============================================================================
// Simple test synthesizer
// ============================================================================

/// A basic sine wave synthesizer for testing MIDI playback.
/// Supports up to 16 simultaneous voices with per-note tracking.
pub struct TestSynth {
    sample_rate: f32,
    channels: usize,
    voices: Vec<Voice>,
    amplitude: f32,
}

struct Voice {
    note: u8,
    frequency: f32,
    phase: f32,
    active: bool,
    velocity: f32,
}

impl TestSynth {
    pub fn new(channels: usize, sample_rate: u32) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            channels,
            voices: Vec::new(),
            amplitude: 0.3,
        }
    }

    fn note_to_freq(note: u8) -> f32 {
        440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
    }
}

impl InstrumentPlugin for TestSynth {
    fn info(&self) -> sotf_plugins::PluginInfo {
        sotf_plugins::PluginInfo::new("TestSynth", "0.1", "test")
    }

    fn output_channels(&self) -> usize {
        self.channels
    }

    fn parameters(&self) -> Vec<sotf_plugins::parameters::Parameter> {
        vec![]
    }

    fn set_parameter(
        &mut self,
        _: sotf_plugins::parameters::ParameterId,
        _: sotf_plugins::parameters::ParameterValue,
    ) -> sotf_plugins::PluginResult<()> {
        Err("no parameters".into())
    }

    fn get_parameter(
        &self,
        _: &sotf_plugins::parameters::ParameterId,
    ) -> Option<sotf_plugins::parameters::ParameterValue> {
        None
    }

    fn process_events(
        &mut self,
        events: &[NoteEvent],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> sotf_plugins::PluginResult<usize> {
        let nf = context.num_frames;
        let ch = self.channels;

        // Process sample-by-sample, applying events at their sample offsets
        let mut event_idx = 0;

        for frame in 0..nf {
            // Apply events at this sample offset
            while event_idx < events.len() && events[event_idx].sample_offset <= frame as u32 {
                match &events[event_idx].kind {
                    NoteEventKind::NoteOn { note, velocity } => {
                        // Reuse existing voice or add new one
                        let existing = self.voices.iter_mut().find(|v| v.note == *note);
                        if let Some(v) = existing {
                            v.active = true;
                            v.velocity = *velocity as f32 / 127.0;
                        } else {
                            self.voices.push(Voice {
                                note: *note,
                                frequency: Self::note_to_freq(*note),
                                phase: 0.0,
                                active: true,
                                velocity: *velocity as f32 / 127.0,
                            });
                        }
                    }
                    NoteEventKind::NoteOff { note } => {
                        if let Some(v) = self.voices.iter_mut().find(|v| v.note == *note) {
                            v.active = false;
                        }
                    }
                    _ => {}
                }
                event_idx += 1;
            }

            // Generate audio from active voices
            let mut sample = 0.0f32;
            for voice in &mut self.voices {
                if voice.active {
                    sample += (voice.phase * 2.0 * std::f32::consts::PI).sin()
                        * voice.velocity
                        * self.amplitude;
                    voice.phase += voice.frequency / self.sample_rate;
                    if voice.phase >= 1.0 {
                        voice.phase -= 1.0;
                    }
                }
            }

            // Write to all channels
            let offset = frame * ch;
            for c in 0..ch {
                if offset + c < output.len() {
                    output[offset + c] = sample;
                }
            }
        }

        // Remove inactive voices
        self.voices.retain(|v| v.active);

        Ok(nf)
    }

    fn reset(&mut self) {
        self.voices.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio_player_midi::sequencer::{MidiClip, MidiEvent, MidiRegion};

    #[test]
    fn test_synth_silent_without_notes() {
        let mut synth = TestSynth::new(1, 48000);
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 256,
        };
        let mut output = vec![0.0f32; 256];
        synth.process_events(&[], &mut output, &ctx).unwrap();
        for &s in &output {
            assert!(s.abs() < 1e-6, "No notes = silence");
        }
    }

    #[test]
    fn test_synth_produces_audio_on_note() {
        let mut synth = TestSynth::new(1, 48000);
        let events = vec![NoteEvent {
            sample_offset: 0,
            channel: 0,
            kind: NoteEventKind::NoteOn {
                note: 69, // A4 = 440Hz
                velocity: 127,
            },
        }];
        let ctx = ProcessContext {
            sample_rate: 48000,
            num_frames: 256,
        };
        let mut output = vec![0.0f32; 256];
        synth.process_events(&events, &mut output, &ctx).unwrap();

        // Should have non-zero audio
        let rms: f32 = (output.iter().map(|s| s * s).sum::<f32>() / 256.0).sqrt();
        assert!(rms > 0.01, "Note should produce audio, RMS={rms}");
    }

    #[test]
    fn test_midi_track_renders_notes() {
        let synth = TestSynth::new(1, 48000);
        let mut track = MidiTrack::new("Synth", Box::new(synth), 48000);

        let mut clip = MidiClip::new(48000);
        clip.add_event(MidiEvent::note_on(0, 0, 60, 100)); // C4 at start
        clip.add_event(MidiEvent::note_off(24000, 0, 60)); // Off at 500ms
        clip.sort();

        track.add_region(MidiRegion::new(clip, 0));
        track.build().unwrap();

        let mut output = vec![0.0f32; 1024];
        track.render_block(0, 1024, 48000, &mut output).unwrap();

        // Should have non-zero output (C4 note is playing)
        let rms: f32 = (output.iter().map(|s| s * s).sum::<f32>() / 1024.0).sqrt();
        assert!(rms > 0.01, "MIDI track should produce audio, RMS={rms}");
    }

    #[test]
    fn test_midi_track_silence_after_note_off() {
        let synth = TestSynth::new(1, 48000);
        let mut track = MidiTrack::new("Synth", Box::new(synth), 48000);

        let mut clip = MidiClip::new(48000);
        clip.add_event(MidiEvent::note_on(0, 0, 60, 100));
        clip.add_event(MidiEvent::note_off(512, 0, 60)); // Off after 512 samples
        clip.sort();

        track.add_region(MidiRegion::new(clip, 0));
        track.build().unwrap();

        // First block: note plays for 512 samples then stops
        let mut output1 = vec![0.0f32; 1024];
        track.render_block(0, 1024, 48000, &mut output1).unwrap();

        // Second block: should be silent (note is off)
        let mut output2 = vec![0.0f32; 1024];
        track.render_block(1024, 1024, 48000, &mut output2).unwrap();

        let rms2: f32 = (output2.iter().map(|s| s * s).sum::<f32>() / 1024.0).sqrt();
        assert!(rms2 < 0.001, "After note off, should be silent, RMS={rms2}");
    }

    #[test]
    fn test_midi_track_preserves_same_sample_event_order() {
        let synth = TestSynth::new(1, 48000);
        let mut track = MidiTrack::new("Synth", Box::new(synth), 48000);

        let mut clip = MidiClip::new(1024);
        clip.add_event(MidiEvent::note_off(0, 0, 60));
        clip.add_event(MidiEvent::note_on(0, 0, 60, 100));
        clip.sort();

        track.add_region(MidiRegion::new(clip, 0));
        track.build().unwrap();

        let mut output = vec![0.0f32; 1024];
        track.render_block(0, 1024, 48000, &mut output).unwrap();

        let rms: f32 = (output.iter().map(|s| s * s).sum::<f32>() / 1024.0).sqrt();
        assert!(
            rms > 0.01,
            "same-sample note-off then note-on order should leave the note active, RMS={rms}"
        );
    }
}
