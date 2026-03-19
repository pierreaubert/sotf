use std::time::{Duration, Instant};

/// Toast message type for color coding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastType {
    Success,
    Error,
    Info,
    Warning,
}

/// An optional action button shown in the toast.
#[derive(Debug, Clone)]
pub struct ToastAction {
    pub label: String,
    pub action_id: String,
}

impl ToastAction {
    pub fn new(label: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action_id: action_id.into(),
        }
    }
}

/// Toast message with type, timing, and optional action button.
#[derive(Debug, Clone)]
pub struct ToastMessage {
    pub message: String,
    pub toast_type: ToastType,
    pub created_at: Instant,
    pub auto_dismiss_ms: Option<u64>, // None = no auto-dismiss
    pub action: Option<ToastAction>,
}

impl ToastMessage {
    pub fn new(message: String, toast_type: ToastType) -> Self {
        Self {
            message,
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: Some(5000), // Default 5 seconds
            action: None,
        }
    }

    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Success)
    }

    /// Errors never auto-dismiss — user must acknowledge or click action.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            toast_type: ToastType::Error,
            created_at: Instant::now(),
            auto_dismiss_ms: None, // Errors persist until dismissed
            action: None,
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Info)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message.into(), ToastType::Warning)
    }

    pub fn persistent(message: impl Into<String>, toast_type: ToastType) -> Self {
        Self {
            message: message.into(),
            toast_type,
            created_at: Instant::now(),
            auto_dismiss_ms: None, // No auto-dismiss
            action: None,
        }
    }

    /// Attach an action button (e.g., "Retry", "Details", "Undo").
    pub fn with_action(mut self, action: ToastAction) -> Self {
        self.action = action.into();
        self
    }

    pub fn should_dismiss(&self) -> bool {
        if let Some(dismiss_ms) = self.auto_dismiss_ms {
            self.created_at.elapsed() > Duration::from_millis(dismiss_ms)
        } else {
            false
        }
    }
}
