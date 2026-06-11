#![allow(clippy::module_inception, clippy::needless_range_loop)]
// ============================================================================
// Timeline integration tests
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::decoder::source::AudioSource;
    use crate::timeline::clip::{Clip, Region};
    use crate::timeline::timeline::Timeline;
    use crate::timeline::track::Track;
    use std::path::Path;

    /// Create a test WAV file with a constant DC value per channel.
    fn create_dc_wav(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        num_frames: usize,
        dc: f32,
    ) -> hound::Result<()> {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec)?;
        for _ in 0..num_frames {
            for _ in 0..channels {
                writer.write_sample(dc)?;
            }
        }
        writer.finalize()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sotf_timeline_test_{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_single_track_playback() {
        let dir = temp_dir("single_track");
        let wav_path = dir.join("dc.wav");
        let sr = 48000;
        let ch = 2;
        let frames = 4800;
        let dc = 0.5;
        create_dc_wav(&wav_path, sr, ch, frames, dc).unwrap();

        let mut timeline = Timeline::new(ch as usize, sr, 1024);

        let mut track = Track::new("Track 1", ch as usize, sr);
        let clip = Clip::from_file(&wav_path, frames as u64);
        track.add_region(Region::new(clip, 0));
        timeline.add_track(track);

        timeline.build().unwrap();
        timeline.transport.play();

        // Process enough blocks to cover the clip
        let mut all_output = Vec::new();
        let blocks_needed = frames.div_ceil(1024);
        for _ in 0..blocks_needed {
            let mut output = vec![0.0f32; 1024 * ch as usize];
            timeline.process(&mut output).unwrap();
            all_output.extend_from_slice(&output);
        }

        // Check that output contains the DC value (within tolerance for decoder precision)
        let check_start = ch as usize; // skip first frame (decoder warmup)
        let check_end = frames * ch as usize;
        for i in check_start..check_end.min(all_output.len()) {
            assert!(
                (all_output[i] - dc).abs() < 0.01,
                "Sample {i}: expected ~{dc}, got {}",
                all_output[i]
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_multi_track_mixing() {
        let dir = temp_dir("multi_track");
        let wav1 = dir.join("track1.wav");
        let wav2 = dir.join("track2.wav");
        let sr = 48000;
        let ch: u16 = 1;
        let frames = 2048;
        create_dc_wav(&wav1, sr, ch, frames, 0.3).unwrap();
        create_dc_wav(&wav2, sr, ch, frames, 0.4).unwrap();

        let mut timeline = Timeline::new(ch as usize, sr, 1024);

        let mut t1 = Track::new("T1", ch as usize, sr);
        t1.add_region(Region::new(Clip::from_file(&wav1, frames as u64), 0));
        timeline.add_track(t1);

        let mut t2 = Track::new("T2", ch as usize, sr);
        t2.add_region(Region::new(Clip::from_file(&wav2, frames as u64), 0));
        timeline.add_track(t2);

        timeline.build().unwrap();
        timeline.transport.play();

        let mut output = vec![0.0f32; 1024];
        timeline.process(&mut output).unwrap();

        // Output should be sum of both tracks: 0.3 + 0.4 = 0.7
        let check_start = 1;
        for i in check_start..1024 {
            assert!(
                (output[i] - 0.7).abs() < 0.02,
                "Sample {i}: expected ~0.7, got {}",
                output[i]
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_mute_track() {
        let dir = temp_dir("mute");
        let wav = dir.join("audio.wav");
        let sr = 48000;
        create_dc_wav(&wav, sr, 1, 2048, 1.0).unwrap();

        let mut timeline = Timeline::new(1, sr, 1024);

        let mut track = Track::new("Muted", 1, sr);
        track.muted = true;
        track.add_region(Region::new(Clip::from_file(&wav, 2048), 0));
        timeline.add_track(track);

        timeline.build().unwrap();
        timeline.transport.play();

        let mut output = vec![0.0f32; 1024];
        timeline.process(&mut output).unwrap();

        // Muted track should produce silence
        for (i, &s) in output.iter().enumerate() {
            assert!(
                s.abs() < 1e-6,
                "Sample {i}: muted track should be silent, got {s}"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_solo_track() {
        let dir = temp_dir("solo");
        let wav1 = dir.join("t1.wav");
        let wav2 = dir.join("t2.wav");
        let sr = 48000;
        create_dc_wav(&wav1, sr, 1, 2048, 0.5).unwrap();
        create_dc_wav(&wav2, sr, 1, 2048, 0.8).unwrap();

        let mut timeline = Timeline::new(1, sr, 1024);

        let mut t1 = Track::new("T1", 1, sr);
        t1.solo = true; // Only T1 should be heard
        t1.add_region(Region::new(Clip::from_file(&wav1, 2048), 0));
        timeline.add_track(t1);

        let mut t2 = Track::new("T2", 1, sr);
        t2.add_region(Region::new(Clip::from_file(&wav2, 2048), 0));
        timeline.add_track(t2);

        timeline.build().unwrap();
        timeline.transport.play();

        let mut output = vec![0.0f32; 1024];
        timeline.process(&mut output).unwrap();

        // Only T1 (0.5) should be heard, T2 (0.8) silenced by solo
        for i in 1..1024 {
            assert!(
                (output[i] - 0.5).abs() < 0.02,
                "Sample {i}: solo should play only T1 (0.5), got {}",
                output[i]
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clip_offset_on_timeline() {
        // Place a clip starting at sample 1024 (not at 0)
        let dir = temp_dir("offset");
        let wav = dir.join("audio.wav");
        let sr = 48000;
        create_dc_wav(&wav, sr, 1, 2048, 0.9).unwrap();

        let mut timeline = Timeline::new(1, sr, 1024);

        let mut track = Track::new("T1", 1, sr);
        track.add_region(Region::new(Clip::from_file(&wav, 2048), 1024));
        timeline.add_track(track);

        timeline.build().unwrap();
        timeline.transport.play();

        // First block (0..1024): clip hasn't started yet → silence
        let mut output1 = vec![0.0f32; 1024];
        timeline.process(&mut output1).unwrap();
        for (i, &s) in output1.iter().enumerate() {
            assert!(
                s.abs() < 1e-6,
                "Block 1, sample {i}: should be silent, got {s}"
            );
        }

        // Second block (1024..2048): clip is playing → DC
        let mut output2 = vec![0.0f32; 1024];
        timeline.process(&mut output2).unwrap();
        for i in 1..1024 {
            assert!(
                (output2[i] - 0.9).abs() < 0.02,
                "Block 2, sample {i}: expected ~0.9, got {}",
                output2[i]
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_clip_fade_in() {
        let dir = temp_dir("fade_in");
        let wav = dir.join("audio.wav");
        let sr = 48000;
        let frames = 4800;
        create_dc_wav(&wav, sr, 1, frames, 1.0).unwrap();

        let block_size = 1024;
        let mut timeline = Timeline::new(1, sr, block_size);

        let mut track = Track::new("T1", 1, sr);
        let mut clip = Clip::from_file(&wav, frames as u64);
        clip.fade_in_samples = 2400; // Fade-in over first half
        track.add_region(Region::new(clip, 0));
        timeline.add_track(track);

        timeline.build().unwrap();
        timeline.transport.play();

        // Process enough blocks and collect all output
        let mut all_output = Vec::new();
        let blocks_needed = frames.div_ceil(block_size);
        for _ in 0..blocks_needed {
            let mut output = vec![0.0f32; block_size];
            timeline.process(&mut output).unwrap();
            all_output.extend_from_slice(&output);
        }

        // First sample should be near 0 (fade start)
        assert!(all_output[0].abs() < 0.01, "Fade start should be ~0");
        // Midpoint of fade (sample 1200): ~0.5
        assert!(
            (all_output[1200] - 0.5).abs() < 0.05,
            "Fade midpoint should be ~0.5, got {}",
            all_output[1200]
        );
        // After fade (sample 2500): ~1.0
        assert!(
            (all_output[2500] - 1.0).abs() < 0.02,
            "After fade should be ~1.0, got {}",
            all_output[2500]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_transport_loop_with_timeline() {
        let dir = temp_dir("loop");
        let wav = dir.join("audio.wav");
        let sr = 48000;
        create_dc_wav(&wav, sr, 1, 4096, 0.6).unwrap();

        let mut timeline = Timeline::new(1, sr, 1024);

        let mut track = Track::new("T1", 1, sr);
        track.add_region(Region::new(Clip::from_file(&wav, 4096), 0));
        timeline.add_track(track);

        timeline.build().unwrap();
        timeline.transport.play();
        timeline.transport.set_loop(Some((0, 2048))); // Loop first 2048 samples

        // Process 3 blocks (3072 samples), which should loop back
        for _ in 0..3 {
            let mut output = vec![0.0f32; 1024];
            timeline.process(&mut output).unwrap();
        }

        // After 3 blocks of 1024 with loop at 2048:
        // Block 0: pos 0→1024
        // Block 1: pos 1024→2048
        // Block 2: pos wraps to 0→1024 (loop)
        assert_eq!(timeline.transport.position_samples, 1024);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_timeline_duration() {
        let mut timeline = Timeline::new(2, 48000, 1024);

        let mut t1 = Track::new("T1", 2, 48000);
        t1.add_region(Region::new(Clip::new(AudioSource::Driver, 48000), 0));
        timeline.add_track(t1);

        let mut t2 = Track::new("T2", 2, 48000);
        t2.add_region(Region::new(Clip::new(AudioSource::Driver, 24000), 48000));
        timeline.add_track(t2);

        // T1: [0, 48000), T2: [48000, 72000)
        assert_eq!(timeline.duration_samples(), 72000);
        assert!((timeline.duration_seconds() - 1.5).abs() < 1e-6);
    }
}
