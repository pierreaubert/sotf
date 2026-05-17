// ============================================================================
// TimelineProcessor — Wraps a Timeline to produce AudioFrames for the engine
// ============================================================================
//
// This bridges the Timeline (which processes blocks of audio internally) with
// the engine's AudioFrame-based pipeline. It can be used:
// 1. As a direct audio source (replacing the decoder thread)
// 2. With the offline renderer for timeline bounce
// 3. For real-time playback via a dedicated thread

use super::timeline::Timeline;
use crate::engine::AudioFrame;

/// Wraps a Timeline and generates AudioFrames for consumption by the engine.
///
/// The processor manages the Timeline's transport and produces frames
/// on demand, suitable for both real-time and offline processing.
pub struct TimelineProcessor {
    /// The timeline being processed
    pub timeline: Timeline,
    /// Pre-allocated output buffer
    output_buf: Vec<f32>,
}

impl TimelineProcessor {
    pub fn new(timeline: Timeline) -> Self {
        let buf_size = timeline.frame_size * timeline.output_channels;
        Self {
            timeline,
            output_buf: vec![0.0; buf_size],
        }
    }

    /// Process one block and return it as an AudioFrame.
    ///
    /// Returns None if the transport is not playing or has reached the end.
    pub fn next_frame(&mut self) -> Result<Option<AudioFrame>, String> {
        let mut frame = AudioFrame {
            data: Vec::with_capacity(self.timeline.frame_size * self.timeline.output_channels),
            num_frames: 0,
            num_channels: self.timeline.output_channels,
            sample_rate: self.timeline.transport.sample_rate,
        };
        if self.next_frame_into(&mut frame)? {
            Ok(Some(frame))
        } else {
            Ok(None)
        }
    }

    /// Process one block into a caller-owned frame buffer.
    ///
    /// Returns `Ok(false)` when the transport is not playing or has reached the
    /// end. Reusing the same `AudioFrame` across calls keeps its data allocation
    /// hot for offline rendering or direct engine integration.
    pub fn next_frame_into(&mut self, frame: &mut AudioFrame) -> Result<bool, String> {
        if !self.timeline.transport.playing {
            return Ok(false);
        }

        // Check if we've passed the end of all content
        let duration = self.timeline.duration_samples();
        if duration > 0 && self.timeline.transport.position_samples >= duration {
            // Check if looping — if not, we're done
            if self.timeline.transport.loop_range.is_none() {
                return Ok(false);
            }
        }

        let nf = self.timeline.frame_size;
        let ch = self.timeline.output_channels;
        let total = nf * ch;

        if self.output_buf.len() < total {
            self.output_buf.resize(total, 0.0);
        }

        let frames = self.timeline.process(&mut self.output_buf[..total])?;
        let samples = frames * ch;

        if frame.data.len() < samples {
            frame.data.resize(samples, 0.0);
        } else {
            frame.data.truncate(samples);
        }
        frame.data[..samples].copy_from_slice(&self.output_buf[..samples]);
        frame.num_frames = frames;
        frame.num_channels = ch;
        frame.sample_rate = self.timeline.transport.sample_rate;

        Ok(true)
    }

    /// Process the entire timeline and collect all frames.
    /// Useful for offline bounce.
    pub fn render_all(&mut self) -> Result<Vec<f32>, String> {
        self.timeline.transport.seek(0);
        self.timeline.transport.play();

        let capacity = self
            .timeline
            .duration_samples()
            .saturating_mul(self.timeline.output_channels as u64)
            .min(usize::MAX as u64) as usize;
        let mut all_samples = Vec::with_capacity(capacity);
        let mut frame = AudioFrame {
            data: Vec::with_capacity(self.timeline.frame_size * self.timeline.output_channels),
            num_frames: 0,
            num_channels: self.timeline.output_channels,
            sample_rate: self.timeline.transport.sample_rate,
        };
        while self.next_frame_into(&mut frame)? {
            all_samples.extend_from_slice(&frame.data);
        }
        Ok(all_samples)
    }

    /// Render the timeline to a WAV file.
    pub fn render_to_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        let sr = self.timeline.transport.sample_rate;
        let ch = self.timeline.output_channels;

        let spec = hound::WavSpec {
            channels: ch as u16,
            sample_rate: sr,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = hound::WavWriter::create(path, spec)
            .map_err(|e| format!("Failed to create file: {e}"))?;

        self.timeline.seek(0);
        self.timeline.transport.play();

        let mut frame = AudioFrame {
            data: Vec::with_capacity(self.timeline.frame_size * self.timeline.output_channels),
            num_frames: 0,
            num_channels: self.timeline.output_channels,
            sample_rate: self.timeline.transport.sample_rate,
        };
        while self.next_frame_into(&mut frame)? {
            for &s in &frame.data {
                writer
                    .write_sample(s.clamp(-1.0f32, 1.0))
                    .map_err(|e| format!("Write error: {e}"))?;
            }
        }

        writer
            .finalize()
            .map_err(|e| format!("Finalize error: {e}"))?;
        Ok(())
    }

    /// Get the current transport position.
    pub fn position_samples(&self) -> u64 {
        self.timeline.transport.position_samples
    }

    /// Check if playback has finished.
    pub fn is_finished(&self) -> bool {
        let duration = self.timeline.duration_samples();
        if duration == 0 {
            return true;
        }
        if self.timeline.transport.loop_range.is_some() {
            return false; // Looping never finishes
        }
        self.timeline.transport.position_samples >= duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::clip::{Clip, Region};
    use crate::timeline::track::Track;

    fn create_dc_wav(path: &std::path::Path, sr: u32, ch: u16, frames: usize, dc: f32) {
        let spec = hound::WavSpec {
            channels: ch,
            sample_rate: sr,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..frames {
            for _ in 0..ch {
                w.write_sample(dc).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn test_timeline_processor_renders_to_file() {
        let dir = std::env::temp_dir().join("sotf_test_tl_processor");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        let out = dir.join("out.wav");
        create_dc_wav(&src, 48000, 1, 4800, 0.7);

        let mut tl = Timeline::new(1, 48000, 1024);
        let mut track = Track::new("T1", 1, 48000);
        track.add_region(Region::new(Clip::from_file(&src, 4800), 0));
        tl.add_track(track);
        tl.build().unwrap();

        let mut proc = TimelineProcessor::new(tl);
        proc.render_to_file(&out).unwrap();

        // Verify output
        let reader = hound::WavReader::open(&out).unwrap();
        assert_eq!(reader.spec().sample_rate, 48000);
        let samples: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
        assert!(samples.len() >= 4800);
        // Check DC value (skip first sample for decoder warmup)
        for &s in &samples[1..4800] {
            assert!((s - 0.7).abs() < 0.02, "Expected ~0.7, got {s}");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_timeline_processor_render_all() {
        let dir = std::env::temp_dir().join("sotf_test_tl_render_all");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        create_dc_wav(&src, 48000, 1, 2048, 0.4);

        let mut tl = Timeline::new(1, 48000, 1024);
        let mut track = Track::new("T1", 1, 48000);
        track.add_region(Region::new(Clip::from_file(&src, 2048), 0));
        tl.add_track(track);
        tl.build().unwrap();

        let mut proc = TimelineProcessor::new(tl);
        let samples = proc.render_all().unwrap();

        assert!(samples.len() >= 2048);
        assert!(proc.is_finished());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn next_frame_into_reuses_frame_allocation() {
        let dir = std::env::temp_dir().join("sotf_test_tl_next_frame_into");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        create_dc_wav(&src, 48000, 1, 2048, 0.4);

        let mut tl = Timeline::new(1, 48000, 1024);
        let mut track = Track::new("T1", 1, 48000);
        track.add_region(Region::new(Clip::from_file(&src, 2048), 0));
        tl.add_track(track);
        tl.build().unwrap();
        tl.transport.play();

        let mut proc = TimelineProcessor::new(tl);
        let mut frame = AudioFrame {
            data: Vec::with_capacity(1024),
            num_frames: 0,
            num_channels: 1,
            sample_rate: 48000,
        };

        assert!(proc.next_frame_into(&mut frame).unwrap());
        let ptr = frame.data.as_ptr();
        assert!(proc.next_frame_into(&mut frame).unwrap());

        assert_eq!(frame.data.as_ptr(), ptr);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_timeline_processor_not_playing() {
        let tl = Timeline::new(2, 48000, 1024);
        let mut proc = TimelineProcessor::new(tl);
        // Not playing → returns None
        let frame = proc.next_frame().unwrap();
        assert!(frame.is_none());
    }
}
