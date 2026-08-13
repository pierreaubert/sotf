//! XTC Validation Benchmarks
//!
//! Measures performance of validation functions and cancellation depth calculations.
//!
//! Run with:
//!   cargo bench -p plugins --no-default-features -- xtc-validation

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_xtc::validation::{
    CANCELLATION_DEPTH_TARGETS, measure_cancellation_depth_db, measure_cancellation_depth_spectrum,
    reference_ild_db, reference_itd_ms, run_validation,
};
use sotf_plugin_xtc::{XtcPlugin, XtcPluginParams};
use std::hint::black_box;
use std::time::Duration;

fn matrix_fixture(output_channels: usize) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "xtc-process-benchmark-{}-{output_channels}.json",
        std::process::id()
    ));
    let speakers: Vec<String> = (0..output_channels).map(|ch| format!("S{ch}")).collect();
    let mut filters = Vec::with_capacity(output_channels * 2);
    for speaker in &speakers {
        for (ear, gain) in [("left_ear", 0.5), ("right_ear", 0.5)] {
            filters.push(serde_json::json!({
                "speaker": speaker,
                "target_ear": ear,
                "taps": [gain]
            }));
        }
    }
    let artifact = serde_json::json!({
        "version": "ctc-recommended-v1",
        "source": "benchmark",
        "sample_rate": 48_000,
        "speakers": speakers,
        "ears": ["left_ear", "right_ear"],
        "filters": filters
    });
    std::fs::write(&path, serde_json::to_vec(&artifact).unwrap()).unwrap();
    path
}

fn bench_streaming_process(c: &mut Criterion) {
    let mut group = c.benchmark_group("xtc_streaming_process");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(2));

    for output_channels in [2, 3, 8, 16] {
        let path = matrix_fixture(output_channels);
        let params = XtcPluginParams {
            source_mode: "roomeq_recommended".into(),
            recommended_matrix_file: Some(path.to_string_lossy().into_owned()),
            auto_gain_enabled: false,
            ..Default::default()
        };
        let mut plugin = XtcPlugin::new(params, 48_000).unwrap();
        plugin.initialize(48_000).unwrap();
        let frames = 512;
        let input = vec![0.125_f32; frames * 2];
        let mut output = vec![0.0_f32; frames * output_channels];
        let context = ProcessContext::new(48_000, frames);
        group.bench_with_input(
            BenchmarkId::new("outputs", output_channels),
            &output_channels,
            |b, _| {
                b.iter(|| {
                    plugin
                        .process(black_box(&input), black_box(&mut output), &context)
                        .unwrap()
                });
            },
        );
        std::fs::remove_file(path).unwrap();
    }

    for frames in [128, 512, 2048] {
        let mut plugin = XtcPlugin::new(XtcPluginParams::default(), 48_000).unwrap();
        plugin.initialize(48_000).unwrap();
        let input = vec![0.125_f32; frames * 2];
        let mut output = vec![0.0_f32; frames * 2];
        let context = ProcessContext::new(48_000, frames);
        group.bench_with_input(BenchmarkId::new("block_frames", frames), &frames, |b, _| {
            b.iter(|| {
                plugin
                    .process(black_box(&input), black_box(&mut output), &context)
                    .unwrap()
            });
        });
    }
    group.finish();
}

fn bench_reference_itd(c: &mut Criterion) {
    let mut group = c.benchmark_group("xtc_reference_itd");

    for angle in [15.0, 30.0, 45.0, 60.0, 75.0, 90.0] {
        group.bench_with_input(
            BenchmarkId::new("angle_deg", angle as i32),
            &angle,
            |b, &angle| {
                b.iter(|| black_box(reference_itd_ms(angle, 0.0875)));
            },
        );
    }
    group.finish();
}

fn bench_reference_ild(c: &mut Criterion) {
    let mut group = c.benchmark_group("xtc_reference_ild");

    for freq in [250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0] {
        group.bench_with_input(
            BenchmarkId::new("freq_hz", freq as i32),
            &freq,
            |b, &freq| {
                b.iter(|| black_box(reference_ild_db(freq, 30.0, 0.0875)));
            },
        );
    }
    group.finish();
}

fn bench_cancellation_depth_single_freq(c: &mut Criterion) {
    let params = XtcPluginParams::default();
    let sample_rate = 48000;

    let mut group = c.benchmark_group("xtc_cancellation_depth_single");

    for &(freq, _, _) in CANCELLATION_DEPTH_TARGETS {
        group.bench_with_input(
            BenchmarkId::new("freq_hz", freq as i32),
            &freq,
            |b, &freq| {
                b.iter(|| black_box(measure_cancellation_depth_db(&params, sample_rate, freq)));
            },
        );
    }
    group.finish();
}

fn bench_cancellation_depth_spectrum(c: &mut Criterion) {
    let params = XtcPluginParams::default();
    let sample_rate = 48000;

    c.bench_function("xtc_cancellation_spectrum_7_points", |b| {
        let freqs = [100.0, 200.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];
        b.iter(|| {
            black_box(measure_cancellation_depth_spectrum(
                &params,
                sample_rate,
                &freqs,
            ))
        });
    });
}

fn bench_full_validation_suite(c: &mut Criterion) {
    let params = XtcPluginParams::default();

    c.bench_function("xtc_full_validation_default", |b| {
        b.iter(|| black_box(run_validation(&params, 48000)));
    });
}

fn bench_validation_with_varying_geometry(c: &mut Criterion) {
    let mut group = c.benchmark_group("xtc_validation_geometry");

    let configs = [
        ("30deg_2m", 30.0, 2.0),
        ("45deg_1.5m", 45.0, 1.5),
        ("60deg_1m", 60.0, 1.0),
    ];

    for (name, angle, distance) in configs {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(angle, distance),
            |b, &(a, d)| {
                let params = XtcPluginParams {
                    speaker_angle_deg: a,
                    distance_m: d,
                    ..Default::default()
                };
                b.iter(|| black_box(run_validation(&params, 48000)));
            },
        );
    }
    group.finish();
}

criterion_group!(
    xtc_validation,
    bench_reference_itd,
    bench_reference_ild,
    bench_cancellation_depth_single_freq,
    bench_cancellation_depth_spectrum,
    bench_full_validation_suite,
    bench_validation_with_varying_geometry,
    bench_streaming_process,
);

criterion_main!(xtc_validation);
