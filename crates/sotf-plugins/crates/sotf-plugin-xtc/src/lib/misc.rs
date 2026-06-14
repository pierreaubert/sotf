use rustfft::{FftPlanner, num_complex::Complex};

pub(super) const MAX_PROCESS_FRAMES: usize = 16_384;

pub(super) fn fir_taps_to_half_spectrum(
    taps: &[f64],
    num_bins: usize,
) -> Result<Vec<Complex<f32>>, String> {
    if num_bins < 2 {
        return Err("num_bins must contain at least DC and Nyquist".to_string());
    }
    let fft_size = (num_bins - 1) * 2;
    let mut buffer = vec![Complex::new(0.0_f32, 0.0_f32); fft_size];
    let copy_len = taps.len().min(fft_size);
    for idx in 0..copy_len {
        buffer[idx] = Complex::new(taps[idx] as f32, 0.0);
    }
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut buffer);
    buffer.truncate(num_bins);
    Ok(buffer)
}
