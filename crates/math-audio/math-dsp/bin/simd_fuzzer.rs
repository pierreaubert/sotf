// ============================================================================
// SIMD Fuzzer - Compare SIMD implementations against scalar reference
// ============================================================================
//
// Generates random inputs of various sizes and value ranges, then compares
// each SIMD-accelerated function against a pure scalar implementation.
// Reports any mismatches exceeding tolerance thresholds.
//
// Usage:
//   cargo run --bin simd-fuzzer --no-default-features -- --iterations 10000
//   cargo run --bin simd-fuzzer --no-default-features -- --iterations 5000 --seed 42
//   cargo run --bin simd-fuzzer --no-default-features -- --iterations 1000 --function blend

use clap::Parser;
use math_audio_dsp::simd;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rustfft::num_complex::Complex;
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// CLI
// ============================================================================

#[derive(Parser, Debug)]
#[command(name = "simd_fuzzer")]
#[command(about = "Fuzz test SIMD functions against scalar reference implementations")]
struct Args {
    /// Number of random test iterations per function
    #[arg(short, long, default_value = "10000")]
    iterations: usize,

    /// Random seed (omit for random)
    #[arg(short, long)]
    seed: Option<u64>,

    /// Only test a specific function (omit to test all)
    #[arg(short, long)]
    function: Option<String>,
}

// ============================================================================
// Scalar reference implementations
// ============================================================================

fn scalar_complex_mul_add(dst: &mut [Complex<f32>], src: &[Complex<f32>], hrtf: &[Complex<f32>]) {
    for i in 0..dst.len() {
        dst[i] += src[i] * hrtf[i];
    }
}

fn scalar_complex_mul(dst: &mut [Complex<f32>], src: &[Complex<f32>], hrtf: &[Complex<f32>]) {
    for i in 0..dst.len() {
        dst[i] = src[i] * hrtf[i];
    }
}

fn scalar_complex_mul_inplace(dst: &mut [Complex<f32>], hrtf: &[Complex<f32>]) {
    for i in 0..dst.len() {
        dst[i] *= hrtf[i];
    }
}

fn scalar_scale_add(dst: &mut [f32], src: &[f32], scale: f32) {
    for i in 0..dst.len() {
        dst[i] += src[i] * scale;
    }
}

fn scalar_blend(dst: &mut [f32], prev: &[f32], alpha: f32) {
    for i in 0..dst.len() {
        dst[i] = prev[i] + alpha * (dst[i] - prev[i]);
    }
}

fn scalar_window_mul(dst: &mut [f32], src: &[f32], window: &[f32]) {
    for i in 0..dst.len() {
        dst[i] = src[i] * window[i];
    }
}

fn scalar_window_mul_inplace(data: &mut [f32], window: &[f32]) {
    let len = data.len().min(window.len());
    for i in 0..len {
        data[i] *= window[i];
    }
}

fn scalar_deinterleave_stereo(input: &[f32], left: &mut [f32], right: &mut [f32]) {
    for (i, chunk) in input.chunks_exact(2).enumerate() {
        left[i] = chunk[0];
        right[i] = chunk[1];
    }
}

fn scalar_apply_gain(buffer: &mut [f32], gain: f32) {
    for val in buffer.iter_mut() {
        *val *= gain;
    }
}

fn scalar_apply_per_channel_gain(buffer: &mut [f32], channels: usize, gains: &[f32]) {
    let num_frames = buffer.len() / channels;
    for frame in 0..num_frames {
        for ch in 0..channels {
            buffer[frame * channels + ch] *= gains[ch];
        }
    }
}

fn scalar_compute_covariance(
    left: &[Complex<f32>],
    right: &[Complex<f32>],
    start: usize,
    end: usize,
) -> (f32, f32, Complex<f32>) {
    let mut cov_xx = 0.0_f32;
    let mut cov_yy = 0.0_f32;
    let mut cov_xy = Complex::new(0.0, 0.0);
    for i in start..end {
        cov_xx += left[i].norm_sqr();
        cov_yy += right[i].norm_sqr();
        cov_xy += left[i] * right[i].conj();
    }
    (cov_xx, cov_yy, cov_xy)
}

fn scalar_flush_denormals(samples: &mut [f32]) {
    const DENORM_THRESHOLD: f32 = 1e-30;
    for sample in samples.iter_mut() {
        if sample.abs() < DENORM_THRESHOLD {
            *sample = 0.0;
        }
    }
}

// ============================================================================
// Random data generators
// ============================================================================

fn rand_complex_vec(rng: &mut StdRng, len: usize, range: f32) -> Vec<Complex<f32>> {
    (0..len)
        .map(|_| {
            Complex::new(
                rng.random_range(-range..range),
                rng.random_range(-range..range),
            )
        })
        .collect()
}

fn rand_f32_vec(rng: &mut StdRng, len: usize, range: f32) -> Vec<f32> {
    (0..len).map(|_| rng.random_range(-range..range)).collect()
}

/// Pick a random buffer size that exercises SIMD boundaries
fn rand_size(rng: &mut StdRng) -> usize {
    // Mix of aligned and unaligned sizes, small and large
    let choices = [
        1, 2, 3, 4, 5, 7, 8, 9, 13, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256,
        257, 511, 512, 513, 1023, 1024, 1025, 2048, 4096,
    ];
    choices[rng.random_range(0..choices.len())]
}

/// Pick a random value range (normal, small, large)
fn rand_value_range(rng: &mut StdRng) -> f32 {
    let ranges = [0.001, 0.1, 1.0, 10.0, 100.0, 1e5, 1e10, 1e-10, 1e-20];
    ranges[rng.random_range(0..ranges.len())]
}

// ============================================================================
// Comparison helpers
// ============================================================================

fn max_abs_diff_complex(a: &[Complex<f32>], b: &[Complex<f32>]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x.re - y.re).abs().max((x.im - y.im).abs()))
        .fold(0.0_f32, f32::max)
}

fn max_rel_diff_complex(a: &[Complex<f32>], b: &[Complex<f32>]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let denom_re = x.re.abs().max(y.re.abs()).max(1e-30);
            let denom_im = x.im.abs().max(y.im.abs()).max(1e-30);
            ((x.re - y.re).abs() / denom_re).max((x.im - y.im).abs() / denom_im)
        })
        .fold(0.0_f32, f32::max)
}

fn max_abs_diff_f32(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0_f32, f32::max)
}

fn max_rel_diff_f32(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let denom = x.abs().max(y.abs()).max(1e-30);
            (x - y).abs() / denom
        })
        .fold(0.0_f32, f32::max)
}

fn check_complex(
    name: &str,
    iter: usize,
    size: usize,
    range: f32,
    simd_result: &[Complex<f32>],
    scalar_result: &[Complex<f32>],
    failures: &AtomicUsize,
) {
    let abs_diff = max_abs_diff_complex(simd_result, scalar_result);
    let rel_diff = max_rel_diff_complex(simd_result, scalar_result);

    // Allow relative error up to 1e-5 or absolute error up to 1e-6 * range
    let abs_threshold = (range * 1e-5).max(1e-10);
    let rel_threshold = 1e-4;

    if abs_diff > abs_threshold && rel_diff > rel_threshold {
        failures.fetch_add(1, Ordering::Relaxed);
        eprintln!(
            "FAIL {name} iter={iter} size={size} range={range:.0e}: abs_diff={abs_diff:.2e} rel_diff={rel_diff:.2e}"
        );
    }
}

fn check_f32(
    name: &str,
    iter: usize,
    size: usize,
    range: f32,
    simd_result: &[f32],
    scalar_result: &[f32],
    failures: &AtomicUsize,
) {
    let abs_diff = max_abs_diff_f32(simd_result, scalar_result);
    let rel_diff = max_rel_diff_f32(simd_result, scalar_result);

    let abs_threshold = (range * 1e-5).max(1e-10);
    let rel_threshold = 1e-4;

    if abs_diff > abs_threshold && rel_diff > rel_threshold {
        failures.fetch_add(1, Ordering::Relaxed);
        eprintln!(
            "FAIL {name} iter={iter} size={size} range={range:.0e}: abs_diff={abs_diff:.2e} rel_diff={rel_diff:.2e}"
        );
    }
}

// ============================================================================
// Fuzz test functions
// ============================================================================

fn fuzz_complex_mul_add(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let src = rand_complex_vec(rng, size, range);
        let hrtf = rand_complex_vec(rng, size, range);
        let initial = rand_complex_vec(rng, size, range);

        let mut simd_dst = initial.clone();
        let mut scalar_dst = initial;

        simd::complex_mul_add_simd(&mut simd_dst, &src, &hrtf);
        scalar_complex_mul_add(&mut scalar_dst, &src, &hrtf);

        check_complex(
            "complex_mul_add",
            iter,
            size,
            range * range, // product range
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_complex_mul(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let src = rand_complex_vec(rng, size, range);
        let hrtf = rand_complex_vec(rng, size, range);

        let mut simd_dst = vec![Complex::new(0.0, 0.0); size];
        let mut scalar_dst = vec![Complex::new(0.0, 0.0); size];

        simd::complex_mul_simd(&mut simd_dst, &src, &hrtf);
        scalar_complex_mul(&mut scalar_dst, &src, &hrtf);

        check_complex(
            "complex_mul",
            iter,
            size,
            range * range,
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_complex_mul_inplace(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let data = rand_complex_vec(rng, size, range);
        let hrtf = rand_complex_vec(rng, size, range);

        let mut simd_dst = data.clone();
        let mut scalar_dst = data;

        simd::complex_mul_inplace_simd(&mut simd_dst, &hrtf);
        scalar_complex_mul_inplace(&mut scalar_dst, &hrtf);

        check_complex(
            "complex_mul_inplace",
            iter,
            size,
            range * range,
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_scale_add(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let src = rand_f32_vec(rng, size, range);
        let initial = rand_f32_vec(rng, size, range);
        let scale: f32 = rng.random_range(-range..range);

        let mut simd_dst = initial.clone();
        let mut scalar_dst = initial;

        simd::scale_add_simd(&mut simd_dst, &src, scale);
        scalar_scale_add(&mut scalar_dst, &src, scale);

        check_f32(
            "scale_add",
            iter,
            size,
            range * range,
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_blend(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let prev = rand_f32_vec(rng, size, range);
        let current = rand_f32_vec(rng, size, range);
        let alpha: f32 = rng.random_range(0.0..1.0);

        let mut simd_dst = current.clone();
        let mut scalar_dst = current;

        simd::blend_simd(&mut simd_dst, &prev, alpha);
        scalar_blend(&mut scalar_dst, &prev, alpha);

        check_f32(
            "blend",
            iter,
            size,
            range,
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_window_mul(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let src = rand_f32_vec(rng, size, range);
        // Window values are typically 0..1 but we fuzz with wider range
        let window = rand_f32_vec(rng, size, 1.0);

        let mut simd_dst = vec![0.0_f32; size];
        let mut scalar_dst = vec![0.0_f32; size];

        simd::window_mul_simd(&mut simd_dst, &src, &window);
        scalar_window_mul(&mut scalar_dst, &src, &window);

        check_f32(
            "window_mul",
            iter,
            size,
            range,
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_window_mul_inplace(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let data = rand_f32_vec(rng, size, range);
        let window = rand_f32_vec(rng, size, 1.0);

        let mut simd_dst = data.clone();
        let mut scalar_dst = data;

        simd::window_mul_simd_inplace(&mut simd_dst, &window);
        scalar_window_mul_inplace(&mut scalar_dst, &window);

        check_f32(
            "window_mul_inplace",
            iter,
            size,
            range,
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_deinterleave_stereo(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let num_frames = rand_size(rng);
        let range = rand_value_range(rng);

        let interleaved = rand_f32_vec(rng, num_frames * 2, range);

        let mut simd_left = vec![0.0_f32; num_frames];
        let mut simd_right = vec![0.0_f32; num_frames];
        let mut scalar_left = vec![0.0_f32; num_frames];
        let mut scalar_right = vec![0.0_f32; num_frames];

        simd::deinterleave_stereo(&interleaved, &mut simd_left, &mut simd_right);
        scalar_deinterleave_stereo(&interleaved, &mut scalar_left, &mut scalar_right);

        // Deinterleave should be bit-exact (no arithmetic, just copying)
        let left_diff = max_abs_diff_f32(&simd_left, &scalar_left);
        let right_diff = max_abs_diff_f32(&simd_right, &scalar_right);

        if left_diff > 0.0 || right_diff > 0.0 {
            failures.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "FAIL deinterleave_stereo iter={iter} frames={num_frames}: left_diff={left_diff:.2e} right_diff={right_diff:.2e}"
            );
        }
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_apply_gain(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);
        let range = rand_value_range(rng);

        let data = rand_f32_vec(rng, size, range);
        let gain: f32 = rng.random_range(-10.0..10.0);

        let mut simd_dst = data.clone();
        let mut scalar_dst = data;

        simd::apply_gain_simd(&mut simd_dst, gain);
        scalar_apply_gain(&mut scalar_dst, gain);

        check_f32(
            "apply_gain",
            iter,
            size,
            range * gain.abs(),
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_apply_per_channel_gain(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let channels = [1, 2, 6, 8][rng.random_range(0..4)];
        let num_frames = rand_size(rng);
        let size = num_frames * channels;
        let range = rand_value_range(rng);

        let data = rand_f32_vec(rng, size, range);
        let gains: Vec<f32> = (0..channels).map(|_| rng.random_range(-5.0..5.0)).collect();
        let max_gain = gains.iter().map(|g| g.abs()).fold(0.0_f32, f32::max);

        let mut simd_dst = data.clone();
        let mut scalar_dst = data;

        simd::apply_per_channel_gain_simd(&mut simd_dst, channels, &gains);
        scalar_apply_per_channel_gain(&mut scalar_dst, channels, &gains);

        check_f32(
            "apply_per_channel_gain",
            iter,
            size,
            range * max_gain,
            &simd_dst,
            &scalar_dst,
            &failures,
        );
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_compute_covariance(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng).max(2); // need at least 2 for start < end
        let range = rand_value_range(rng);

        let left = rand_complex_vec(rng, size, range);
        let right = rand_complex_vec(rng, size, range);

        let start = rng.random_range(0..size - 1);
        let end = rng.random_range(start + 1..=size);

        let (simd_xx, simd_yy, simd_xy) = simd::compute_covariance_simd(&left, &right, start, end);
        let (scalar_xx, scalar_yy, scalar_xy) =
            scalar_compute_covariance(&left, &right, start, end);

        let result_range = range * range * (end - start) as f32;
        let abs_threshold = (result_range * 1e-5).max(1e-10);
        let rel_threshold = 1e-3; // covariance accumulates more error

        let xx_diff = (simd_xx - scalar_xx).abs();
        let yy_diff = (simd_yy - scalar_yy).abs();
        let xy_re_diff = (simd_xy.re - scalar_xy.re).abs();
        let xy_im_diff = (simd_xy.im - scalar_xy.im).abs();

        let xx_rel = xx_diff / scalar_xx.abs().max(1e-30);
        let yy_rel = yy_diff / scalar_yy.abs().max(1e-30);
        let xy_re_rel = xy_re_diff / scalar_xy.re.abs().max(1e-30);
        let xy_im_rel = xy_im_diff / scalar_xy.im.abs().max(1e-30);

        let max_abs = xx_diff.max(yy_diff).max(xy_re_diff).max(xy_im_diff);
        let max_rel = xx_rel.max(yy_rel).max(xy_re_rel).max(xy_im_rel);

        if max_abs > abs_threshold && max_rel > rel_threshold {
            failures.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "FAIL compute_covariance iter={iter} size={size} range={range:.0e} [{start}..{end}): abs={max_abs:.2e} rel={max_rel:.2e}"
            );
        }
    }
    failures.load(Ordering::Relaxed)
}

fn fuzz_flush_denormals(rng: &mut StdRng, iterations: usize) -> usize {
    let failures = AtomicUsize::new(0);
    for iter in 0..iterations {
        let size = rand_size(rng);

        // Generate mix of normal, denormal, and zero values
        let data: Vec<f32> = (0..size)
            .map(|_| {
                let kind = rng.random_range(0..5);
                match kind {
                    0 => 0.0,
                    1 => rng.random_range(-1.0..1.0),      // normal
                    2 => rng.random_range(1e-35..1e-31),   // denormal-ish
                    3 => rng.random_range(-1e-31..-1e-35), // negative denormal
                    _ => rng.random_range(-100.0..100.0),  // large normal
                }
            })
            .collect();

        let mut simd_dst = data.clone();
        let mut scalar_dst = data;

        simd::flush_denormals_inplace(&mut simd_dst);
        scalar_flush_denormals(&mut scalar_dst);

        // Should be bit-exact
        let diff = max_abs_diff_f32(&simd_dst, &scalar_dst);
        if diff > 0.0 {
            failures.fetch_add(1, Ordering::Relaxed);
            eprintln!("FAIL flush_denormals iter={iter} size={size}: diff={diff:.2e}");
        }
    }
    failures.load(Ordering::Relaxed)
}

// ============================================================================
// Main
// ============================================================================

type FuzzFn = fn(&mut StdRng, usize) -> usize;

struct FuzzResult {
    name: &'static str,
    iterations: usize,
    failures: usize,
}

fn main() {
    let args = Args::parse();

    let seed = args.seed.unwrap_or_else(|| {
        let s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        println!("Using random seed: {s} (reproduce with --seed {s})");
        s
    });

    let iterations = args.iterations;

    // Platform info
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    println!("Platform: x86_64 with AVX2");
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    println!("Platform: aarch64 with NEON");
    #[cfg(not(any(
        all(target_arch = "x86_64", target_feature = "avx2"),
        all(target_arch = "aarch64", target_feature = "neon")
    )))]
    println!("Platform: scalar fallback (no SIMD)");

    println!("Running {iterations} iterations per function...\n");

    let all_functions: Vec<(&str, FuzzFn)> = vec![
        ("complex_mul_add", fuzz_complex_mul_add),
        ("complex_mul", fuzz_complex_mul),
        ("complex_mul_inplace", fuzz_complex_mul_inplace),
        ("scale_add", fuzz_scale_add),
        ("blend", fuzz_blend),
        ("window_mul", fuzz_window_mul),
        ("window_mul_inplace", fuzz_window_mul_inplace),
        ("deinterleave_stereo", fuzz_deinterleave_stereo),
        ("apply_gain", fuzz_apply_gain),
        ("apply_per_channel_gain", fuzz_apply_per_channel_gain),
        ("compute_covariance", fuzz_compute_covariance),
        ("flush_denormals", fuzz_flush_denormals),
    ];

    let functions: Vec<_> = if let Some(ref filter) = args.function {
        all_functions
            .into_iter()
            .filter(|(name, _)| name.contains(filter.as_str()))
            .collect()
    } else {
        all_functions
    };

    if functions.is_empty() {
        eprintln!("No functions matching filter '{}'", args.function.unwrap());
        eprintln!(
            "Available: complex_mul_add, complex_mul, complex_mul_inplace, scale_add, blend, window_mul, window_mul_inplace, deinterleave_stereo, apply_gain, apply_per_channel_gain, compute_covariance, flush_denormals"
        );
        std::process::exit(1);
    }

    let mut results = Vec::new();

    for (name, fuzz_fn) in &functions {
        let mut rng = StdRng::seed_from_u64(seed);
        print!("  {name:<30}");
        let failures = fuzz_fn(&mut rng, iterations);
        if failures == 0 {
            println!("PASS ({iterations} iterations)");
        } else {
            println!("FAIL ({failures}/{iterations} failures)");
        }
        results.push(FuzzResult {
            name,
            iterations,
            failures,
        });
    }

    // Summary
    println!();
    let total_failures: usize = results.iter().map(|r| r.failures).sum();
    let total_tests: usize = results.iter().map(|r| r.iterations).sum();

    if total_failures == 0 {
        println!(
            "ALL PASSED: {total_tests} total tests across {} functions",
            results.len()
        );
    } else {
        println!("FAILURES: {total_failures}/{total_tests} tests failed");
        for r in &results {
            if r.failures > 0 {
                println!("  {} : {}/{} failed", r.name, r.failures, r.iterations);
            }
        }
        std::process::exit(1);
    }
}
