//! Utility functions for audio feature extraction.
//!
//! Ported from bliss-audio utils.rs — pure Rust implementations of
//! mean, geometric mean, normalization, zero-crossing, STFT, etc.

use ndarray::{Array, Array1, Array2, arr1, s};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

use crate::analysis::plan_fft_forward;
use crate::stft::generate_hann_window;

/// Normalize a value from [min, max] to [-1, 1].
pub fn normalize(value: f32, min_value: f32, max_value: f32) -> f32 {
    2. * (value - min_value) / (max_value - min_value) - 1.
}

/// Arithmetic mean of a slice.
pub fn mean(input: &[f32]) -> f32 {
    if input.is_empty() {
        return 0.0;
    }
    input.iter().sum::<f32>() / input.len() as f32
}

/// Optimized geometric mean (from bliss, courtesy of Jacques-Henri Jourdan).
/// Only works for input of size a multiple of 8, with values in [0, 2^65].
pub fn geometric_mean(input: &[f32]) -> f32 {
    assert!(
        input.len().is_multiple_of(8),
        "geometric_mean input length must be a multiple of 8, got {}",
        input.len()
    );
    let mut exponents: i32 = 0;
    let mut mantissas: f64 = 1.;
    for ch in input.chunks_exact(8) {
        let mut m = (ch[0] as f64 * ch[1] as f64) * (ch[2] as f64 * ch[3] as f64);
        m *= 3.273390607896142e150; // 2^500 : avoid underflows and denormals
        m *= (ch[4] as f64 * ch[5] as f64) * (ch[6] as f64 * ch[7] as f64);
        if m == 0. {
            return 0.;
        }
        exponents += (m.to_bits() >> 52) as i32;
        mantissas *= f64::from_bits((m.to_bits() & 0xFFFFFFFFFFFFF) | 0x3FF0000000000000);
    }

    let n = input.len() as u32;
    (((mantissas as f32).log2() + exponents as f32) / n as f32 - (1023. + 500.) / 8.).exp2()
}

/// Count zero-crossings in a signal (Essentia algorithm).
pub fn number_crossings(input: &[f32]) -> u32 {
    if input.is_empty() {
        return 0;
    }
    let mut crossings = 0u32;
    let mut was_positive = input[0] > 0.;

    for &sample in input {
        let is_positive = sample > 0.;
        if was_positive != is_positive {
            crossings += 1;
            was_positive = is_positive;
        }
    }
    crossings
}

/// Reflect-pad an array (mirror boundary conditions).
pub fn reflect_pad(array: &[f32], pad: usize) -> Vec<f32> {
    let prefix = array[1..=pad].iter().rev().copied().collect::<Vec<f32>>();
    let suffix = array[(array.len() - 2) - pad + 1..array.len() - 1]
        .iter()
        .rev()
        .copied()
        .collect::<Vec<f32>>();
    let mut output = Vec::with_capacity(prefix.len() + array.len() + suffix.len());
    output.extend(prefix);
    output.extend(array);
    output.extend(suffix);
    output
}

/// Short-time Fourier transform with Hann window.
pub fn stft(signal: &[f32], window_length: usize, hop_length: usize) -> Array2<f64> {
    let mut stft = Array2::zeros((
        (signal.len() as f32 / hop_length as f32).ceil() as usize,
        window_length / 2 + 1,
    ));
    let signal = reflect_pad(signal, window_length / 2);

    // Periodic Hann window
    let hann_window = Array::from_vec(generate_hann_window(window_length));

    let fft = plan_fft_forward(window_length);

    for (window, mut stft_col) in signal
        .windows(window_length)
        .step_by(hop_length)
        .zip(stft.rows_mut())
    {
        let mut fft_input = (arr1(window) * &hann_window).mapv(|x| Complex::new(x, 0.));
        match fft_input.as_slice_mut() {
            Some(s) => fft.process(s),
            None => {
                fft.process(&mut fft_input.to_vec());
            }
        };
        stft_col.assign(
            &fft_input
                .slice(s![..window_length / 2 + 1])
                .mapv(|x| (x.re * x.re + x.im * x.im).sqrt() as f64),
        );
    }
    stft.permuted_axes((1, 0))
}

/// Convert Hz frequencies to fractional octaves (in-place).
pub fn hz_to_octs_inplace(
    frequencies: &mut Array1<f64>,
    tuning: f64,
    bins_per_octave: u32,
) -> &mut Array1<f64> {
    let a440 = 440.0 * 2_f64.powf(tuning / f64::from(bins_per_octave));
    *frequencies /= a440 / 16.;
    frequencies.mapv_inplace(f64::log2);
    frequencies
}

/// FFT-based convolution (same-size output).
pub fn convolve(input: &Array1<f64>, kernel: &Array1<f64>) -> Result<Array1<f64>, String> {
    let mut common_length = input.len() + kernel.len();
    if !common_length.is_multiple_of(2) {
        common_length -= 1;
    }
    let mut padded_input = Array::from_elem(
        common_length,
        Complex {
            re: 0.0_f64,
            im: 0.0,
        },
    );
    padded_input
        .slice_mut(s![..input.len()])
        .assign(&input.mapv(|x| Complex::new(x, 0.)));
    let mut padded_kernel = Array::from_elem(
        common_length,
        Complex {
            re: 0.0_f64,
            im: 0.0,
        },
    );
    padded_kernel
        .slice_mut(s![..kernel.len()])
        .assign(&kernel.mapv(|x| Complex::new(x, 0.)));

    let mut planner = FftPlanner::new();
    let forward = planner.plan_fft_forward(common_length);
    let padded_input_slice = padded_input
        .as_slice_mut()
        .ok_or("FFT buffer must be contiguous")?;
    forward.process(padded_input_slice);
    let padded_kernel_slice = padded_kernel
        .as_slice_mut()
        .ok_or("FFT buffer must be contiguous")?;
    forward.process(padded_kernel_slice);

    let mut multiplication = padded_input * padded_kernel;

    let back = planner.plan_fft_inverse(common_length);
    let multiplication_slice = multiplication
        .as_slice_mut()
        .ok_or("FFT buffer must be contiguous")?;
    back.process(multiplication_slice);

    let multiplication_length = multiplication.len() as f64;
    let multiplication = multiplication
        .slice_move(s![
            (kernel.len() - 1) / 2..(kernel.len() - 1) / 2 + input.len()
        ])
        .mapv(|x| x.re);
    Ok(multiplication / multiplication_length)
}

/// Standard deviation of a slice of f32 values.
pub fn std_deviation(values: &[f32]) -> f32 {
    if values.len() <= 1 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|&x| (x - m) * (x - m)).sum::<f32>() / values.len() as f32;
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mean() {
        let numbers = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        assert_eq!(2.0, mean(&numbers));
    }

    #[test]
    fn test_geometric_mean() {
        let numbers = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        assert_eq!(0.0, geometric_mean(&numbers));

        let numbers = vec![4.0, 2.0, 1.0, 4.0, 2.0, 1.0, 2.0, 2.0];
        assert!(0.0001 > (2.0 - geometric_mean(&numbers)).abs());
    }

    #[test]
    #[should_panic(expected = "geometric_mean input length must be a multiple of 8")]
    fn test_geometric_mean_panics_on_non_multiple_of_8() {
        // Issue #3: chunks_exact(8) silently drops the remainder, producing
        // a wrong result when the length is not a multiple of 8.
        let numbers = vec![1.0f32; 9];
        geometric_mean(&numbers);
    }

    #[test]
    fn test_number_crossings() {
        let input = vec![-1.0, 1.0, -1.0, 1.0];
        assert_eq!(3, number_crossings(&input));

        let input = vec![1.0, 1.0, 1.0];
        assert_eq!(0, number_crossings(&input));
    }

    #[test]
    fn test_normalize() {
        assert!((0.0 - normalize(0.5, 0.0, 1.0)).abs() < 1e-6);
        assert!((-1.0 - normalize(0.0, 0.0, 1.0)).abs() < 1e-6);
        assert!((1.0 - normalize(1.0, 0.0, 1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_std_deviation() {
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((2.0 - std_deviation(&values)).abs() < 0.01);
    }

    #[test]
    fn test_reflect_pad() {
        let array: Vec<f32> = (0..100).map(|x| x as f32).collect();
        let output = reflect_pad(&array, 3);
        assert_eq!(&output[..4], &[3.0, 2.0, 1.0, 0.0]);
        assert_eq!(&output[3..103], &array[..]);
        assert_eq!(&output[103..106], &[98.0, 97.0, 96.0]);
    }
}

#[test]
fn test_mean_empty() {
    assert_eq!(mean(&[]), 0.0);
}

#[test]
fn test_std_deviation_empty_and_single() {
    assert_eq!(std_deviation(&[]), 0.0);
    assert_eq!(std_deviation(&[5.0]), 0.0);
}

#[test]
fn test_number_crossings_empty_and_no_cross() {
    assert_eq!(number_crossings(&[]), 0);
    assert_eq!(number_crossings(&[0.0, 0.0, 0.0]), 0);
    assert_eq!(number_crossings(&[-1.0, -0.5, -0.1]), 0);
}

#[test]
fn test_reflect_pad_small() {
    let array = vec![0.0f32, 1.0, 2.0];
    let out = reflect_pad(&array, 1);
    assert_eq!(out, vec![1.0, 0.0, 1.0, 2.0, 1.0]);
}

#[test]
#[should_panic]
fn test_reflect_pad_too_large_panics() {
    reflect_pad(&[0.0, 1.0], 2);
}

#[test]
fn test_stft_basic_shape() {
    let signal: Vec<f32> = (0..1024)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 22050.0).sin())
        .collect();
    let spec = stft(&signal, 512, 128);
    assert_eq!(spec.dim().0, 512 / 2 + 1);
    assert_eq!(spec.dim().1, (1024_f32 / 128.0).ceil() as usize);
}

#[test]
fn test_hz_to_octs_inplace() {
    let mut freqs = ndarray::arr1(&[220.0, 440.0, 880.0]);
    hz_to_octs_inplace(&mut freqs, 0.0, 12);
    // Reference is A440/16 = A27.5, so A220=8=2^3, A440=16=2^4, A880=32=2^5.
    assert!((freqs[0] - 3.0).abs() < 1e-6, "got {}", freqs[0]);
    assert!((freqs[1] - 4.0).abs() < 1e-6, "got {}", freqs[1]);
    assert!((freqs[2] - 5.0).abs() < 1e-6, "got {}", freqs[2]);
}

#[test]
fn test_convolve_delta() {
    let input = ndarray::arr1(&[0.0, 0.0, 1.0, 0.0, 0.0]);
    let kernel = ndarray::arr1(&[1.0, 2.0, 3.0]);
    let out = convolve(&input, &kernel).unwrap();
    assert_eq!(out.len(), input.len());
    assert!((out[2] - 2.0).abs() < 1e-6, "got {}", out[2]);
}
