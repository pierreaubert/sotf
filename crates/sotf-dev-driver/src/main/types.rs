use clap::{Parser, Subcommand};
use sotf_dev_driver::fuzz::TargetId;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "sotf-dev-driver", version)]
pub(super) struct Args {
    #[command(subcommand)]
    pub(super) command: Option<Command>,
    /// Path to a .scn scenario file (legacy single-scenario mode).
    pub(super) script: Option<PathBuf>,
    /// Base URL of the running SotF dev API.
    #[arg(long)]
    pub(super) url: Option<String>,
    /// Print every verb + result.
    #[arg(short, long)]
    pub(super) verbose: bool,
}

#[derive(Subcommand, Debug)]
pub(super) enum Command {
    /// Start SotF with --qa and run every scenario in a suite TOML file.
    RunSuite {
        /// Path to a suite TOML file.
        suite: PathBuf,
        /// Print process and scenario details.
        #[arg(short, long)]
        verbose: bool,
    },
    /// Run the seeded state-aware all-app fuzzer.
    Fuzz {
        /// Stable target ID from the checked-in surface inventory.
        #[arg(long)]
        target: TargetId,
        /// Root deterministic seed.
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Maximum actions per worker.
        #[arg(long, default_value_t = 1_000)]
        steps: u64,
        /// Optional wall-clock budget (for example 30s or 5m).
        #[arg(long)]
        time: Option<String>,
        /// Explicit worker count; defaults to one for isolation.
        #[arg(long, default_value_t = 1)]
        workers: u32,
        /// Fixture profile declared by the surface manifest.
        #[arg(long, default_value = "default")]
        fixture: String,
        /// Artifact root. Every worker creates a private run below it.
        #[arg(long, default_value = "target/sotf-fuzz")]
        artifacts: PathBuf,
        /// Override the checked-in target surface manifest.
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Launch this already-built target binary.
        #[arg(long)]
        executable: Option<PathBuf>,
        /// Drive an already-running authenticated dev API instead of launching.
        #[arg(long)]
        url: Option<String>,
        /// Sync every trace event to storage after flushing.
        #[arg(long)]
        durable_trace: bool,
        /// Explicitly permit physical audio hardware.
        #[arg(long)]
        allow_hardware_audio: bool,
        /// Explicitly permit non-loopback networking/accounts.
        #[arg(long)]
        allow_network: bool,
        /// Explicitly permit loading installed external plugins.
        #[arg(long)]
        allow_external_plugins: bool,
        /// Explicitly permit a real HAL installation.
        #[arg(long)]
        allow_hal_install: bool,
        /// Explicitly permit physical iOS/tvOS devices.
        #[arg(long)]
        allow_physical_device: bool,
    },
    /// Replay the exact resolved actions recorded in replay.toml.
    Replay {
        replay: PathBuf,
        #[arg(long)]
        executable: Option<PathBuf>,
        #[arg(long)]
        url: Option<String>,
        /// Continue diagnostically when capabilities differ from the recording.
        #[arg(long)]
        best_effort_capabilities: bool,
    },
    /// Delta-minimize a recorded crash/hang/invariant trace.
    Minimize {
        replay: PathBuf,
        #[arg(long)]
        executable: Option<PathBuf>,
        #[arg(long)]
        url: Option<String>,
    },
}

pub(super) struct Ctx {
    pub(super) client: reqwest::blocking::Client,
    pub(super) base: String,
    pub(super) verbose: bool,
}

pub(super) enum ExpectedValue {
    Bool(bool),
    Number(f64),
    String(String),
    Null,
}
