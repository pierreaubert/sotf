/// Canonical error string returned when the recording-side cancel flag
/// is observed during a capture. Stable so UI code can match on it.
pub const CANCELLED_ERR: &str = "cancelled";

/// Default level for auxiliary recording stimuli (delay probe and bass anchor).
///
/// These are short/narrowband signals and are noticeably quieter than sweeps
/// when played at the same peak level, so keep the historical 0.5 linear
/// amplitude default unless a caller explicitly chooses otherwise.
pub const DEFAULT_AUXILIARY_SIGNAL_LEVEL_DB: f32 = -6.0206;

/// Default MLS order for recording workflows.
pub const DEFAULT_MLS_ORDER: u8 = 16;

/// Default number of repeated sweeps captured per channel when the caller
/// does not specify a count (Task 8). Four takes is the smallest count for
/// which math-dsp's robust averaging also yields a per-bin coherence
/// estimate (`average_ess_recordings` computes coherence only from ≥ 4
/// accepted takes), and it matches autoeq's
/// `RecordingConfiguration.num_sweeps` default of 4.
pub const DEFAULT_NUM_SWEEPS: u16 = 4;

/// Minimum sweep count once the repeat path is engaged.
///
/// With two takes math-dsp's median/MAD outlier rejection always admits
/// both (zero breakdown), so a single corrupt take would silently poison
/// the averaged capture; three is the smallest count that can reject one
/// bad take. Callers requesting 2 are bumped here with a warning.
pub const MIN_REPEAT_SWEEPS: u16 = 3;

/// Fixed seed for allpass probe generation — ensures the same probe is used
/// for generation and matched-filter detection.
pub const PROBE_SEED: u64 = 0xDEAD_BEEF_CAFE_1337;
