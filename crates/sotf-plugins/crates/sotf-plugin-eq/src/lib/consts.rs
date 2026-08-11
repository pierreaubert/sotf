pub(super) const DEFAULT_SAMPLE_RATE: u32 = 44100;

pub(super) const MEASUREMENT_THROTTLE: usize = 10;

/// Duration of coefficient interpolation in seconds (~5ms)
pub(super) const TRANSITION_DURATION_SECS: f64 = 0.005;

pub(super) const FREQ_MIN: f32 = 20.0;

pub(super) const FREQ_MAX: f32 = 20000.0;

pub(super) const Q_MIN: f32 = 0.1;

/// Validation/load ceiling for every filter type except Notch. Matches the
/// optimizers' `max_q` ceiling (20.0) so optimized chains load unclamped.
/// The UI edit ceiling for these types is lower (10.0, see params.rs).
pub(super) const Q_MAX: f32 = 20.0;

/// Notch filters allow much higher Q (extremely narrow rejection bands).
pub(super) const Q_MAX_NOTCH: f32 = 40.0;

pub(super) const GAIN_MIN: f32 = -24.0;

pub(super) const GAIN_MAX: f32 = 24.0;
