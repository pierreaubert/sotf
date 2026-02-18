//! XTC Validation Benchmarks
//!
//! Measures performance of validation functions and cancellation depth calculations.
//!
//! Run with:
//!   cargo bench -p plugins --no-default-features -- xtc-validation

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sotf_plugins::validation::{
    measure_cancellation_depth_db, measure_cancellation_depth_spectrum, reference_ild_db,
    reference_itd_ms, run_validation, CANCELLATION_DEPTH_TARGETS,
};
use sotf_plugins::XtcPluginParams;

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
                let mut params = XtcPluginParams::default();
                params.speaker_angle_deg = a;
                params.distance_m = d;
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
);

criterion_main!(xtc_validation);
