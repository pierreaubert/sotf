use crate::decoder::source::AudioSource;
use crate :: engine :: { PluginConfig } ;
use std :: path :: { Path } ;
use super::offline_render_config::OfflineRenderConfig;
use super::render::render_offline;
use super::render::render_passthrough;
use super::render::render_timeline;
use super::render_progress::RenderProgress;
use super::types::OutputFormat;

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

