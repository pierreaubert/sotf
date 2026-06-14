/// Priority for config updates
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ConfigUpdatePriority {
    FileWatcher = 1,  // Lowest priority - automatic file watching
    SignalReload = 2, // Medium priority - SIGHUP signal
    UserDirect = 3,   // Highest priority - direct API/command
}

/// Pending config update
#[derive(Debug)]
pub(super) struct PendingConfigUpdate {
    pub(super) plugins: Vec<super::super::PluginConfig>,
    pub(super) timestamp: std::time::Instant,
    pub(super) priority: ConfigUpdatePriority,
}
