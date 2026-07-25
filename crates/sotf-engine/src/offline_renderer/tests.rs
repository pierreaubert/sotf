use super::offline_render_config::OfflineRenderConfig;
use super::render::render_offline;
use super::render::render_passthrough;
use super::render::render_timeline;
use super::render_progress::RenderProgress;
use super::types::OutputFormat;
use crate::decoder::source::AudioSource;
use crate::engine::PluginConfig;
use std::path::Path;

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

fn create_constant_test_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    num_frames: usize,
    value: f32,
) {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..num_frames * usize::from(channels) {
        writer.write_sample(value).unwrap();
    }
    writer.finalize().unwrap();
}

fn create_impulse_test_wav(path: &Path, num_frames: usize, impulse_frames: &[usize]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for frame in 0..num_frames {
        writer
            .write_sample(if impulse_frames.contains(&frame) {
                1.0f32
            } else {
                0.0
            })
            .unwrap();
    }
    writer.finalize().unwrap();
}

fn convolution_plugin_config(ir_path: &Path) -> PluginConfig {
    PluginConfig {
        plugin_type: "convolution".into(),
        parameters: serde_json::json!({
            "ir_file": ir_path,
            "mix": 1.0,
            "gain_db": 0.0,
            "use_nupc": false,
            "zero_latency_head": false,
            "head_taps": 0
        }),
    }
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
fn test_offline_render_resamples_to_requested_rate_and_duration() {
    let dir = std::env::temp_dir().join("sotf_test_offline_resample");
    std::fs::create_dir_all(&dir).unwrap();
    let input_path = dir.join("input.wav");
    let output_path = dir.join("output.wav");
    create_test_wav(&input_path, 48_000, 2, 4_800);

    let mut config = OfflineRenderConfig::new(AudioSource::File(input_path), &output_path);
    config.output_sample_rate = Some(44_100);
    config.frame_size = 257;
    render_offline(&config, None).unwrap();

    let reader = hound::WavReader::open(&output_path).unwrap();
    assert_eq!(reader.spec().sample_rate, 44_100);
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.duration(), 4_410);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_offline_render_supports_integer_wav_depths_with_dither() {
    for bits_per_sample in [16, 24] {
        let dir = std::env::temp_dir().join(format!("sotf_test_offline_integer_{bits_per_sample}"));
        std::fs::create_dir_all(&dir).unwrap();
        let input_path = dir.join("silence.wav");
        let output_path = dir.join("output.wav");
        create_constant_test_wav(&input_path, 48_000, 1, 4_096, 0.0);

        let mut config = OfflineRenderConfig::new(AudioSource::File(input_path), &output_path);
        config.format = OutputFormat::Wav { bits_per_sample };
        render_offline(&config, None).unwrap();

        let reader = hound::WavReader::open(&output_path).unwrap();
        assert_eq!(reader.spec().bits_per_sample, bits_per_sample);
        assert_eq!(reader.spec().sample_format, hound::SampleFormat::Int);
        let codes: Vec<i32> = reader
            .into_samples::<i32>()
            .map(|sample| sample.unwrap())
            .collect();
        assert!(codes.iter().any(|&sample| sample < 0));
        assert!(codes.iter().any(|&sample| sample > 0));
        assert!(codes.iter().all(|&sample| (-1..=1).contains(&sample)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn test_offline_float_render_preserves_headroom() {
    let dir = std::env::temp_dir().join("sotf_test_offline_float_headroom");
    std::fs::create_dir_all(&dir).unwrap();
    let input_path = dir.join("input.wav");
    let output_path = dir.join("output.wav");
    create_constant_test_wav(&input_path, 48_000, 1, 128, 1.25);

    render_passthrough(&input_path, &output_path, 32).unwrap();

    let reader = hound::WavReader::open(&output_path).unwrap();
    let peak = reader
        .into_samples::<f32>()
        .map(|sample| sample.unwrap().abs())
        .fold(0.0f32, f32::max);
    assert!(peak > 1.2, "float output clipped headroom to {peak}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_offline_render_compensates_and_drains_plugin_latency() {
    let dir = std::env::temp_dir().join("sotf_test_offline_plugin_latency");
    std::fs::create_dir_all(&dir).unwrap();
    let input_path = dir.join("input.wav");
    let ir_path = dir.join("ir.wav");
    let output_path = dir.join("output.wav");
    let frames = 2_048;
    create_impulse_test_wav(&input_path, frames, &[0, frames - 1]);
    create_impulse_test_wav(&ir_path, 1, &[0]);

    let mut config = OfflineRenderConfig::new(AudioSource::File(input_path), &output_path);
    config.frame_size = 127;
    config.plugins.push(convolution_plugin_config(&ir_path));
    render_offline(&config, None).unwrap();

    let reader = hound::WavReader::open(&output_path).unwrap();
    assert_eq!(reader.duration(), frames as u32);
    let samples: Vec<f32> = reader
        .into_samples::<f32>()
        .map(|sample| sample.unwrap())
        .collect();
    let strongest: Vec<(usize, f32)> = samples
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, sample)| sample.abs() > 0.5)
        .collect();
    assert!(
        samples[0] > 0.9,
        "first impulse shifted to {}; strongest samples: {strongest:?}",
        samples[0]
    );
    assert!(
        samples[frames - 1] > 0.9,
        "final impulse was truncated to {}; strongest samples: {strongest:?}",
        samples[frames - 1],
    );
    assert_eq!(strongest.len(), 2, "unexpected impulses: {strongest:?}");
    assert_eq!(strongest[0].0, 0);
    assert_eq!(strongest[1].0, frames - 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_offline_render_rejects_invalid_configuration() {
    let dir = std::env::temp_dir().join("sotf_test_offline_invalid");
    std::fs::create_dir_all(&dir).unwrap();
    let input_path = dir.join("input.wav");
    create_test_wav(&input_path, 48_000, 1, 16);

    let mut config =
        OfflineRenderConfig::new(AudioSource::File(input_path), dir.join("output.wav"));
    config.frame_size = 0;
    assert!(
        render_offline(&config, None)
            .unwrap_err()
            .contains("frame_size")
    );

    config.frame_size = 16;
    config.output_sample_rate = Some(0);
    assert!(
        render_offline(&config, None)
            .unwrap_err()
            .contains("sample rate")
    );

    config.output_sample_rate = None;
    config.format = OutputFormat::Wav {
        bits_per_sample: 20,
    };
    assert!(
        render_offline(&config, None)
            .unwrap_err()
            .contains("Unsupported WAV depth")
    );
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
    assert_eq!(samples.len(), 4800);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_render_timeline_compensates_and_drains_master_latency() {
    use crate::engine::build_plugin_host;
    use crate::timeline::clip::{Clip, Region};
    use crate::timeline::timeline::Timeline;
    use crate::timeline::track::Track;

    let dir = std::env::temp_dir().join("sotf_test_render_timeline_latency");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("src.wav");
    let ir = dir.join("ir.wav");
    let out = dir.join("bounced.wav");
    let frames = 2_048;
    create_impulse_test_wav(&src, frames, &[0, frames - 1]);
    create_impulse_test_wav(&ir, 1, &[0]);

    let plugin = convolution_plugin_config(&ir);
    let mut timeline = Timeline::new(1, 48_000, 127);
    let mut track = Track::new("T1", 1, 48_000);
    track.add_region(Region::new(Clip::from_file(&src, frames as u64), 0));
    timeline.add_track(track);
    timeline.master_chain = build_plugin_host(std::slice::from_ref(&plugin), 48_000, 1)
        .unwrap()
        .0;
    timeline.master_plugin_configs.push(plugin);
    timeline.build().unwrap();

    timeline.transport.play();
    let mut dirty_output = vec![0.0; timeline.frame_size];
    for _ in 0..5 {
        timeline.process(&mut dirty_output).unwrap();
    }

    render_timeline(
        &mut timeline,
        &out,
        &OutputFormat::Wav {
            bits_per_sample: 32,
        },
        None,
    )
    .unwrap();

    let reader = hound::WavReader::open(&out).unwrap();
    assert_eq!(reader.duration(), frames as u32);
    let samples: Vec<f32> = reader
        .into_samples::<f32>()
        .map(|sample| sample.unwrap())
        .collect();
    let strongest: Vec<(usize, f32)> = samples
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, sample)| sample.abs() > 0.5)
        .collect();
    assert!(
        samples[0] > 0.9,
        "first impulse shifted to {}; strongest samples: {strongest:?}",
        samples[0]
    );
    assert!(
        samples[frames - 1] > 0.9,
        "final impulse was truncated to {}; strongest samples: {strongest:?}",
        samples[frames - 1],
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_render_timeline_aligns_parallel_track_latencies() {
    use crate::engine::build_plugin_host;
    use crate::timeline::clip::{Clip, Region};
    use crate::timeline::timeline::Timeline;
    use crate::timeline::track::Track;

    let dir = std::env::temp_dir().join("sotf_test_render_timeline_pdc");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("src.wav");
    let ir = dir.join("ir.wav");
    let out = dir.join("bounced.wav");
    let frames = 2_048;
    create_impulse_test_wav(&src, frames, &[0, frames - 1]);
    create_impulse_test_wav(&ir, 1, &[0]);

    let mut direct = Track::new("direct", 1, 48_000);
    direct.add_region(Region::new(Clip::from_file(&src, frames as u64), 0));

    let plugin = convolution_plugin_config(&ir);
    let mut latent = Track::new("latent", 1, 48_000);
    latent.add_region(Region::new(Clip::from_file(&src, frames as u64), 0));
    latent.chain = build_plugin_host(std::slice::from_ref(&plugin), 48_000, 1)
        .unwrap()
        .0;
    latent.plugin_configs.push(plugin);

    let mut timeline = Timeline::new(1, 48_000, 127);
    timeline.add_track(direct);
    timeline.add_track(latent);
    timeline.build().unwrap();

    render_timeline(
        &mut timeline,
        &out,
        &OutputFormat::Wav {
            bits_per_sample: 32,
        },
        None,
    )
    .unwrap();

    let reader = hound::WavReader::open(&out).unwrap();
    assert_eq!(reader.duration(), frames as u32);
    let samples: Vec<f32> = reader
        .into_samples::<f32>()
        .map(|sample| sample.unwrap())
        .collect();
    assert!(
        (samples[0] - 2.0).abs() < 1e-4,
        "track impulses were not aligned at start: {}",
        samples[0]
    );
    assert!(
        (samples[frames - 1] - 2.0).abs() < 1e-4,
        "track impulses were not aligned at end: {}",
        samples[frames - 1]
    );
    assert!(
        samples[1..frames - 1]
            .iter()
            .all(|sample| sample.abs() < 1e-4),
        "parallel path compensation introduced shifted impulses"
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
