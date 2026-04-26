/// Focused high-frequency stationary noise reducer.
///
/// This is intentionally simpler than the full STFT denoiser: it splits each
/// channel into a low-passed body and a high-frequency residual, tracks the
/// residual envelope slowly, and attenuates only the residual when it looks
/// stationary and low-level.
pub struct HissReducer {
    channels: usize,
    sample_rate: u32,
    cutoff_hz: f32,
    threshold_db: f32,
    strength: f32,
    lowpass_state: Vec<f32>,
    noise_env: Vec<f32>,
    alpha: f32,
    threshold_linear: f32,
}

impl HissReducer {
    pub fn new(channels: usize) -> Self {
        let mut reducer = Self {
            channels,
            sample_rate: 48000,
            cutoff_hz: 4000.0,
            threshold_db: -30.0,
            strength: 0.5,
            lowpass_state: vec![0.0; channels],
            noise_env: vec![0.0; channels],
            alpha: 0.0,
            threshold_linear: 0.0,
        };
        reducer.update_coefficients();
        reducer
    }

    pub fn initialize(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate.max(1);
        self.update_coefficients();
    }

    pub fn set_params(&mut self, cutoff_hz: f32, threshold_db: f32, strength: f32) {
        self.cutoff_hz = cutoff_hz.max(20.0);
        self.threshold_db = threshold_db;
        self.strength = strength.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    pub fn reset(&mut self) {
        self.lowpass_state.fill(0.0);
        self.noise_env.fill(0.0);
    }

    pub fn process(&mut self, buffer: &mut [f32]) {
        if self.channels == 0 {
            return;
        }

        for frame in buffer.chunks_mut(self.channels) {
            for (ch, sample) in frame.iter_mut().enumerate() {
                let low = self.alpha * *sample + (1.0 - self.alpha) * self.lowpass_state[ch];
                self.lowpass_state[ch] = low;

                let high = *sample - low;
                let high_abs = high.abs();
                self.noise_env[ch] = self.noise_env[ch] * 0.999 + high_abs * 0.001;

                let stationary_ratio = self.noise_env[ch] / (high_abs + 1e-9);
                let below_threshold = self.noise_env[ch] < self.threshold_linear;
                let stationary = stationary_ratio > 0.25;

                let attenuation = if below_threshold && stationary {
                    1.0 - self.strength
                } else {
                    1.0
                };
                *sample = low + high * attenuation;
            }
        }
    }

    fn update_coefficients(&mut self) {
        let sr = self.sample_rate.max(1) as f32;
        let cutoff = self.cutoff_hz.min(sr * 0.45);
        let rc = 1.0 / (2.0 * std::f32::consts::PI * cutoff.max(20.0));
        let dt = 1.0 / sr;
        self.alpha = dt / (rc + dt);
        self.threshold_linear = 10.0_f32.powf(self.threshold_db / 20.0);
    }
}
