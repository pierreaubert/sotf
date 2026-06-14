use super::types::PluginScanStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginScanSummary {
    pub total: usize,
    pub discovered: usize,
    pub loadable: usize,
    pub unsupported_by_build: usize,
}

impl PluginScanSummary {
    pub fn record(&mut self, status: PluginScanStatus) {
        self.total += 1;
        match status {
            PluginScanStatus::Discovered => self.discovered += 1,
            PluginScanStatus::Loadable => self.loadable += 1,
            PluginScanStatus::UnsupportedByBuild => self.unsupported_by_build += 1,
        }
    }
}
