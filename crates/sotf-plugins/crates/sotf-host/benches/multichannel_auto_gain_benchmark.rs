use criterion::{Criterion, criterion_group, criterion_main};
use sotf_host::{AutoGainLoudnessType, AutoGainParams, MultichannelAutoGain};

fn enabled_params() -> AutoGainParams {
    AutoGainParams {
        enabled: true,
        loudness_type: AutoGainLoudnessType::Momentary,
        max_gain_db: 12.0,
        smoothing_ms: 50.0,
    }
}

fn benchmark_multichannel_auto_gain(c: &mut Criterion) {
    let mut group = c.benchmark_group("multichannel_auto_gain");

    for &frames in &[512, 1024, 4096] {
        group.bench_function(format!("5_1_{}frames", frames), |b| {
            let mut mag = MultichannelAutoGain::new(48000, enabled_params()).unwrap();
            let cfg = sotf_host::speaker_config::get_speaker_config("5.1").unwrap();
            let mut output = vec![0.5_f32; frames * cfg.total_channels];
            let input: Vec<f32> = (0..frames * 2)
                .map(|i| (i as f32 * 0.01).sin() * 0.4)
                .collect();
            mag.measure_input(&input).unwrap();

            // Warm up to avoid the first-frame allocation in the measurement.
            mag.measure_and_apply(&mut output, frames, cfg.total_channels, cfg)
                .unwrap();

            b.iter_custom(|iters| {
                let start = std::time::Instant::now();
                for _ in 0..iters {
                    mag.measure_and_apply(
                        std::hint::black_box(&mut output),
                        frames,
                        cfg.total_channels,
                        cfg,
                    )
                    .unwrap();
                }
                start.elapsed()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, benchmark_multichannel_auto_gain);
criterion_main!(benches);
