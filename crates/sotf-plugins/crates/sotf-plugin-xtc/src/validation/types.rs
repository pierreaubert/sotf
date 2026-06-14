/// Validation categories for the full suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationCategory {
    Itd,
    Ild,
    CancellationDepth,
    SpatialCue,
    Stability,
}
