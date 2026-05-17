pub mod delay {
    pub const DELAY_MS_DEFAULT: f32 = 100.0;
    pub const DELAY_MS_MIN: f32 = 0.1;
    pub const DELAY_MS_MAX: f32 = 5000.0;

    pub const FEEDBACK_DEFAULT: f32 = 0.3;
    pub const FEEDBACK_MIN: f32 = 0.0;
    pub const FEEDBACK_MAX: f32 = 0.95;

    pub const MIX_DEFAULT: f32 = 0.5;
    pub const MIX_MIN: f32 = 0.0;
    pub const MIX_MAX: f32 = 1.0;

    pub const LFO_RATE_HZ_DEFAULT: f32 = 0.0;
    pub const LFO_RATE_HZ_MIN: f32 = 0.0;
    pub const LFO_RATE_HZ_MAX: f32 = 10.0;

    pub const LFO_DEPTH_MS_DEFAULT: f32 = 0.0;
    pub const LFO_DEPTH_MS_MIN: f32 = 0.0;
    pub const LFO_DEPTH_MS_MAX: f32 = 5.0;

    pub const ALLPASS_FEEDBACK_DEFAULT: bool = false;

    pub const ALLPASS_COEFF_DEFAULT: f32 = 0.5;
    pub const ALLPASS_COEFF_MIN: f32 = 0.0;
    pub const ALLPASS_COEFF_MAX: f32 = 0.99;
}
