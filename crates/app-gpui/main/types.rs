use super::misc::parse_size;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "SotF")]
#[command(version, about = "SOTF GPUI Music Player", long_about = None)]
pub(super) struct Args {
    /// Use a custom data directory (for QA testing)
    #[arg(long)]
    pub(super) qa: Option<PathBuf>,

    /// Exercise first-launch onboarding in a hermetic dev-api QA run.
    #[cfg(feature = "dev-api")]
    #[arg(long, hide = true)]
    pub(super) qa_onboarding: bool,

    /// Run in headless server mode (MPD/DLNA) without UI
    #[arg(long)]
    pub(super) server: bool,

    /// Override the initial window size as WIDTHxHEIGHT (e.g. 1440x900).
    /// Takes precedence over the size stored in the preferences file.
    #[arg(long, value_name = "WIDTHxHEIGHT", value_parser = parse_size)]
    pub(super) size: Option<(f32, f32)>,
}
