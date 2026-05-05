//! Constants - Algorithm and model option arrays for AutoEQ forms.

/// Optimization type - determines which options are shown in the form
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptimizationType {
    /// Speaker optimization - shows system type, speaker-specific target curves
    #[default]
    Speaker,
    /// Headphone optimization - hides system type, shows Harman target curves
    Headphone,
}

/// Optimization mode options
pub const OPT_MODE_OPTIONS: &[(&str, &str)] = &[
    ("iir", "IIR (PEQ)"),
    ("fir", "FIR (Convolution)"),
    ("mixed", "Mixed (IIR + FIR)"),
    ("mixed_phase", "Mixed Phase (IIR + Excess Phase FIR)"),
];

/// FIR Phase options
pub const FIR_PHASE_OPTIONS: &[(&str, &str)] = &[
    ("linear", "Linear Phase"),
    ("minimum", "Minimum Phase"),
    ("kirkeby", "Kirkeby Inverse"),
];

/// Loss Type options for speakers (Room EQ / Spinorama)
pub const LOSS_TYPE_OPTIONS: &[(&str, &str)] = &[
    ("flat", "Flat Target Match"),
    ("flat-asymmetric", "Natural Correction"),
    ("score", "Listener Preference"),
    ("epa", "Perceptual (EPA)"),
];

/// Loss Type options for headphones
pub const HEADPHONE_LOSS_TYPE_OPTIONS: &[(&str, &str)] =
    &[("flat", "Target Match"), ("score", "Score")];

/// Short descriptions for loss types (used as tooltips / inline help)
pub const LOSS_TYPE_DESCRIPTIONS: &[(&str, &str)] = &[
    ("flat", "Minimize deviation from target curve"),
    (
        "flat-asymmetric",
        "Tolerates dips, penalizes peaks \u{2014} better for rooms",
    ),
    ("score", "Optimize for how listeners rate the sound"),
    (
        "epa",
        "Psychoacoustic optimization: loudness, sharpness, roughness",
    ),
];

/// Target curve options for headphones (Harman curves)
pub const HEADPHONE_TARGET_CURVE_OPTIONS: &[(&str, &str)] = &[
    ("flat", "Flat"),
    ("harman-over-ear-2018", "Harman Over-Ear 2018"),
    ("harman-over-ear-2015", "Harman Over-Ear 2015"),
    ("harman-over-ear-2013", "Harman Over-Ear 2013"),
    ("harman-in-ear-2019", "Harman In-Ear 2019"),
    ("custom", "Custom (File Path)"),
];

/// Base target curve options for speakers (always available)
pub const SPEAKER_TARGET_CURVE_OPTIONS: &[(&str, &str)] =
    &[("flat", "Flat (0 dB)"), ("custom", "Custom (Manual Entry)")];

/// Spinorama curve options for speakers (available when spinorama data is loaded)
pub const SPINORAMA_CURVE_OPTIONS: &[(&str, &str)] = &[
    ("ON", "On-Axis (ON)"),
    ("LW", "Listening Window (LW)"),
    ("ER", "Early Reflections (ER)"),
    ("SP", "Sound Power (SP)"),
    ("PIR", "Predicted In-Room (PIR)"),
];

/// System Type options
pub const SYSTEM_TYPE_OPTIONS: &[(&str, &str)] = &[
    ("stereo", "Stereo / Independent"),
    ("multisub", "Multi-Subwoofer"),
    ("dba", "Double Bass Array"),
];

/// Tilt Type options
pub const TILT_TYPE_OPTIONS: &[(&str, &str)] = &[
    ("flat", "Flat (None)"),
    ("harman", "Harman (-0.8 dB/oct)"),
    ("custom", "Custom Tilt"),
];

/// Tilt Type options for Room EQ (no flat — use slopes only)
pub const ROOMEQ_TILT_TYPE_OPTIONS: &[(&str, &str)] = &[
    ("harman", "Harman (-0.8 dB/oct)"),
    ("custom", "Custom Tilt"),
];

/// Highpass Filter options
pub const HIGHPASS_TYPE_OPTIONS: &[(&str, &str)] =
    &[("lr", "Linkwitz-Riley"), ("bw", "Butterworth")];

/// Multi-Seat Strategy options
pub const MULTI_SEAT_STRATEGY_OPTIONS: &[(&str, &str)] = &[
    ("variance", "Minimize Variance"),
    ("primary", "Primary + Constraints"),
    ("average", "Average Response"),
    ("modal_basis", "Modal Basis / SFM"),
];

/// Algorithm options for optimization
pub const ALGORITHM_OPTIONS: &[(&str, &str)] = &[
    ("autoeq:de", "Auto DE (Recommended)"),
    ("autoeq:bo", "Bayesian Optimization"),
    ("mh:de", "MH Differential Evolution"),
    ("mh:pso", "MH Particle Swarm"),
    ("mh:rga", "MH Genetic Algorithm"),
    ("mh:tlbo", "MH TLBO"),
    ("mh:firefly", "MH Firefly"),
    ("nlopt:isres", "NLOPT ISRES"),
    ("nlopt:ags", "NLOPT AGS"),
    ("nlopt:cobyla", "NLOPT COBYLA"),
    ("nlopt:bobyqa", "NLOPT BOBYQA"),
    ("nlopt:neldermead", "NLOPT Nelder-Mead"),
];

/// Bayesian optimization acquisition options
pub const BO_ACQUISITION_OPTIONS: &[(&str, &str)] = &[
    ("qei", "MC q-EI"),
    ("ei", "Expected Improvement"),
    ("thompson", "Thompson Sampling"),
];

/// DE strategy options (all variants from math-optimisation Strategy enum)
pub const DE_STRATEGY_OPTIONS: &[(&str, &str)] = &[
    ("currenttobest1bin", "Current-to-Best/1/Bin (Recommended)"),
    ("currenttobest1exp", "Current-to-Best/1/Exp"),
    ("best1bin", "Best/1/Bin"),
    ("best1exp", "Best/1/Exp"),
    ("best2bin", "Best/2/Bin"),
    ("best2exp", "Best/2/Exp"),
    ("rand1bin", "Rand/1/Bin"),
    ("rand1exp", "Rand/1/Exp"),
    ("rand2bin", "Rand/2/Bin"),
    ("rand2exp", "Rand/2/Exp"),
    ("randtobest1bin", "Rand-to-Best/1/Bin"),
    ("randtobest1exp", "Rand-to-Best/1/Exp"),
    ("adaptivebin", "Adaptive/Bin"),
    ("adaptiveexp", "Adaptive/Exp"),
    ("lshadebin", "L-SHADE/Bin"),
    ("lshadeexp", "L-SHADE/Exp"),
];

/// PEQ model options with human-readable labels
pub const PEQ_MODEL_OPTIONS: &[(&str, &str)] = &[
    ("pk", "Peaks Only"),
    ("hp-pk", "Highpass + Peaks"),
    ("ls-pk", "Bass Shelf + Peaks"),
    ("hp-pk-lp", "Bandpass + Peaks"),
    ("ls-pk-hs", "Shelves + Peaks"),
    ("free-pk-free", "Flexible Ends"),
    ("free", "Fully Automatic"),
];

/// Short descriptions for PEQ models (used as tooltips / inline help)
pub const PEQ_MODEL_DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "pk",
        "All filters are bell/peak type. Simplest and most compatible.",
    ),
    (
        "hp-pk",
        "Adds a bass rolloff filter. Good for limited low extension.",
    ),
    ("ls-pk", "Adds a low shelf for broad bass adjustment."),
    ("hp-pk-lp", "Constrains both low and high ends."),
    (
        "ls-pk-hs",
        "Low and high shelves plus peaks. Most flexible.",
    ),
    ("free-pk-free", "End filters auto-select their type."),
    (
        "free",
        "Every filter chooses its own type. Longest optimization.",
    ),
];

/// Mixed mode crossover type options
pub const MIXED_CROSSOVER_TYPE_OPTIONS: &[(&str, &str)] = &[
    ("LR24", "Linkwitz-Riley 24dB"),
    ("LR48", "Linkwitz-Riley 48dB"),
];

/// Mixed mode FIR band options
pub const MIXED_FIR_BAND_OPTIONS: &[(&str, &str)] = &[
    ("low", "Low Frequencies (Bass)"),
    ("high", "High Frequencies"),
];

/// Multi-Measurement Strategy options
pub const MULTI_MEASUREMENT_STRATEGY_OPTIONS: &[(&str, &str)] = &[
    ("average", "Average (RMS)"),
    ("weighted_sum", "Weighted Sum"),
    ("minimax", "Minimax (Worst Case)"),
    ("variance_penalized", "Variance Penalized"),
];

/// Local algorithm options for refinement
pub const LOCAL_ALGO_OPTIONS: &[(&str, &str)] = &[
    ("cobyla", "COBYLA"),
    ("bobyqa", "BOBYQA"),
    ("newuoa", "NEWUOA"),
];

/// Target distance presets: (id, label, description, recommended_slope_db_per_oct)
pub const TARGET_DISTANCE_OPTIONS: &[(&str, &str, &str, f64)] = &[
    ("near", "Near-Field", "~1m \u{2014} Good for desk", -0.5),
    (
        "mid",
        "Mid-Field",
        "1.2~3m \u{2014} Good for large desk, console",
        -0.8,
    ),
    (
        "far",
        "Far-Field",
        ">3m \u{2014} Classical Home Cinema or Dining room",
        -1.2,
    ),
    ("custom", "Custom", "Set your own slope", 0.0),
];

/// Optimisation goal presets: (id, label, description)
pub const OPTIMIZATION_GOAL_OPTIONS: &[(&str, &str, &str)] = &[
    (
        "match_target",
        "Match Target",
        "As close as possible from target",
    ),
    (
        "natural",
        "Natural Correction",
        "Correct dips more than peaks",
    ),
    (
        "psychoacoustic",
        "Psychoacoustic Correction",
        "Take psychoacoustics into account",
    ),
];

/// Loss Type options for spinorama (Target or Score only)
pub const SPINORAMA_LOSS_TYPE_OPTIONS: &[(&str, &str)] = &[("flat", "Target"), ("score", "Score")];

/// Smoothing strategy options
pub const SMOOTHING_STRATEGY_OPTIONS: &[(&str, &str)] =
    &[("psychoacoustic", "Psychoacoustic"), ("window", "Window")];
