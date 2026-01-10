//! Manager Protocol Definition
//!
//! Standard interface for state managers in the application architecture.

#[derive(Debug, Clone)]
pub struct ManagerError {
    pub message: String,
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ManagerError: {}", self.message)
    }
}

impl std::error::Error for ManagerError {}

impl From<String> for ManagerError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for ManagerError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

/// Standard protocol for state managers
pub trait Manager {
    type State;
    type Event;
    type Query;
    type Response;

    /// Handle a state-modifying event
    fn handle_event(&mut self, event: Self::Event) -> Result<(), ManagerError>;

    /// handle a read-only query
    fn query(&self, query: Self::Query) -> Self::Response;

    /// Get reference to the underlying state
    fn state(&self) -> &Self::State;
}
