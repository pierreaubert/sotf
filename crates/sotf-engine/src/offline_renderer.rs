// ============================================================================
// Offline Audio Renderer — Processes audio at max CPU speed without cpal
// ============================================================================
//
// Used for: bounce-to-disk, batch processing, headless server rendering.
// Single-threaded synchronous pipeline: decode → process → write.

use crate::decoder::core::DecodedAudio;
use crate::decoder::source::AudioSource;
use crate::engine::{PluginConfig, build_plugin_host};
use std::path::{Path, PathBuf};

/// Output format for offline rendering.
#[derive(Debug, Clone)]
pub enum OutputFormat {
    /// WAV file with specified bits per sample (16, 24, or 32-bit float)
    Wav { bits_per_sample: u16 },
}

/// Configuration for offline rendering.
#[derive(Debug, Clone)]
pub struct OfflineRenderConfig {
    /// Audio source to render
    pub source: AudioSource,
    /// Output file path
    pub output_path: PathBuf,
    /// Output format
    pub format: OutputFormat,
    /// Plugin chain to apply during rendering
    pub plugins: Vec<PluginConfig>,
    /// Output sample rate (None = use source sample rate)
    pub output_sample_rate: Option<u32>,
    /// Processing block size in frames (default 1024)
    pub frame_size: usize,
}

impl OfflineRenderConfig {
    pub fn new(source: AudioSource, output_path: impl Into<PathBuf>) -> Self {
        Self {
            source,
            output_path: output_path.into(),
            format: OutputFormat::Wav {
                bits_per_sample: 32,
            },
            plugins: Vec::new(),
            frame_size: 1024,
            output_sample_rate: None,
        }
    }
}

/// Progress information passed to the callback during rendering.
#[derive(Debug, Clone)]
pub struct RenderProgress {
    /// Number of frames processed so far
    pub frames_processed: u64,
    /// Total frames in the source (if known)
    pub total_frames: Option<u64>,
}

impl RenderProgress {
    /// Returns completion percentage (0.0 to 100.0) if total is known.
    pub fn percent(&self) -> Option<f32> {
        self.total_frames
            .map(|t| (self.frames_processed as f32 / t.max(1) as f32) * 100.0)
    }
}

struct TimelineRenderStateGuard<'a> {
    timeline: &'a mut crate::timeline::Timeline,
    saved_loop: Option<(u64, u64)>,
}

impl<'a> TimelineRenderStateGuard<'a> {
    fn new(timeline: &'a mut crate::timeline::Timeline) -> Self {
        let saved_loop = timeline.transport.loop_range.take();
        timeline.seek(0);
        timeline.transport.play();
        Self {
            timeline,
            saved_loop,
        }
    }
}

impl Drop for TimelineRenderStateGuard<'_> {
    fn drop(&mut self) {
        self.timeline.transport.pause();
        self.timeline.transport.loop_range = self.saved_loop.take();
    }
}

/// Render audio offline at maximum CPU speed.
///
/// Decodes the source, processes through the plugin chain, and writes to the
/// output file. No real-time constraints — runs as fast as the CPU allows.
///
/// # Arguments
/// * `config` — Render configuration (source, output, plugins, format)
/// * `on_progress` — Optional callback invoked after each decoded chunk
pub fn render_offline(
    config: &OfflineRenderConfig,
    mut on_progress: Option<&mut dyn FnMut(&RenderProgress)>,
) -> Result<(), String> {
    // 1. Create decoder
    let mut decoder = crate::decoder::core::create_decoder_from_source(&config.source)
        .map_err(|e| format!("Failed to open source: {e}"))?;
    let spec = decoder.spec().clone();

    let sample_rate = config.output_sample_rate.unwrap_or(spec.sample_rate);
    let input_channels = spec.channels as usize;

    // 2. Build plugin host
    let (mut host, _warnings) = build_plugin_host(&config.plugins, sample_rate, input_channels)?;
    host.build()?;
    let output_channels = host.output_channels();

    // 3. Create WAV writer
    let wav_spec = match config.format {
        OutputFormat::Wav { bits_per_sample } => hound::WavSpec {
            channels: output_channels as u16,
            sample_rate,
            bits_per_sample: if bits_per_sample == 32 {
                32
            } else {
                bits_per_sample
            },
            sample_format: if bits_per_sample == 32 {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        },
    };
    let mut writer = hound::WavWriter::create(&config.output_path, wav_spec)
        .map_err(|e| format!("Failed to create output file: {e}"))?;

    // 4. Pre-allocate buffers
    let frame_size = config.frame_size;
    let mut decode_buf = DecodedAudio::new(spec.clone());
    let mut process_output = vec![0.0f32; frame_size * output_channels * 2];
    let mut frames_processed: u64 = 0;

    // 5. Decode → process → write loop
    loop {
        decode_buf.clear();
        let decoded_frames = decoder
            .decode_into(&mut decode_buf)
            .map_err(|e| format!("Decode error: {e}"))?;

        if decoded_frames == 0 {
            break;
        }

        let samples = &decode_buf.samples;

        // Process in frame_size chunks
        let mut offset = 0;
        while offset < decoded_frames {
            let chunk_frames = (decoded_frames - offset).min(frame_size);
            let in_start = offset * input_channels;
            let in_end = in_start + chunk_frames * input_channels;
            let out_len = chunk_frames * output_channels;

            if process_output.len() < out_len {
                process_output.resize(out_len, 0.0);
            }

            let actual_frames =
                host.process(&samples[in_start..in_end], &mut process_output[..out_len])?;

            // Write to WAV
            let write_samples = actual_frames * output_channels;
            match wav_spec.sample_format {
                hound::SampleFormat::Float => {
                    for &s in &process_output[..write_samples] {
                        writer
                            .write_sample(s.clamp(-1.0, 1.0))
                            .map_err(|e| format!("Write error: {e}"))?;
                    }
                }
                hound::SampleFormat::Int => {
                    let scale = ((1i64 << (wav_spec.bits_per_sample - 1)) - 1) as f32;
                    for &s in &process_output[..write_samples] {
                        let i = (s.clamp(-1.0, 1.0) * scale) as i32;
                        writer
                            .write_sample(i)
                            .map_err(|e| format!("Write error: {e}"))?;
                    }
                }
            }

            frames_processed += actual_frames as u64;
            offset += chunk_frames;
        }

        // Progress callback
        if let Some(ref mut cb) = on_progress {
            cb(&RenderProgress {
                frames_processed,
                total_frames: spec.total_frames,
            });
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize output: {e}"))?;
    Ok(())
}

/// Convenience: render a file with no plugins (passthrough / format conversion).
pub fn render_passthrough(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    bits_per_sample: u16,
) -> Result<(), String> {
    let config = OfflineRenderConfig {
        source: AudioSource::File(input_path.as_ref().to_path_buf()),
        output_path: output_path.as_ref().to_path_buf(),
        format: OutputFormat::Wav { bits_per_sample },
        plugins: Vec::new(),
        frame_size: 1024,
        output_sample_rate: None,
    };
    render_offline(&config, None)
}

/// Render a Timeline to a WAV file at max CPU speed.
///
/// Processes the entire timeline from start to end (or loop if enabled)
/// and writes the output to the specified path.
pub fn render_timeline(
    timeline: &mut crate::timeline::Timeline,
    output_path: impl AsRef<Path>,
    format: &OutputFormat,
    mut on_progress: Option<&mut dyn FnMut(&RenderProgress)>,
) -> Result<(), String> {
    let sr = timeline.transport.sample_rate;
    let ch = timeline.output_channels;
    let nf = timeline.frame_size;
    let duration = timeline.duration_samples();

    let wav_spec = match format {
        OutputFormat::Wav { bits_per_sample } => hound::WavSpec {
            channels: ch as u16,
            sample_rate: sr,
            bits_per_sample: if *bits_per_sample == 32 {
                32
            } else {
                *bits_per_sample
            },
            sample_format: if *bits_per_sample == 32 {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        },
    };

    let mut writer = hound::WavWriter::create(output_path.as_ref(), wav_spec)
        .map_err(|e| format!("Failed to create output: {e}"))?;

    // Disable looping for bounce — we render exactly once through the timeline.
    let mut render_state = TimelineRenderStateGuard::new(timeline);

    let mut output = vec![0.0f32; nf * ch];
    let mut frames_processed: u64 = 0;

    while frames_processed < duration {
        let frames = render_state.timeline.process(&mut output)?;
        let write_samples = frames * ch;

        match wav_spec.sample_format {
            hound::SampleFormat::Float => {
                for &s in &output[..write_samples] {
                    writer
                        .write_sample(s.clamp(-1.0, 1.0))
                        .map_err(|e| format!("Write error: {e}"))?;
                }
            }
            hound::SampleFormat::Int => {
                let scale = ((1i64 << (wav_spec.bits_per_sample - 1)) - 1) as f32;
                for &s in &output[..write_samples] {
                    let i = (s.clamp(-1.0, 1.0) * scale) as i32;
                    writer
                        .write_sample(i)
                        .map_err(|e| format!("Write error: {e}"))?;
                }
            }
        }

        frames_processed += frames as u64;
        if let Some(ref mut cb) = on_progress {
            cb(&RenderProgress {
                frames_processed,
                total_frames: Some(duration),
            });
        }
    }

    writer
        .finalize()
        .map_err(|e| format!("Finalize error: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_wav(path: &Path, sample_rate: u32, channels: u16, num_frames: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for frame in 0..num_frames {
            let val = (frame as f32 / num_frames as f32) * 2.0 - 1.0; // ramp -1..1
            for _ in 0..channels {
                writer.write_sample(val).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn test_offline_render_passthrough() {
        let dir = std::env::temp_dir().join("sotf_test_offline");
        std::fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("input.wav");
        let output_path = dir.join("output_passthrough.wav");

        let sr = 48000;
        let ch = 2;
        let frames = 4800; // 100ms
        create_test_wav(&input_path, sr, ch, frames);

        render_passthrough(&input_path, &output_path, 32).unwrap();

        // Verify output exists and has correct spec
        let reader = hound::WavReader::open(&output_path).unwrap();
        let out_spec = reader.spec();
        assert_eq!(out_spec.sample_rate, sr);
        assert_eq!(out_spec.channels, ch);

        // Verify sample count matches
        let out_samples: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(
            out_samples.len(),
            frames * ch as usize,
            "Output should have same number of samples as input"
        );

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_offline_render_with_gain() {
        let dir = std::env::temp_dir().join("sotf_test_offline_gain");
        std::fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("input.wav");
        let output_path = dir.join("output_gain.wav");

        let sr = 48000;
        let ch = 2;
        let frames = 480;
        create_test_wav(&input_path, sr, ch, frames);

        let config = OfflineRenderConfig {
            source: AudioSource::File(input_path.clone()),
            output_path: output_path.clone(),
            format: OutputFormat::Wav {
                bits_per_sample: 32,
            },
            plugins: vec![PluginConfig {
                plugin_type: "gain".to_string(),
                parameters: serde_json::json!({ "gain_db": -6.0 }),
            }],
            frame_size: 256,
            output_sample_rate: None,
        };

        render_offline(&config, None).unwrap();

        // Read input and output
        let in_reader = hound::WavReader::open(&input_path).unwrap();
        let in_samples: Vec<f32> = in_reader
            .into_samples::<f32>()
            .map(|s| s.unwrap())
            .collect();

        let out_reader = hound::WavReader::open(&output_path).unwrap();
        let out_samples: Vec<f32> = out_reader
            .into_samples::<f32>()
            .map(|s| s.unwrap())
            .collect();

        assert_eq!(in_samples.len(), out_samples.len());

        // -6dB ≈ 0.5012 gain. Output should be roughly half the input amplitude.
        // Skip first few samples (gain smoother ramp-up) and check the tail.
        let gain_linear = 10.0f32.powf(-6.0 / 20.0);
        let check_start = (frames * ch as usize) / 2; // check second half
        for i in check_start..in_samples.len() {
            let expected = in_samples[i] * gain_linear;
            assert!(
                (out_samples[i] - expected).abs() < 0.05,
                "Sample {i}: expected ~{expected:.4}, got {:.4}",
                out_samples[i]
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_offline_render_progress() {
        let dir = std::env::temp_dir().join("sotf_test_offline_progress");
        std::fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("input.wav");
        let output_path = dir.join("output_progress.wav");

        create_test_wav(&input_path, 48000, 1, 9600);

        let config = OfflineRenderConfig::new(AudioSource::File(input_path), &output_path);

        let mut progress_calls = 0u32;
        let mut last_frames = 0u64;

        render_offline(
            &config,
            Some(&mut |p: &RenderProgress| {
                progress_calls += 1;
                assert!(
                    p.frames_processed >= last_frames,
                    "Progress should be monotonically increasing"
                );
                last_frames = p.frames_processed;
                assert!(p.total_frames.is_some());
            }),
        )
        .unwrap();

        assert!(
            progress_calls > 0,
            "Progress callback should have been called"
        );
        assert!(last_frames > 0, "Should have processed frames");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_render_timeline() {
        use crate::timeline::clip::{Clip, Region};
        use crate::timeline::timeline::Timeline;
        use crate::timeline::track::Track;

        let dir = std::env::temp_dir().join("sotf_test_render_timeline");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        let out = dir.join("bounced.wav");
        create_test_wav(&src, 48000, 1, 4800);

        let mut tl = Timeline::new(1, 48000, 1024);
        let mut t = Track::new("T1", 1, 48000);
        t.add_region(Region::new(Clip::from_file(&src, 4800), 0));
        tl.add_track(t);
        tl.build().unwrap();

        let fmt = OutputFormat::Wav {
            bits_per_sample: 32,
        };
        render_timeline(&mut tl, &out, &fmt, None).unwrap();

        let reader = hound::WavReader::open(&out).unwrap();
        assert_eq!(reader.spec().sample_rate, 48000);
        let samples: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
        assert!(
            samples.len() >= 4800,
            "Should have at least 4800 samples, got {}",
            samples.len()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_timeline_restores_loop_range_when_progress_panics() {
        use crate::timeline::clip::{Clip, Region};
        use crate::timeline::timeline::Timeline;
        use crate::timeline::track::Track;

        let dir = std::env::temp_dir().join("sotf_test_render_timeline_panic");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.wav");
        let out = dir.join("bounced.wav");
        create_test_wav(&src, 48000, 1, 4800);

        let mut tl = Timeline::new(1, 48000, 1024);
        let mut t = Track::new("T1", 1, 48000);
        t.add_region(Region::new(Clip::from_file(&src, 4800), 0));
        tl.add_track(t);
        tl.transport.loop_range = Some((128, 256));
        tl.build().unwrap();

        let fmt = OutputFormat::Wav {
            bits_per_sample: 32,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            render_timeline(
                &mut tl,
                &out,
                &fmt,
                Some(&mut |_p: &RenderProgress| panic!("progress panic")),
            )
        }));

        assert!(result.is_err());
        assert_eq!(tl.transport.loop_range, Some((128, 256)));
        assert!(!tl.transport.playing);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
