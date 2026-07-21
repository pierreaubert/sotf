use crate::PluginBuildDiagnostic;

/// Structured config error types
#[derive(Debug, Clone)]
pub(super) enum ConfigError {
    /// Failed to parse config file
    ParseError {
        path: std::path::PathBuf,
        reason: String,
    },
    /// Config validation failed
    ValidationError { plugin_index: usize, reason: String },
    /// Plugin update timed out
    TimeoutError { waited_ms: u64 },
    /// Plugin update failed in processing thread
    ProcessingError { reason: String },
    /// Plugin host construction failed before the candidate could be applied.
    PluginBuild { diagnostic: PluginBuildDiagnostic },
    /// Unexpected response from processing thread
    UnexpectedResponse,
    /// Communication channel disconnected
    ChannelDisconnected,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ParseError { path, reason } => {
                write!(f, "Failed to parse config {:?}: {}", path, reason)
            }
            Self::ValidationError {
                plugin_index,
                reason,
            } => {
                write!(f, "Plugin {} validation failed: {}", plugin_index, reason)
            }
            Self::TimeoutError { waited_ms } => {
                write!(f, "Plugin update timed out after {}ms", waited_ms)
            }
            Self::ProcessingError { reason } => {
                write!(f, "Plugin processing error: {}", reason)
            }
            Self::PluginBuild { diagnostic } => {
                write!(f, "Plugin build error: {}", diagnostic)
            }
            Self::UnexpectedResponse => {
                write!(f, "Unexpected response from processing thread")
            }
            Self::ChannelDisconnected => {
                write!(f, "Communication channel disconnected")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl ConfigError {
    pub(super) fn is_plugin_build_error(&self) -> bool {
        matches!(self, Self::PluginBuild { .. })
    }
}
