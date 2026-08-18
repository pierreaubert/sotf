use clap::Parser;
use std::path::PathBuf;

/// Audio recorder for test signals with analysis
#[derive(Parser)]
#[command(name = "sotf_recorder")]
#[command(about = "Generate and record test signals with analysis", long_about = None)]
pub(super) struct Cli {
    /// Signal type: tone, two-tone, sweep, white-noise, pink-noise, m-noise, mls, dirac
    #[arg(long)]
    pub(super) signal: Option<String>,

    /// Duration in seconds
    #[arg(long)]
    pub(super) duration: Option<f32>,

    /// Sample rate in Hz
    #[arg(long, default_value = "48000")]
    pub(super) sample_rate: u32,

    /// Number of signal channels (must be 1)
    #[arg(long, default_value = "1")]
    pub(super) channels: u16,

    /// Hardware output channel to send signal to (0-based, single channel only)
    #[arg(long)]
    pub(super) hwaudio_send_to: Option<String>,

    /// Hardware input channels to record from (0-based, comma-separated)
    #[arg(long)]
    pub(super) hwaudio_record_from: Option<String>,

    /// Optional filename prefix
    #[arg(long)]
    pub(super) name: Option<String>,

    /// Output directory for recorded WAV/CSV files (defaults to current directory)
    #[arg(long)]
    pub(super) output_dir: Option<PathBuf>,

    /// Audio device name (use --list-devices to see available devices). If not specified, uses default device.
    #[arg(long)]
    pub(super) device: Option<String>,

    /// List available audio devices and exit
    #[arg(long)]
    pub(super) list_devices: bool,

    // Signal-specific parameters
    /// Tone frequency in Hz (for tone signal)
    #[arg(long)]
    pub(super) freq: Option<f32>,

    /// First frequency in Hz (for two-tone signal)
    #[arg(long)]
    pub(super) freq1: Option<f32>,

    /// Second frequency in Hz (for two-tone signal)
    #[arg(long)]
    pub(super) freq2: Option<f32>,

    /// Start frequency in Hz (for sweep signal)
    #[arg(long, default_value_t = sotf_audio_player::recording_helpers::DEFAULT_SWEEP_START_FREQ)]
    pub(super) start_freq: f32,

    /// End frequency in Hz (for sweep signal)
    #[arg(long, default_value_t = sotf_audio_player::recording_helpers::DEFAULT_SWEEP_END_FREQ)]
    pub(super) end_freq: f32,

    /// Amplitude (0.0-1.0]
    #[arg(long)]
    pub(super) amp: Option<f32>,

    /// First amplitude (0.0-1.0, for two-tone signal)
    #[arg(long)]
    pub(super) amp1: Option<f32>,

    /// Second amplitude (0.0-1.0, for two-tone signal)
    #[arg(long)]
    pub(super) amp2: Option<f32>,

    /// MLS order (2-24, for MLS signal)
    #[arg(long)]
    pub(super) mls_order: Option<u8>,

    /// Microphone compensation file (freq/SPL pairs in CSV format)
    /// When provided, inverse compensation is applied to the CSV output.
    /// Applies to all channels as a default fallback.
    #[arg(long)]
    pub(super) microphone_compensation: Option<String>,

    /// Per-channel microphone calibration file in channel:path format.
    /// Can be specified multiple times. Example: --mic-calibration 0:/path/to/umik1.txt
    #[arg(long = "mic-calibration", value_name = "CHANNEL:PATH")]
    pub(super) mic_calibration: Vec<String>,
}
