
/// Progress information passed to the callback during rendering.
#[derive(Debug, Clone)]
pub struct RenderProgress {
    /// Number of frames processed so far
    pub frames_processed: u64,
    /// Total frames in the source (if known)
    pub total_frames: Option<u64>,
}

impl RenderProgress {
    /// Returns completion percentage (0.0 to 100.0) if total is known.
    pub fn percent(&self) -> Option<f32> {
        self.total_frames
            .map(|t| (self.frames_processed as f32 / t.max(1) as f32) * 100.0)
    }
}

