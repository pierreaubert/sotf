mod dev_api;
mod process;
mod server;
#[cfg(unix)]
mod systemwide;

pub use dev_api::{DevApiTarget, DevApiTargetConfig};
pub use process::{ProcessTarget, ProcessTargetConfig};
pub use server::{ServerTarget, ServerTargetConfig};
#[cfg(unix)]
pub use systemwide::{SystemwideTarget, SystemwideTargetConfig};

use std::path::Path;

use sotf_dev_api::{Capabilities, Snapshot};

use super::model::{Action, Observation, StructuredSkip, TargetId, TargetSpec};
use super::supervisor::{FuzzTarget, LaunchContext, TargetError};

/// Map socket read/write timeouts to `TargetError::Timeout` so a slow command
/// reaches the supervisor's stall/hang state machine instead of being
/// reported as an unexpected exit. Connect failures stay `Io`: against a dead
/// child they correctly classify as an exit.
pub(super) fn map_io_timeout(error: std::io::Error) -> TargetError {
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        TargetError::Timeout(error.to_string())
    } else {
        TargetError::Io(error)
    }
}

/// Same mapping for helpers that already return `TargetError`.
pub(super) fn map_target_timeout(error: TargetError) -> TargetError {
    match error {
        TargetError::Io(error) => map_io_timeout(error),
        other => other,
    }
}

pub struct UnsupportedTarget {
    target: TargetId,
    reason_code: String,
    reason: String,
}

impl UnsupportedTarget {
    pub fn new(
        target: TargetId,
        reason_code: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            target,
            reason_code: reason_code.into(),
            reason: reason.into(),
        }
    }
}

impl FuzzTarget for UnsupportedTarget {
    fn target_id(&self) -> TargetId {
        self.target
    }

    fn launch(&mut self, _context: &LaunchContext<'_>) -> Result<TargetSpec, TargetError> {
        Err(TargetError::Unsupported(StructuredSkip {
            target_id: self.target,
            reason_code: self.reason_code.clone(),
            reason: self.reason.clone(),
        }))
    }

    fn capabilities(&mut self) -> Result<Capabilities, TargetError> {
        Err(TargetError::Protocol(
            "unsupported target was not launched".into(),
        ))
    }

    fn snapshot(&mut self) -> Result<Snapshot, TargetError> {
        Err(TargetError::Protocol(
            "unsupported target was not launched".into(),
        ))
    }

    fn execute(&mut self, _action: &Action) -> Result<Observation, TargetError> {
        Err(TargetError::Protocol(
            "unsupported target was not launched".into(),
        ))
    }

    fn live(&mut self) -> Result<bool, TargetError> {
        Ok(false)
    }

    fn capture_hang(&mut self, _directory: &Path) -> Result<Vec<std::path::PathBuf>, TargetError> {
        Ok(vec![])
    }

    fn shutdown(&mut self) -> Result<(), TargetError> {
        Ok(())
    }
}
