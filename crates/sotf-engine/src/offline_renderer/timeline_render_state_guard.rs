
pub(super) struct TimelineRenderStateGuard<'a> {
    pub(super) timeline: &'a mut crate::timeline::Timeline,
    pub(super) saved_loop: Option<(u64, u64)>,
}

impl<'a> TimelineRenderStateGuard<'a> {
    pub(super) fn new(timeline: &'a mut crate::timeline::Timeline) -> Self {
        let saved_loop = timeline.transport.loop_range.take();
        timeline.seek(0);
        timeline.transport.play();
        Self {
            timeline,
            saved_loop,
        }
    }
}

impl Drop for TimelineRenderStateGuard<'_> {
    fn drop(&mut self) {
        self.timeline.transport.pause();
        self.timeline.transport.loop_range = self.saved_loop.take();
    }
}

