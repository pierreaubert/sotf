/// Single first-order allpass stage: y[n] = -a*x[n] + x[n-1] + a*y[n-1]
/// where a = (tan(π·fc/fs) - 1) / (tan(π·fc/fs) + 1).
/// Phase at fc is exactly -90°; approaches 0° at DC, -180° at Nyquist.
pub(super) struct AllpassStage {
    pub(super) coeff_a: f32,
    pub(super) x_prev: f32,
    pub(super) y_prev: f32,
}

impl AllpassStage {
    pub(super) fn new(fc: f32, sample_rate: u32) -> Self {
        Self {
            coeff_a: Self::compute_coeff(fc, sample_rate),
            x_prev: 0.0,
            y_prev: 0.0,
        }
    }

    pub(super) fn compute_coeff(fc: f32, sample_rate: u32) -> f32 {
        let t = (std::f32::consts::PI * fc / sample_rate as f32).tan();
        (t - 1.0) / (t + 1.0)
    }

    pub(super) fn update_sample_rate(&mut self, fc: f32, sample_rate: u32) {
        self.coeff_a = Self::compute_coeff(fc, sample_rate);
    }

    #[inline]
    pub(super) fn process(&mut self, x: f32) -> f32 {
        let y = -self.coeff_a * x + self.x_prev + self.coeff_a * self.y_prev;
        self.x_prev = x;
        self.y_prev = y;
        y
    }

    pub(super) fn reset(&mut self) {
        self.x_prev = 0.0;
        self.y_prev = 0.0;
    }
}
