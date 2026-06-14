use serde::{Deserialize, Serialize};
use std::fmt;

/// Controls where auto-gain measurement and compensation are applied
/// relative to the EQ filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoGainPosition {
    /// Measure input before filters, apply compensation after filters (current default)
    Post,
    /// Measure and apply compensation before filters (pre-filter gain matching)
    Pre,
    /// Auto-gain disabled
    Disabled,
}

impl fmt::Display for AutoGainPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutoGainPosition::Post => write!(f, "post"),
            AutoGainPosition::Pre => write!(f, "pre"),
            AutoGainPosition::Disabled => write!(f, "disabled"),
        }
    }
}

impl AutoGainPosition {
    pub(super) fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "pre" => AutoGainPosition::Pre,
            "disabled" | "off" => AutoGainPosition::Disabled,
            _ => AutoGainPosition::Post,
        }
    }
}
