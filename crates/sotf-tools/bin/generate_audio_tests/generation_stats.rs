#[derive(Debug)]
pub(super) struct GenerationStats {
    pub(super) generated: usize,
    pub(super) skipped: usize,
    pub(super) failed: usize,
}

impl GenerationStats {
    pub(super) fn new() -> Self {
        Self {
            generated: 0,
            skipped: 0,
            failed: 0,
        }
    }
}
