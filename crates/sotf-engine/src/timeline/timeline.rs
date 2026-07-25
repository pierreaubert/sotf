// ============================================================================
// Timeline — Multi-track arrangement with transport and master chain
// ============================================================================

use super::midi_track::MidiTrack;
use super::track::Track;
use super::transport::Transport;
use crate::engine::PluginConfig;
use sotf_plugins::DawHost;

struct PathDelay {
    samples: Vec<f32>,
    write_frame: usize,
    delay_frames: usize,
    channels: usize,
    active: bool,
    #[cfg(test)]
    clear_calls: usize,
}

impl PathDelay {
    fn new(delay_frames: usize, channels: usize) -> Self {
        Self {
            samples: vec![0.0; delay_frames.saturating_mul(channels)],
            write_frame: 0,
            delay_frames,
            channels,
            active: false,
            #[cfg(test)]
            clear_calls: 0,
        }
    }

    fn process(&mut self, block: &mut [f32]) {
        self.active = true;
        if self.delay_frames == 0 || self.channels == 0 {
            return;
        }
        for frame in block.chunks_exact_mut(self.channels) {
            let base = self.write_frame * self.channels;
            for (channel, sample) in frame.iter_mut().enumerate() {
                std::mem::swap(sample, &mut self.samples[base + channel]);
            }
            self.write_frame += 1;
            if self.write_frame == self.delay_frames {
                self.write_frame = 0;
            }
        }
    }

    fn clear(&mut self) {
        self.samples.fill(0.0);
        self.write_frame = 0;
        #[cfg(test)]
        {
            self.clear_calls += 1;
        }
    }

    fn deactivate(&mut self) {
        if self.active {
            self.clear();
            self.active = false;
        }
    }
}

/// A multi-track audio timeline with global transport and master plugin chain.
///
/// Processing flow:
/// 1. For each non-muted track (respecting solo):
///    - Decode overlapping clips at the current transport position
///    - Apply per-clip fades/gain
///    - Process through track's plugin chain
/// 2. Sum all track outputs into the mix bus
/// 3. Process through master plugin chain
/// 4. Advance transport
pub struct Timeline {
    /// Audio tracks
    pub tracks: Vec<Track>,
    /// MIDI tracks (instrument + sequencing)
    pub midi_tracks: Vec<MidiTrack>,
    /// Global transport state
    pub transport: Transport,
    /// Master plugin chain (applied after mixing all tracks)
    pub master_chain: DawHost,
    /// Serializable plugin configs used to rebuild `master_chain`.
    pub master_plugin_configs: Vec<PluginConfig>,
    /// Number of output channels (master bus)
    pub output_channels: usize,
    /// Processing block size in frames
    pub frame_size: usize,
    /// Pre-allocated track output buffer
    track_buf: Vec<f32>,
    /// Pre-allocated mix buffer (sum of all tracks)
    mix_buf: Vec<f32>,
    /// Per-path delays align all track chains before summing.
    audio_path_delays: Vec<PathDelay>,
    midi_path_delays: Vec<PathDelay>,
    maximum_track_latency: usize,
}

impl Timeline {
    pub fn new(output_channels: usize, sample_rate: u32, frame_size: usize) -> Self {
        let buf_size = frame_size * output_channels;
        Self {
            tracks: Vec::new(),
            midi_tracks: Vec::new(),
            transport: Transport::new(sample_rate),
            master_chain: DawHost::new(output_channels, sample_rate),
            master_plugin_configs: Vec::new(),
            output_channels,
            frame_size,
            track_buf: vec![0.0; buf_size],
            mix_buf: vec![0.0; buf_size],
            audio_path_delays: Vec::new(),
            midi_path_delays: Vec::new(),
            maximum_track_latency: 0,
        }
    }

    /// Add a track to the timeline.
    pub fn add_track(&mut self, track: Track) -> usize {
        let idx = self.tracks.len();
        self.tracks.push(track);
        idx
    }

    /// Add a MIDI track to the timeline.
    pub fn add_midi_track(&mut self, track: MidiTrack) -> usize {
        let idx = self.midi_tracks.len();
        self.midi_tracks.push(track);
        idx
    }

    /// Build all track chains and the master chain. Must be called before process().
    pub fn build(&mut self) -> Result<(), String> {
        for track in &mut self.tracks {
            track.build()?;
            track.prepare_render_buffers(self.frame_size);
        }
        for midi_track in &mut self.midi_tracks {
            midi_track.build()?;
        }
        self.master_chain.build()?;
        self.maximum_track_latency = self
            .tracks
            .iter()
            .map(|track| track.chain.total_latency_samples())
            .chain(
                self.midi_tracks
                    .iter()
                    .map(|track| track.chain.total_latency_samples()),
            )
            .max()
            .unwrap_or(0);
        self.audio_path_delays = self
            .tracks
            .iter()
            .map(|track| {
                PathDelay::new(
                    self.maximum_track_latency
                        .saturating_sub(track.chain.total_latency_samples()),
                    track.channels,
                )
            })
            .collect();
        self.midi_path_delays = self
            .midi_tracks
            .iter()
            .map(|track| {
                PathDelay::new(
                    self.maximum_track_latency
                        .saturating_sub(track.chain.total_latency_samples()),
                    track.output_channels(),
                )
            })
            .collect();
        Ok(())
    }

    /// Maximum active track latency plus the master-chain latency.
    ///
    /// Offline renderers use this to remove leading processing delay and drain
    /// the corresponding delayed program material after the timeline ends.
    pub fn output_latency_samples(&self) -> usize {
        self.maximum_track_latency
            .saturating_add(self.master_chain.total_latency_samples())
    }

    /// Process one block of audio from the timeline.
    ///
    /// Mixes all active tracks and processes through the master chain.
    /// The transport advances by `frame_size` samples if playing.
    ///
    /// `output` must have at least `frame_size * output_channels` samples.
    pub fn process(&mut self, output: &mut [f32]) -> Result<usize, String> {
        let nf = self.frame_size;
        let total = nf * self.output_channels;
        let pos = self.transport.position_samples;

        let any_solo =
            self.tracks.iter().any(|t| t.solo) || self.midi_tracks.iter().any(|t| t.solo);
        let has_active_audio = self
            .tracks
            .iter()
            .any(|t| !t.muted && (!any_solo || t.solo));
        let has_active_midi = self
            .midi_tracks
            .iter()
            .any(|t| !t.muted && (!any_solo || t.solo));
        let out_len = total.min(output.len());

        if !has_active_audio && !has_active_midi && self.master_chain.plugin_count() == 0 {
            for delay in &mut self.audio_path_delays {
                delay.deactivate();
            }
            for delay in &mut self.midi_path_delays {
                delay.deactivate();
            }
            output[..out_len].fill(0.0);
            self.transport.advance(nf);
            return Ok(nf);
        }

        // Ensure buffers
        if self.mix_buf.len() < total {
            self.mix_buf.resize(total, 0.0);
        }
        if self.track_buf.len() < total {
            self.track_buf.resize(total, 0.0);
        }
        self.mix_buf[..total].fill(0.0);

        // Render audio tracks
        for (track_index, track) in self.tracks.iter_mut().enumerate() {
            if track.muted || (any_solo && !track.solo) {
                if let Some(delay) = self.audio_path_delays.get_mut(track_index) {
                    delay.deactivate();
                }
                continue;
            }
            self.track_buf[..total].fill(0.0);
            track.render_block(pos, nf, &mut self.track_buf[..total])?;
            if let Some(delay) = self.audio_path_delays.get_mut(track_index) {
                let track_samples = nf.saturating_mul(track.channels).min(self.track_buf.len());
                delay.process(&mut self.track_buf[..track_samples]);
            }
            mix_with_channel_adapt(
                &self.track_buf,
                track.channels,
                &mut self.mix_buf,
                self.output_channels,
                nf,
            );
        }

        // Render MIDI tracks
        let sr = self.transport.sample_rate;
        for (track_index, midi_track) in self.midi_tracks.iter_mut().enumerate() {
            if midi_track.muted || (any_solo && !midi_track.solo) {
                if let Some(delay) = self.midi_path_delays.get_mut(track_index) {
                    delay.deactivate();
                }
                continue;
            }
            self.track_buf[..total].fill(0.0);
            midi_track.render_block(pos, nf, sr, &mut self.track_buf[..total])?;
            if let Some(delay) = self.midi_path_delays.get_mut(track_index) {
                let track_samples = nf
                    .saturating_mul(midi_track.output_channels())
                    .min(self.track_buf.len());
                delay.process(&mut self.track_buf[..track_samples]);
            }
            mix_with_channel_adapt(
                &self.track_buf,
                midi_track.output_channels(),
                &mut self.mix_buf,
                self.output_channels,
                nf,
            );
        }

        // Process through master chain
        if self.master_chain.plugin_count() > 0 {
            self.master_chain
                .process(&self.mix_buf[..total], &mut output[..out_len])?;
        } else {
            output[..out_len].copy_from_slice(&self.mix_buf[..out_len]);
        }

        // Advance transport
        self.transport.advance(nf);

        Ok(nf)
    }

    /// Seek all tracks to a new position (resets decoders).
    pub fn seek(&mut self, position_samples: u64) {
        self.transport.seek(position_samples);
        for track in &mut self.tracks {
            track.reset_decoders();
        }
        for track in &mut self.midi_tracks {
            track.reset_processing_state();
        }
        self.master_chain.reset();
        for delay in &mut self.audio_path_delays {
            delay.clear();
            delay.active = false;
        }
        for delay in &mut self.midi_path_delays {
            delay.clear();
            delay.active = false;
        }
    }

    /// Get the total duration of the timeline in samples (end of last region).
    pub fn duration_samples(&self) -> u64 {
        let audio_end = self
            .tracks
            .iter()
            .flat_map(|t| t.regions.iter())
            .map(|r| r.end_samples())
            .max()
            .unwrap_or(0);
        let midi_end = self
            .midi_tracks
            .iter()
            .flat_map(|t| t.regions.iter())
            .map(|r| r.end_samples())
            .max()
            .unwrap_or(0);
        audio_end.max(midi_end)
    }

    /// Get the total duration in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.duration_samples() as f64 / self.transport.sample_rate as f64
    }
}

/// Sum `src` (interleaved with `src_ch` channels) into `dst` (interleaved with `dst_ch` channels).
/// Handles mono→stereo (duplicate), stereo→mono (sum/2), and channel-matched cases.
fn mix_with_channel_adapt(
    src: &[f32],
    src_ch: usize,
    dst: &mut [f32],
    dst_ch: usize,
    num_frames: usize,
) {
    if src_ch == dst_ch {
        // Same channel count — direct sum
        let total = num_frames * dst_ch;
        for i in 0..total.min(src.len()).min(dst.len()) {
            dst[i] += src[i];
        }
    } else if src_ch == 1 && dst_ch == 2 {
        // Mono → Stereo: duplicate to both channels
        for f in 0..num_frames {
            if f < src.len() {
                let s = src[f];
                let dst_idx = f * 2;
                if dst_idx + 1 < dst.len() {
                    dst[dst_idx] += s;
                    dst[dst_idx + 1] += s;
                }
            }
        }
    } else if src_ch == 2 && dst_ch == 1 {
        // Stereo → Mono: average L+R
        for f in 0..num_frames {
            let src_idx = f * 2;
            if src_idx + 1 < src.len() && f < dst.len() {
                dst[f] += (src[src_idx] + src[src_idx + 1]) * 0.5;
            }
        }
    } else {
        // General case: copy min(src_ch, dst_ch) channels, zero-fill extra
        let copy_ch = src_ch.min(dst_ch);
        for f in 0..num_frames {
            for ch in 0..copy_ch {
                let src_idx = f * src_ch + ch;
                let dst_idx = f * dst_ch + ch;
                if src_idx < src.len() && dst_idx < dst.len() {
                    dst[dst_idx] += src[src_idx];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PathDelay, Timeline};
    use crate::timeline::track::Track;

    #[test]
    fn inactive_path_delay_clears_only_on_transition() {
        let mut delay = PathDelay::new(4_096, 8);
        let mut block = vec![1.0; 64 * 8];
        delay.process(&mut block);
        delay.deactivate();
        assert_eq!(delay.clear_calls, 1);

        for _ in 0..128 {
            delay.deactivate();
        }
        assert_eq!(delay.clear_calls, 1);
    }

    #[test]
    fn all_inactive_fast_path_clears_stale_track_delay_once() {
        let mut timeline = Timeline::new(1, 48_000, 8);
        timeline.add_track(Track::new("muted", 1, 48_000));
        timeline.audio_path_delays = vec![PathDelay::new(4, 1)];
        timeline.audio_path_delays[0].process(&mut [1.0]);
        timeline.tracks[0].muted = true;
        timeline.transport.play();

        let mut output = vec![1.0; 8];
        timeline.process(&mut output).unwrap();
        assert_eq!(output, vec![0.0; 8]);
        assert!(!timeline.audio_path_delays[0].active);
        assert!(
            timeline.audio_path_delays[0]
                .samples
                .iter()
                .all(|&sample| sample == 0.0)
        );
        assert_eq!(timeline.audio_path_delays[0].clear_calls, 1);

        timeline.process(&mut output).unwrap();
        assert_eq!(timeline.audio_path_delays[0].clear_calls, 1);
    }
}
