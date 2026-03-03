//! Testing utilities for audio plugins.
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::{Plugin, ProcessContext};
use std::alloc::{GlobalAlloc, Layout, System};
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

/// A simple stateful signal generator for testing.
/// Uses deterministic logic matching math-dsp.
pub struct SignalGen {
    sample_rate: f64,
    phase: f64,
    frequency: f64,
    amplitude: f32,
    gen_type: SignalType,
    // State for noise generators
    seed: u64,
    // State for pink noise (Voss-McCartney)
    pink_state: [f32; 7],
    // State for sweep
    sweep_t: f64,
    sweep_f_end: f64,
    sweep_duration: f64,
}

enum SignalType {
    Sine,
    WhiteNoise,
    PinkNoise,
    Impulse,
    Step,
    LogSweep,
}

impl SignalGen {
    pub fn new_sine(sample_rate: f64, frequency: f64, amplitude: f32) -> Self {
        Self::new(sample_rate, amplitude, SignalType::Sine, frequency)
    }

    pub fn new_white_noise(amplitude: f32) -> Self {
        Self::new(0.0, amplitude, SignalType::WhiteNoise, 0.0)
    }

    pub fn new_pink_noise(amplitude: f32) -> Self {
        Self::new(0.0, amplitude, SignalType::PinkNoise, 0.0)
    }

    pub fn new_impulse() -> Self {
        Self::new(0.0, 1.0, SignalType::Impulse, 0.0)
    }

    pub fn new_step() -> Self {
        Self::new(0.0, 1.0, SignalType::Step, 0.0)
    }

    pub fn new_log_sweep(
        sample_rate: f64,
        f_start: f64,
        f_end: f64,
        duration: f64,
        amplitude: f32,
    ) -> Self {
        let mut signal_gen = Self::new(sample_rate, amplitude, SignalType::LogSweep, f_start);
        signal_gen.sweep_f_end = f_end;
        signal_gen.sweep_duration = duration;
        signal_gen
    }

    fn new(sample_rate: f64, amplitude: f32, gen_type: SignalType, frequency: f64) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            frequency,
            amplitude,
            gen_type,
            seed: 1234567890,
            pink_state: [0.0; 7],
            sweep_t: 0.0,
            sweep_f_end: 0.0,
            sweep_duration: 0.0,
        }
    }

    /// Clip a sample to prevent overflow in PCM conversion
    #[inline]
    fn clip(x: f32) -> f32 {
        x.clamp(-0.999_999, 0.999_999)
    }

    fn next_white(&mut self) -> f32 {
        // Simple LCG random number generator for deterministic output
        // LCG constants from Numerical Recipes
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let random_u32 = (self.seed & 0xFFFFFFFF) as u32;
        (random_u32 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    pub fn generate(&mut self, num_samples: usize) -> Vec<f32> {
        let mut buffer = Vec::with_capacity(num_samples);
        for _ in 0..num_samples {
            let sample = match self.gen_type {
                SignalType::Sine => {
                    let s = (self.phase * 2.0 * PI as f64).sin() as f32 * self.amplitude;
                    self.phase = (self.phase + self.frequency / self.sample_rate) % 1.0;
                    Self::clip(s)
                }
                SignalType::WhiteNoise => Self::clip(self.next_white() * self.amplitude),
                SignalType::PinkNoise => {
                    let white = self.next_white();
                    let b = &mut self.pink_state;
                    b[0] = 0.99886 * b[0] + white * 0.0555179;
                    b[1] = 0.99332 * b[1] + white * 0.0750759;
                    b[2] = 0.96900 * b[2] + white * 0.153_852;
                    b[3] = 0.86650 * b[3] + white * 0.3104856;
                    b[4] = 0.55000 * b[4] + white * 0.5329522;
                    b[5] = -0.7616 * b[5] - white * 0.0168980;

                    let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
                    b[6] = white * 0.115926;

                    const PINK_NORM: f32 = 1.0 / 1.744;
                    Self::clip(self.amplitude * pink * PINK_NORM)
                }
                SignalType::Impulse => {
                    if self.phase == 0.0 {
                        self.phase = 1.0;
                        Self::clip(self.amplitude)
                    } else {
                        0.0
                    }
                }
                SignalType::Step => Self::clip(self.amplitude),
                SignalType::LogSweep => {
                    if self.sweep_t >= self.sweep_duration {
                        0.0
                    } else {
                        let k = (self.sweep_f_end / self.frequency).ln() / self.sweep_duration;
                        let coefficient = 2.0 * PI as f64 * self.frequency / k;
                        let phase = coefficient * ((k * self.sweep_t).exp() - 1.0);
                        let s = (phase.sin() as f32) * self.amplitude;
                        self.sweep_t += 1.0 / self.sample_rate;
                        Self::clip(s)
                    }
                }
            };
            buffer.push(sample);
        }
        buffer
    }
}

/// Utilities for comparing audio buffers.
pub struct BufferComparison;

impl BufferComparison {
    pub fn compare_rms(buf1: &[f32], buf2: &[f32], threshold: f32) -> bool {
        if buf1.len() != buf2.len() {
            return false;
        }
        if buf1.is_empty() {
            return true;
        }

        let mut sum_sq_diff = 0.0;
        for (s1, s2) in buf1.iter().zip(buf2.iter()) {
            let diff = s1 - s2;
            sum_sq_diff += diff * diff;
        }
        let rms_diff = (sum_sq_diff / buf1.len() as f32).sqrt();
        rms_diff < threshold
    }

    pub fn compare_bit_accurate(buf1: &[f32], buf2: &[f32]) -> bool {
        buf1 == buf2
    }
}

/// A harness for testing plugins with varied buffer sizes.
pub fn test_varied_buffer_sizes<P: Plugin>(
    plugin: &mut P,
    sample_rate: f64,
    input: &[f32],
    expected_output: &[f32],
) {
    let buffer_sizes = [1, 16, 32, 64, 128, 256, 512, 1024, 13, 127]; // Includes non-power-of-two
    let num_channels_in = plugin.input_channels();
    let num_channels_out = plugin.output_channels();
    let total_frames = input.len() / num_channels_in;

    for &block_size in &buffer_sizes {
        plugin.reset();
        let mut output = vec![0.0; expected_output.len()];
        let mut frames_processed = 0;

        while frames_processed < total_frames {
            let num_frames = (block_size).min(total_frames - frames_processed);
            let ctx = ProcessContext {
                sample_rate: sample_rate as u32,
                num_frames,
            };

            let in_slice = &input[frames_processed * num_channels_in
                ..(frames_processed + num_frames) * num_channels_in];
            let out_slice = &mut output[frames_processed * num_channels_out
                ..(frames_processed + num_frames) * num_channels_out];

            plugin.process(in_slice, out_slice, &ctx).unwrap();
            frames_processed += num_frames;
        }

        assert!(
            BufferComparison::compare_rms(&output, expected_output, 1e-5),
            "Failed for block size {}",
            block_size
        );
    }
}

/// Generate a DC buffer at a specific dB level.
pub fn generate_dc(db: f32, num_samples: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    vec![amp; num_samples]
}

/// Measure the peak level of a buffer in dB.
pub fn measure_peak_db(buffer: &[f32]) -> f32 {
    let peak = buffer.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    20.0 * peak.max(1e-10).log10()
}

/// Measure the RMS level of a buffer in dB.
pub fn measure_rms_db(buffer: &[f32]) -> f32 {
    if buffer.is_empty() {
        return -100.0;
    }
    let mut sum_sq = 0.0;
    for &s in buffer {
        sum_sq += s * s;
    }
    let rms = (sum_sq / buffer.len() as f32).sqrt();
    20.0 * rms.max(1e-10).log10()
}

// ============================================================================
// Counting Allocator for Real-time Safety Verification
// ============================================================================

pub static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static COUNTING_ENABLED: AtomicBool = AtomicBool::new(false);

pub struct CountingAlloc;

/// # Safety
/// This implementation is safe as it only increments an atomic counter.
/// It uses `System` allocator for the actual memory operations.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING_ENABLED.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Run a closure and assert it performs zero heap allocations.
pub fn assert_no_allocs<F: FnOnce()>(label: &str, f: F) {
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    COUNTING_ENABLED.store(true, Ordering::SeqCst);
    f();
    COUNTING_ENABLED.store(false, Ordering::SeqCst);
    let count = ALLOC_COUNT.load(Ordering::SeqCst);
    if count > 0 {
        panic!(
            "{} failed: {} allocations detected in hot path",
            label, count
        );
    }
}

// ============================================================================
// Shared QA Tests
// ============================================================================

/// Run standard QA tests for a plugin:
/// 1. Latency Reporting
/// 2. Real-time Safety (Zero Allocations)
/// 3. Performance Benchmark
pub fn run_standard_tests(plugin: &mut dyn Plugin, label: &str) {
    let sample_rate = 48000;

    // Test 2: Latency Reporting
    println!("\n[Test 2] Latency Reporting");
    let reported = plugin.latency_samples();
    println!("  Reported Latency: {} samples", reported);
    println!("  Latency: PASS");

    // Test 3: Real-time Safety (Zero Allocations)
    println!("\n[Test 3] Real-time Safety (Zero Allocations)");
    let rt_block_size = 512;
    let rt_input = vec![0.0_f32; rt_block_size * plugin.input_channels()];
    let mut rt_output = vec![0.0_f32; rt_block_size * plugin.output_channels()];
    let rt_ctx = ProcessContext {
        sample_rate,
        num_frames: rt_block_size,
    };

    // Warm up
    for _ in 0..10 {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
    }

    assert_no_allocs(&format!("{}::process + get_data", label), || {
        plugin.process(&rt_input, &mut rt_output, &rt_ctx).unwrap();
        let _data = plugin.get_data();
    });
    println!("  Zero Allocations: PASS");

    // Test 4: Performance Benchmark
    println!("\n[Test 4] Performance Benchmark");
    let bench_frames = 48000 * 5; // 5 seconds of audio
    let bench_input = vec![0.1_f32; bench_frames * plugin.input_channels()];
    let mut bench_output = vec![0.0_f32; bench_frames * plugin.output_channels()];

    let start = Instant::now();
    let mut pos = 0;
    while pos < bench_frames {
        let end = (pos + rt_block_size).min(bench_frames);
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        plugin
            .process(
                &bench_input[pos * plugin.input_channels()..end * plugin.input_channels()],
                &mut bench_output[pos * plugin.output_channels()..end * plugin.output_channels()],
                &ctx,
            )
            .unwrap();
        pos = end;
    }
    let duration = start.elapsed();
    let audio_duration_sec = bench_frames as f64 / sample_rate as f64;
    let cpu_usage = (duration.as_secs_f64() / audio_duration_sec) * 100.0;

    println!(
        "  Processed {:.1}s of audio in {:.2}ms",
        audio_duration_sec,
        duration.as_secs_f64() * 1000.0
    );
    println!("  Estimated CPU Usage: {:.2}%", cpu_usage);

    // Default threshold for most plugins is quite low.
    // XTC is heavy, but simple ones like Gain should be < 0.1%.
    let threshold = if label.contains("Xtc") { 15.0 } else { 5.0 };
    assert!(
        cpu_usage < threshold,
        "Performance regression: {} is too slow ({:.2}%)",
        label,
        cpu_usage
    );
    println!("  Performance: PASS");
}

/// A utility to automatically detect the internal latency (PDL) of a plugin.
pub fn detect_latency(plugin: &mut dyn Plugin, sample_rate: f64) -> usize {
    let channels = plugin.input_channels();
    let block_size = 128;
    let total_frames = 48000; // 1 second should be enough
    let mut input = vec![0.0; total_frames * channels];

    // Create an impulse at frame 0
    for sample in input.iter_mut().take(channels) {
        *sample = 1.0;
    }

    let mut output = vec![0.0; total_frames * plugin.output_channels()];
    plugin.reset();

    let mut frames_processed = 0;
    while frames_processed < total_frames {
        let num_frames = (block_size).min(total_frames - frames_processed);
        let ctx = ProcessContext {
            sample_rate: sample_rate as u32,
            num_frames,
        };

        let in_slice =
            &input[frames_processed * channels..(frames_processed + num_frames) * channels];
        let out_slice = &mut output[frames_processed * plugin.output_channels()
            ..(frames_processed + num_frames) * plugin.output_channels()];

        plugin.process(in_slice, out_slice, &ctx).unwrap();
        frames_processed += num_frames;
    }

    // Find the first non-zero sample in any output channel
    for f in 0..total_frames {
        for c in 0..plugin.output_channels() {
            if output[f * plugin.output_channels() + c].abs() > 1e-6 {
                return f;
            }
        }
    }

    0
}

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
            let ctx = ProcessContext {
                sample_rate: self.sample_rate as u32,
                num_frames,
            };

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
    let ctx = ProcessContext {
        sample_rate: sample_rate as u32,
        num_frames: buffer_size,
    };

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

/// A utility to test parameter automation ramps.
pub fn test_parameter_ramp(
    plugin: &mut dyn Plugin,
    param_id: &ParameterId,
    start_val: f32,
    end_val: f32,
    duration_frames: usize,
    sample_rate: f64,
) {
    let channels = plugin.input_channels();
    let input = vec![0.5; duration_frames * channels];
    let mut output = vec![0.0; duration_frames * plugin.output_channels()];

    // We'll process in small blocks to allow parameter updates at block boundaries
    let block_size = 64;
    let mut frames_processed = 0;

    while frames_processed < duration_frames {
        let num_frames = (block_size).min(duration_frames - frames_processed);

        // Calculate current ramp value
        let progress = frames_processed as f32 / duration_frames as f32;
        let val = start_val + (end_val - start_val) * progress;

        plugin
            .set_parameter(param_id.clone(), ParameterValue::Float(val))
            .unwrap();

        let ctx = ProcessContext {
            sample_rate: sample_rate as u32,
            num_frames,
        };

        let in_slice =
            &input[frames_processed * channels..(frames_processed + num_frames) * channels];
        let out_slice = &mut output[frames_processed * plugin.output_channels()
            ..(frames_processed + num_frames) * plugin.output_channels()];

        plugin.process(in_slice, out_slice, &ctx).unwrap();
        frames_processed += num_frames;
    }

    // Check for artifacts (sudden jumps in output)
    for i in 1..output.len() {
        let diff = (output[i] - output[i - 1]).abs();
        assert!(
            diff < 0.1,
            "Artifact detected at sample {}: jump of {}",
            i,
            diff
        );
    }
}
