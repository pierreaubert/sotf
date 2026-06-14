/// Result of a single diagnostic step.
#[derive(Clone, Debug)]
pub enum StepResult {
    /// Step passed.
    Ok(String),
    /// Step failed — subsequent steps were not attempted.
    Fail(String),
    /// Step was skipped (e.g., TLS not enabled).
    Skipped(String),
}

impl StepResult {
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Ok(m) | Self::Fail(m) | Self::Skipped(m) => m,
        }
    }
}
