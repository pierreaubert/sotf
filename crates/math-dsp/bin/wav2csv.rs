use clap::Parser;
use math_audio_dsp::analysis::{WavAnalysisConfig, analyze_wav_file, write_wav_analysis_csv};
use std::path::PathBuf;

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

    /// Number of frequency points (default: 2000)
    #[arg(short, long, default_value = "2000")]
    pub num_points: usize,

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
    println!("Loading WAV file: {:?}", cli.input);

    // Build configuration from CLI arguments
    let config = WavAnalysisConfig {
        num_points: cli.num_points,
        min_freq: cli.min_freq,
        max_freq: cli.max_freq,
        fft_size: cli.fft_size,
        overlap: cli.overlap,
        single_fft: cli.single_fft,
        pink_compensation: cli.pink_compensation,
        no_window: cli.no_window,
    };

    // Analyze WAV file
    let result = analyze_wav_file(&cli.input, &config)?;

    println!(
        "Analyzed {} frequency points from {:.1} Hz to {:.1} Hz",
        result.frequencies.len(),
        result.frequencies.first().unwrap_or(&0.0),
        result.frequencies.last().unwrap_or(&0.0)
    );

    // Determine output path
    let output_path = cli.output.unwrap_or_else(|| {
        let mut path = cli.input.clone();
        path.set_extension("csv");
        path
    });

    // Write CSV
    println!("Writing CSV to: {:?}", output_path);
    write_wav_analysis_csv(&result, &output_path)?;

    println!("Done!");
    Ok(())
}
