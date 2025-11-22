use clap::Parser;
use hound::WavReader;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

/// Type alias for spectrum result (frequencies, magnitudes, phases)
type SpectrumResult = Result<(Vec<f32>, Vec<f32>, Vec<f32>), String>;

/// Convert WAV file to frequency/SPL/phase CSV
#[derive(Parser)]
#[command(name = "wav2csv")]
#[command(about = "Analyze WAV file and output frequency/SPL/phase CSV")]
#[command(
    long_about = "Analyze WAV files and output frequency response as CSV.\n\n\
For stationary signals (music, noise): Use default Welch's method\n\
For log sweeps: Use --single-fft --pink-compensation --no-window\n\
For impulse responses: Use --single-fft"
)]
struct Cli {
    /// Input WAV file
    input: PathBuf,

    /// Output CSV file (defaults to input filename with .csv extension)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Number of frequency points (default: 200)
    #[arg(short, long, default_value = "200")]
    num_points: usize,

    /// Minimum frequency in Hz (default: 20)
    #[arg(long, default_value = "20.0")]
    min_freq: f32,

    /// Maximum frequency in Hz (default: 20000)
    #[arg(long, default_value = "20000.0")]
    max_freq: f32,

    /// FFT size (default: 16384)
    #[arg(long)]
    fft_size: Option<usize>,

    /// Window overlap ratio (0.0-1.0, default: 0.5)
    #[arg(long, default_value = "0.5")]
    overlap: f32,

    /// Use single FFT instead of Welch's method (better for sweeps and impulse responses)
    #[arg(long)]
    single_fft: bool,

    /// Apply pink compensation (-3dB/octave) for log sweeps
    #[arg(long)]
    pink_compensation: bool,

    /// Use rectangular window (no windowing) instead of Hann
    #[arg(long)]
    no_window: bool,
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    // Load WAV file
    println!("Loading WAV file: {:?}", cli.input);
    let (signal, sample_rate) = load_wav_mono(&cli.input)?;
    println!(
        "Loaded {} samples at {} Hz ({:.2} seconds)",
        signal.len(),
        sample_rate,
        signal.len() as f32 / sample_rate as f32
    );

    // Determine FFT size
    let fft_size = if cli.single_fft {
        // For single FFT, use entire signal (rounded up to power of 2)
        cli.fft_size
            .unwrap_or_else(|| next_power_of_two(signal.len()))
    } else {
        // For Welch's method, use smaller windows
        cli.fft_size.unwrap_or(16384)
    };
    println!("Using FFT size: {}", fft_size);

    // Compute spectrum
    let (freqs, magnitudes_db, phases_deg) = if cli.single_fft {
        println!("Computing spectrum using single FFT (entire signal)...");
        compute_single_fft_spectrum(&signal, sample_rate, fft_size, cli.no_window)?
    } else {
        println!("Computing spectrum using Welch's method (averaged windows)...");
        compute_welch_spectrum(&signal, sample_rate, fft_size, cli.overlap)?
    };

    // Generate logarithmically spaced frequency points
    println!(
        "Interpolating to {} points between {:.1} Hz and {:.1} Hz...",
        cli.num_points, cli.min_freq, cli.max_freq
    );
    let log_freqs = generate_log_frequencies(cli.num_points, cli.min_freq, cli.max_freq);

    // Interpolate magnitude and phase at log frequencies
    let mut interp_mag = interpolate_log(&freqs, &magnitudes_db, &log_freqs);
    let interp_phase = interpolate_log(&freqs, &phases_deg, &log_freqs);

    // Apply pink compensation if requested (for log sweeps)
    if cli.pink_compensation {
        println!("Applying log-sweep compensation (+10*log10(f))...");
        let ref_freq = 1000.0; // Reference frequency
        for (i, freq) in log_freqs.iter().enumerate() {
            if *freq > 0.0 {
                // Log sweep compensation: +10*log10(f/f0)
                // This compensates for the 1/f energy distribution in log sweeps
                let correction = 10.0 * (freq / ref_freq).log10();
                interp_mag[i] += correction;
            }
        }
    }

    // Determine output path
    let output_path = cli.output.unwrap_or_else(|| {
        let mut path = cli.input.clone();
        path.set_extension("csv");
        path
    });

    // Write CSV
    println!("Writing CSV to: {:?}", output_path);
    write_csv(&output_path, &log_freqs, &interp_mag, &interp_phase)?;

    println!("Done!");
    Ok(())
}

/// Load WAV file as mono signal
fn load_wav_mono(path: &PathBuf) -> Result<(Vec<f32>, u32), String> {
    let mut reader = WavReader::open(path).map_err(|e| format!("Failed to open WAV: {}", e))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / max_val)
                .collect()
        }
    };

    // Convert to mono by averaging channels
    let mono = if channels == 1 {
        samples
    } else {
        samples
            .chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    };

    Ok((mono, sample_rate))
}

/// Compute spectrum using Welch's method (averaged periodograms)
fn compute_welch_spectrum(
    signal: &[f32],
    sample_rate: u32,
    fft_size: usize,
    overlap: f32,
) -> SpectrumResult {
    if signal.is_empty() {
        return Err("Signal is empty".to_string());
    }

    let overlap_samples = (fft_size as f32 * overlap.clamp(0.0, 0.95)) as usize;
    let hop_size = fft_size - overlap_samples;

    // Calculate number of windows
    let num_windows = if signal.len() >= fft_size {
        1 + (signal.len() - fft_size) / hop_size
    } else {
        1
    };

    println!(
        "  Processing {} windows (hop size: {}, overlap: {:.1}%)",
        num_windows,
        hop_size,
        overlap * 100.0
    );

    // Initialize accumulators for magnitude and phase
    let num_bins = fft_size / 2;
    let mut magnitude_sum = vec![0.0_f32; num_bins];
    let mut phase_real_sum = vec![0.0_f32; num_bins];
    let mut phase_imag_sum = vec![0.0_f32; num_bins];

    // Precompute Hann window
    let hann_window: Vec<f32> = (0..fft_size)
        .map(|i| 0.5 * (1.0 - ((2.0 * PI * i as f32) / (fft_size as f32 - 1.0)).cos()))
        .collect();

    // Window normalization factor (for power)
    let window_power: f32 = hann_window.iter().map(|&w| w * w).sum();
    let scale_factor = 2.0 / window_power; // Factor of 2 for single-sided spectrum

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);

    // Process each window
    for window_idx in 0..num_windows {
        let start = window_idx * hop_size;
        let end = (start + fft_size).min(signal.len());
        let window_len = end - start;

        // Create windowed segment
        let mut windowed = vec![0.0_f32; fft_size];
        for i in 0..window_len {
            windowed[i] = signal[start + i] * hann_window[i];
        }

        // Convert to complex
        let mut buffer: Vec<Complex<f32>> =
            windowed.iter().map(|&x| Complex::new(x, 0.0)).collect();

        // Perform FFT
        fft.process(&mut buffer);

        // Accumulate magnitude and phase components
        for i in 0..num_bins {
            let mag = buffer[i].norm() * scale_factor.sqrt();
            magnitude_sum[i] += mag * mag; // Accumulate power
            phase_real_sum[i] += buffer[i].re;
            phase_imag_sum[i] += buffer[i].im;
        }
    }

    // Average and convert to dBFS and degrees
    let magnitudes_db: Vec<f32> = magnitude_sum
        .iter()
        .map(|&mag_sq| {
            let mag = (mag_sq / num_windows as f32).sqrt();
            if mag > 1e-10 {
                20.0 * mag.log10()
            } else {
                -200.0
            }
        })
        .collect();

    let phases_deg: Vec<f32> = phase_real_sum
        .iter()
        .zip(phase_imag_sum.iter())
        .map(|(&re, &im)| (im / num_windows as f32).atan2(re / num_windows as f32) * 180.0 / PI)
        .collect();

    let freqs: Vec<f32> = (0..num_bins)
        .map(|i| i as f32 * sample_rate as f32 / fft_size as f32)
        .collect();

    Ok((freqs, magnitudes_db, phases_deg))
}

/// Compute spectrum using a single FFT (good for sweeps, impulse responses)
fn compute_single_fft_spectrum(
    signal: &[f32],
    sample_rate: u32,
    fft_size: usize,
    no_window: bool,
) -> SpectrumResult {
    if signal.is_empty() {
        return Err("Signal is empty".to_string());
    }

    // Prepare signal with zero-padding if needed
    let mut windowed = vec![0.0_f32; fft_size];
    let copy_len = signal.len().min(fft_size);
    windowed[..copy_len].copy_from_slice(&signal[..copy_len]);

    // Apply Hann window to reduce spectral leakage (unless disabled)
    let window_scale_factor = if no_window {
        println!("  Using rectangular window (no windowing)");
        1.0
    } else {
        println!("  Using Hann window");
        let hann_window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - ((2.0 * PI * i as f32) / (fft_size as f32 - 1.0)).cos()))
            .collect();

        for (i, sample) in windowed.iter_mut().enumerate() {
            *sample *= hann_window[i];
        }

        // Window power for Hann window
        hann_window.iter().map(|&w| w * w).sum::<f32>()
    };

    // Convert to complex
    let mut buffer: Vec<Complex<f32>> = windowed.iter().map(|&x| Complex::new(x, 0.0)).collect();

    // Perform FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    fft.process(&mut buffer);

    // Window normalization for power
    let scale_factor = if no_window {
        // For rectangular window, window_power = fft_size
        (2.0 / fft_size as f32).sqrt()
    } else {
        // For Hann window, compensate for energy loss
        (2.0 / window_scale_factor).sqrt()
    };

    // Convert to magnitude (dBFS) and phase (degrees)
    let num_bins = fft_size / 2;
    let magnitudes_db: Vec<f32> = buffer[..num_bins]
        .iter()
        .map(|c| {
            let mag = c.norm() * scale_factor;
            if mag > 1e-10 {
                20.0 * mag.log10()
            } else {
                -200.0
            }
        })
        .collect();

    let phases_deg: Vec<f32> = buffer[..num_bins]
        .iter()
        .map(|c| c.arg() * 180.0 / PI)
        .collect();

    let freqs: Vec<f32> = (0..num_bins)
        .map(|i| i as f32 * sample_rate as f32 / fft_size as f32)
        .collect();

    Ok((freqs, magnitudes_db, phases_deg))
}

/// Next power of two
fn next_power_of_two(n: usize) -> usize {
    let mut p = 1;
    while p < n {
        p *= 2;
    }
    p.min(1048576) // Cap at 1M samples (2^20)
}

/// Generate logarithmically spaced frequencies
fn generate_log_frequencies(num_points: usize, min_freq: f32, max_freq: f32) -> Vec<f32> {
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();
    let step = (log_max - log_min) / (num_points - 1) as f32;

    (0..num_points)
        .map(|i| (log_min + i as f32 * step).exp())
        .collect()
}

/// Logarithmic interpolation
fn interpolate_log(x: &[f32], y: &[f32], x_new: &[f32]) -> Vec<f32> {
    x_new
        .iter()
        .map(|&freq| {
            // Find the two nearest points in x
            let idx = x.iter().position(|&f| f >= freq).unwrap_or(x.len() - 1);

            if idx == 0 {
                return y[0];
            }

            let x0 = x[idx - 1];
            let x1 = x[idx];
            let y0 = y[idx - 1];
            let y1 = y[idx];

            // Linear interpolation in log space
            if x1 <= x0 {
                return y0;
            }

            let t = (freq - x0) / (x1 - x0);
            y0 + t * (y1 - y0)
        })
        .collect()
}

/// Write CSV file
fn write_csv(
    path: &PathBuf,
    frequencies: &[f32],
    spl_db: &[f32],
    phase_deg: &[f32],
) -> Result<(), String> {
    let mut file = File::create(path).map_err(|e| format!("Failed to create CSV: {}", e))?;

    writeln!(file, "frequency_hz,spl_db,phase_deg")
        .map_err(|e| format!("Failed to write CSV header: {}", e))?;

    for i in 0..frequencies.len() {
        writeln!(
            file,
            "{:.2},{:.2},{:.2}",
            frequencies[i], spl_db[i], phase_deg[i]
        )
        .map_err(|e| format!("Failed to write CSV row: {}", e))?;
    }

    Ok(())
}
