use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub(super) struct Args {
    /// Import and validate a generic gpui-toolkit DesignSystem token document.
    #[arg(long)]
    pub(super) toolkit: bool,

    /// Input token path. Defaults to the legacy SOTF app token file or toolkit token file.
    #[arg(short, long)]
    pub(super) input: Option<PathBuf>,

    /// Generic toolkit token format when --toolkit is used.
    #[arg(long, default_value = "style-dictionary-json")]
    pub(super) format: String,
}

#[derive(Clone, Copy)]
pub(super) struct ThemeConfig {
    pub(super) set_name: &'static str,
    pub(super) fn_name: &'static str,
    pub(super) file_name: &'static str,
    pub(super) doc_comment: &'static str,
}
