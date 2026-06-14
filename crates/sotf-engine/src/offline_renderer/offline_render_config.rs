use crate::decoder::source::AudioSource;
use crate :: engine :: { PluginConfig } ;
use std :: path :: { PathBuf } ;
use super::types::OutputFormat;

/// Configuration for offline rendering.
#[derive(Debug, Clone)]
pub struct OfflineRenderConfig {
    /// Audio source to render
    pub source: AudioSource,
    /// Output file path
    pub output_path: PathBuf,
    /// Output format
    pub format: OutputFormat,
    /// Plugin chain to apply during rendering
    pub plugins: Vec<PluginConfig>,
    /// Output sample rate (None = use source sample rate)
    pub output_sample_rate: Option<u32>,
    /// Processing block size in frames (default 1024)
    pub frame_size: usize,
}

impl OfflineRenderConfig {
    pub fn new(source: AudioSource, output_path: impl Into<PathBuf>) -> Self {
        Self {
            source,
            output_path: output_path.into(),
            format: OutputFormat::Wav {
                bits_per_sample: 32,
            },
            plugins: Vec::new(),
            frame_size: 1024,
            output_sample_rate: None,
        }
    }
}

