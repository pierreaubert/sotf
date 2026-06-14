/// Wrapper for InteractiveChartState that implements Debug
#[derive(Clone)]
pub struct InteractiveChartStateWrapper(pub gpui_px::interaction::InteractiveChartState);

impl std::fmt::Debug for InteractiveChartStateWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InteractiveChartStateWrapper")
            .field("is_zoomed", &self.0.is_zoomed())
            .finish()
    }
}

impl InteractiveChartStateWrapper {
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self(gpui_px::interaction::InteractiveChartState::new(
            x_min, x_max, y_min, y_max,
        ))
    }

    pub fn with_log_x(mut self, is_log: bool) -> Self {
        self.0 = self.0.with_log_x(is_log);
        self
    }

    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.0 = self.0.with_size(width, height);
        self
    }

    pub fn inner(&self) -> &gpui_px::interaction::InteractiveChartState {
        &self.0
    }
}
