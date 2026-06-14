use crate::plugin::{Plugin, ProcessContext};
use std::time::Instant;

/// A profiler for plugin performance measurements.
pub struct PerformanceProfiler {
    pub label: String,
    pub sample_rate: f64,
    pub channels: usize,
    pub block_size: usize,
}

impl PerformanceProfiler {
    pub fn new(label: &str, sample_rate: f64, channels: usize, block_size: usize) -> Self {
        Self {
            label: label.to_string(),
            sample_rate,
            channels,
            block_size,
        }
    }

    pub fn profile(&self, plugin: &mut dyn Plugin, duration_sec: f64) -> f64 {
        let total_frames = (duration_sec * self.sample_rate) as usize;
        let input = vec![0.1; total_frames * self.channels];
        let mut output = vec![0.0; total_frames * plugin.output_channels()];

        let start = Instant::now();
        let mut frames_processed = 0;
        while frames_processed < total_frames {
            let num_frames = (self.block_size).min(total_frames - frames_processed);
            let ctx = ProcessContext::new(self.sample_rate as u32, num_frames);

            let in_slice = &input
                [frames_processed * self.channels..(frames_processed + num_frames) * self.channels];
            let out_slice = &mut output[frames_processed * plugin.output_channels()
                ..(frames_processed + num_frames) * plugin.output_channels()];

            plugin.process(in_slice, out_slice, &ctx).unwrap();
            frames_processed += num_frames;
        }
        let duration = start.elapsed();
        let audio_duration_sec = total_frames as f64 / self.sample_rate;
        (duration.as_secs_f64() / audio_duration_sec) * 100.0
    }
}

/// Extensions for Criterion to easily benchmark plugins.
#[cfg(feature = "qa")]
pub fn benchmark_plugin_full(
    c: &mut criterion::Criterion,
    name: &str,
    mut plugin: Box<dyn Plugin>,
    sample_rate: f64,
) {
    use crate::detect_latency;
    let mut group = c.benchmark_group(name);

    // 1. Detect Latency
    let latency = detect_latency(plugin.as_mut(), sample_rate);
    println!("  [{}] Detected Latency: {} samples", name, latency);

    // 2. High-level Profiling (CPU usage %)
    let profiler = PerformanceProfiler::new(name, sample_rate, plugin.input_channels(), 512);
    let cpu = profiler.profile(plugin.as_mut(), 1.0);
    println!("  [{}] Estimated CPU Usage: {:.4}%", name, cpu);

    // 3. Micro-benchmarking (Criterion)
    let buffer_size = 512;
    let input = vec![0.1; buffer_size * plugin.input_channels()];
    let mut output = vec![0.0; buffer_size * plugin.output_channels()];
    let ctx = ProcessContext::new(sample_rate as u32, buffer_size);

    group.bench_function("process_512", |b: &mut criterion::Bencher| {
        b.iter(|| {
            plugin
                .process(
                    std::hint::black_box(&input),
                    std::hint::black_box(&mut output),
                    std::hint::black_box(&ctx),
                )
                .unwrap();
        })
    });

    group.finish();
}
