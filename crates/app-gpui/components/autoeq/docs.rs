//! Static documentation content for AutoEQ parameter blocks.
//!
//! Each block of related parameters has a [`BlockDoc`] with a title, overview,
//! and per-field documentation. The docs panel renders the block matching
//! the currently focused section.

/// Documentation for a single form field.
pub struct FieldDoc {
    /// Field label (matches the UI label).
    pub name: &'static str,
    /// What this field controls.
    pub description: &'static str,
    /// Typical or default value.
    pub default: &'static str,
    /// Practical advice.
    pub tip: &'static str,
}

/// Documentation for a block of related parameters.
pub struct BlockDoc {
    /// Block identifier — matches the `focused_block` key.
    pub id: &'static str,
    /// Block title displayed at the top of the docs panel.
    pub title: &'static str,
    /// Overview paragraph explaining the block's purpose.
    pub overview: &'static str,
    /// Per-field documentation.
    pub fields: &'static [FieldDoc],
}

/// Key used when no block is focused — shows the overview.
pub const BLOCK_OVERVIEW: &str = "overview";
pub const BLOCK_PRESET: &str = "preset";
pub const BLOCK_GOALS: &str = "goals";
pub const BLOCK_EQ_DESIGN: &str = "eq-design";
pub const BLOCK_RANGES: &str = "ranges";
pub const BLOCK_PEQ_MODEL: &str = "peq-model";
pub const BLOCK_OPTIMIZER: &str = "optimizer";
pub const BLOCK_DE_PARAMS: &str = "de-params";
pub const BLOCK_REFINEMENT: &str = "refinement";
pub const BLOCK_SMOOTHING: &str = "smoothing";
pub const BLOCK_TARGET_TILT: &str = "target-tilt";
pub const BLOCK_EXCURSION: &str = "excursion";
pub const BLOCK_SCHROEDER: &str = "schroeder";
pub const BLOCK_PHASE_ALIGNMENT: &str = "phase-alignment";
pub const BLOCK_MULTI_SEAT: &str = "multi-seat";

/// All block documentation, in display order.
pub static BLOCK_DOCS: &[BlockDoc] = &[
    BlockDoc {
        id: BLOCK_OVERVIEW,
        title: "AutoEQ Parameters",
        overview: "Configure the parametric EQ optimizer. Use the mode toggle to switch \
                   between Simple (presets only), Customize (key parameters), and All \
                   Parameters (full expert control). Hover over a section on the left \
                   to see detailed help here.",
        fields: &[],
    },
    BlockDoc {
        id: BLOCK_PRESET,
        title: "Presets",
        overview: "Presets bundle all optimizer parameters into a single choice. Select \
                   a preset to get started quickly, then switch to Customize or All \
                   Parameters if you want to fine-tune.",
        fields: &[
            FieldDoc {
                name: "Quick / Quick Fix",
                description: "Fast correction with fewer filters. Good results in seconds.",
                default: "5 filters, no refinement",
                tip: "Use this for a quick preview before committing to a longer run.",
            },
            FieldDoc {
                name: "Balanced",
                description: "Good balance of quality and speed. Recommended for most users.",
                default: "7 filters, refinement enabled",
                tip: "Start here if you're unsure. Covers most use cases well.",
            },
            FieldDoc {
                name: "Maximum Quality / Audiophile",
                description: "Best possible correction with more filters and longer \
                              optimization. Use when quality matters more than speed.",
                default: "10 filters, shelves, full refinement",
                tip: "Can take several minutes. The result is typically the best \
                      achievable with parametric EQ.",
            },
            FieldDoc {
                name: "Custom",
                description: "Unlocks all parameters for full expert control.",
                default: "",
                tip: "Switches the form to All Parameters mode automatically.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_GOALS,
        title: "Goals & Configuration",
        overview: "Defines what the optimizer tries to achieve and the target response \
                   it matches against.",
        fields: &[
            FieldDoc {
                name: "System Type",
                description: "The loudspeaker topology. Affects how measurements are \
                              combined and which loss functions are available.",
                default: "stereo",
                tip: "Use 'multisub' when optimizing a system with independently \
                      placed subwoofers. 'DBA' is for double bass arrays.",
            },
            FieldDoc {
                name: "Loss Function",
                description: "'Flat Target Match' minimizes deviation from the target. \
                              'Natural Correction' tolerates dips but penalizes peaks \
                              (best for rooms where nulls can't be fixed). \
                              'Listener Preference' optimizes the Harman score with a \
                              natural bass shelf. 'Perceptual (EPA)' uses psychoacoustic \
                              metrics: loudness balance, sharpness, and roughness.",
                default: "Flat Target Match",
                tip: "Start with 'Flat Target Match' for accuracy. Try 'Natural \
                      Correction' for room EQ. 'Listener Preference' adds a bass \
                      shelf. 'Perceptual' is experimental and best for advanced users.",
            },
            FieldDoc {
                name: "Target Curve",
                description: "The reference frequency response the optimizer tries to \
                              match. Harman curves are research-based preference targets. \
                              'Flat' targets a ruler-flat response.",
                default: "Harman 2018 (headphones) / flat (speakers)",
                tip: "For headphones, Harman 2018 is the most validated target. For \
                      speakers, 'flat' is typical unless you have a preferred house curve.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_EQ_DESIGN,
        title: "EQ Design",
        overview: "Controls the type and number of filters the optimizer can use.",
        fields: &[
            FieldDoc {
                name: "Mode",
                description: "IIR uses parametric biquad filters (low latency, minimal \
                              phase). FIR uses a finite impulse response (linear phase, \
                              higher latency). Mixed combines both.",
                default: "iir",
                tip: "Use IIR for real-time playback. FIR for offline mastering or \
                      when phase linearity matters.",
            },
            FieldDoc {
                name: "Num Filters",
                description: "Maximum number of parametric EQ bands. More filters can \
                              match the target more closely but risk overfitting and \
                              audible ringing.",
                default: "7",
                tip: "5-9 filters is a good balance. Above 12, diminishing returns \
                      set in and the result may sound worse.",
            },
            FieldDoc {
                name: "FIR Taps",
                description: "Length of the FIR filter in samples. More taps give finer \
                              frequency resolution but increase latency and CPU cost.",
                default: "4096",
                tip: "At 48 kHz, 4096 taps = ~85 ms latency. Use 2048 for lower \
                      latency, 8192+ for surgical room correction.",
            },
            FieldDoc {
                name: "FIR Phase",
                description: "'Linear' preserves transients but adds latency. 'Minimum' \
                              concentrates energy at the start (lower latency). 'Kirkeby' \
                              is an inverse-filter approach for room correction.",
                default: "linear",
                tip: "Linear phase is safest for headphones. Minimum phase for speakers \
                      where pre-ringing matters.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_RANGES,
        title: "Parameter Ranges",
        overview: "Bounds on gain, Q factor, and frequency for each filter. Tighter \
                   bounds speed up the search but may prevent the optimizer from \
                   finding the best solution.",
        fields: &[
            FieldDoc {
                name: "dB Range (min/max)",
                description: "Maximum boost and cut per filter band in decibels.",
                default: "-12 to +6 dB",
                tip: "Limiting boost to +6 dB prevents excessive resonances. Allow \
                      more cut than boost — cutting is always safer.",
            },
            FieldDoc {
                name: "Q Range (min/max)",
                description: "Bandwidth of each filter. Low Q = wide and gentle, \
                              high Q = narrow and surgical.",
                default: "0.5 to 10",
                tip: "Keep max Q below 10 to avoid ringing. Q around 1-2 gives \
                      natural-sounding corrections.",
            },
            FieldDoc {
                name: "Frequency Range (min/max)",
                description: "The frequency bounds for filter placement. Filters \
                              will only be placed within this range.",
                default: "20 Hz to 20,000 Hz",
                tip: "For room EQ, restrict to the region where you have reliable \
                      measurements (e.g., 20-500 Hz for subwoofers).",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_PEQ_MODEL,
        title: "Filter Type",
        overview: "Determines which filter types the optimizer can use. More \
                   flexible models can achieve better results but increase search \
                   complexity.",
        fields: &[FieldDoc {
            name: "Filter Type",
            description: "'Peaks Only' uses bell/peak filters — simplest and most \
                          compatible. 'Highpass + Peaks' adds a bass rolloff. \
                          'Shelves + Peaks' adds low and high shelves — most flexible. \
                          'Fully Automatic' lets each filter choose its own type.",
            default: "Peaks Only",
            tip: "For headphone EQ, 'Peaks Only' is usually sufficient. For speakers, \
                  'Shelves + Peaks' gives better control over the overall tonal balance.",
        }],
    },
    BlockDoc {
        id: BLOCK_OPTIMIZER,
        title: "Optimizer Settings",
        overview: "Controls the search algorithm, its budget, and convergence criteria.",
        fields: &[
            FieldDoc {
                name: "Algorithm",
                description: "The optimization method. DE (Differential Evolution) is \
                              a robust global search. COBYLA is a fast local method. \
                              CMA-ES and PSO/RGA/TLBO are alternative metaheuristics.",
                default: "autoeq:de",
                tip: "'autoeq:de' is the best default — it's a tuned DE variant. \
                      Use 'autoeq:cobyla' for fast local refinement only.",
            },
            FieldDoc {
                name: "Population",
                description: "Number of candidate solutions in the population-based \
                              search. Larger populations explore more but take longer.",
                default: "100",
                tip: "50-200 for quick results, 500+ for thorough searches on \
                      difficult targets.",
            },
            FieldDoc {
                name: "Max Evaluations",
                description: "Maximum number of objective function evaluations. The \
                              optimizer stops after this many, even if not converged.",
                default: "5000",
                tip: "5000 is a good balance. Increase to 20000+ for complex multi-sub \
                      optimization.",
            },
            FieldDoc {
                name: "Tolerance",
                description: "Convergence threshold. The optimizer stops early when \
                              improvement drops below this value.",
                default: "1e-6",
                tip: "Lower tolerance = longer search but potentially better result. \
                      1e-4 is fine for quick previews.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_DE_PARAMS,
        title: "Differential Evolution",
        overview: "Fine-tuning parameters for the DE algorithm. These control how \
                   aggressively the search explores vs. exploits known good solutions.",
        fields: &[
            FieldDoc {
                name: "Strategy",
                description: "The mutation strategy. 'currenttobest1bin' balances \
                              exploration and exploitation. 'best1bin' converges faster \
                              but may miss global optima.",
                default: "currenttobest1bin",
                tip: "Leave at default unless you understand DE theory. 'lshade' is \
                      a self-adaptive variant that needs no tuning.",
            },
            FieldDoc {
                name: "Mutation F",
                description: "Differential weight — controls step size in the search \
                              space. Higher = more exploration, lower = finer tuning.",
                default: "0.5",
                tip: "Range 0.4-0.9 works well. Below 0.3 can stagnate.",
            },
            FieldDoc {
                name: "Recombination CR",
                description: "Crossover probability — how much of a trial vector \
                              comes from the mutant vs. the parent.",
                default: "0.9",
                tip: "High CR (0.8-1.0) for correlated parameters. Low CR (0.1-0.3) \
                      for independent parameters.",
            },
            FieldDoc {
                name: "Adaptive Weights",
                description: "When > 0, the mutation F and CR are self-adapted \
                              during the search. The weight controls how fast \
                              adaptation occurs.",
                default: "0.0 (disabled)",
                tip: "Set to 0.1-0.3 for automatic tuning. Useful when you don't \
                      know good F/CR values.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_REFINEMENT,
        title: "Refinement",
        overview: "After the global search, a local optimizer polishes the result. \
                   This typically improves the solution by 0.5-2 dB.",
        fields: &[
            FieldDoc {
                name: "Enable Refine",
                description: "Run a local optimizer after the global search finishes.",
                default: "enabled",
                tip: "Almost always keep this on. Only disable for speed during \
                      quick previews.",
            },
            FieldDoc {
                name: "Local Algorithm",
                description: "The local optimizer used for refinement. COBYLA is \
                              derivative-free and robust near constraints.",
                default: "cobyla",
                tip: "COBYLA is the local refinement optimizer.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_SMOOTHING,
        title: "Signal Processing",
        overview: "Controls how the target curve and measurements are processed before \
                   optimization. These settings affect the trade-off between accuracy \
                   and stability.",
        fields: &[
            FieldDoc {
                name: "Smoothing",
                description: "Applies fractional-octave smoothing to the inverted target. \
                              Reduces noise sensitivity but may hide narrow features.",
                default: "enabled",
                tip: "Disable for room EQ where narrow modes need surgical correction.",
            },
            FieldDoc {
                name: "Smooth Window",
                description: "Smoothing resolution: 1/N octave. Lower N = more smoothing.",
                default: "1 (1-octave)",
                tip: "Use 1/3 octave (N=3) for a good balance. Use 1/12 (N=12) for \
                      detailed correction.",
            },
            FieldDoc {
                name: "Psychoacoustic Weighting",
                description: "Weights the error by perceptual importance using ERB \
                              (Equivalent Rectangular Bandwidth) spacing. Prioritizes \
                              frequencies where the ear is most sensitive.",
                default: "enabled",
                tip: "Keep enabled for natural-sounding results. Disable for purely \
                      analytical measurements.",
            },
            FieldDoc {
                name: "Asymmetric Loss",
                description: "Penalizes peaks more heavily than dips. Useful for room \
                              EQ where cutting resonances is safe but boosting nulls \
                              wastes amplifier power.",
                default: "enabled (room EQ)",
                tip: "Keep enabled for room correction. Disable for headphone EQ \
                      where dips and peaks are equally problematic.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_TARGET_TILT,
        title: "Target Tilt",
        overview: "Applies a frequency-dependent slope to the target curve. Useful \
                   for matching a preferred house curve or compensating for room gain.",
        fields: &[
            FieldDoc {
                name: "Tilt Type",
                description: "'Harman' applies the Harman research-based slope. \
                              'Custom' lets you set the slope manually. 'Flat' \
                              disables tilt.",
                default: "flat",
                tip: "Start with 'flat'. Add tilt only if the flat result sounds \
                      too bright or too warm.",
            },
            FieldDoc {
                name: "Slope (dB/octave)",
                description: "The rate of tilt. Negative = warmer (more bass), \
                              positive = brighter (more treble).",
                default: "0.0",
                tip: "A slope of -0.5 dB/octave gives a gentle warm tilt.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_EXCURSION,
        title: "Excursion Protection",
        overview: "Adds a highpass filter to protect woofers from excessive \
                   low-frequency boost that could cause mechanical damage.",
        fields: &[
            FieldDoc {
                name: "Auto-detect F3",
                description: "Automatically detect the speaker's -3 dB point from \
                              the measurement and place the protection filter there.",
                default: "enabled",
                tip: "Disable and set manually if the auto-detection picks the \
                      wrong frequency.",
            },
            FieldDoc {
                name: "Filter Order",
                description: "Steepness of the protection highpass. Higher order = \
                              steeper rolloff = more protection but more phase shift.",
                default: "4 (24 dB/oct)",
                tip: "Order 2 is gentle, order 4 is typical, order 6 is aggressive.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_SCHROEDER,
        title: "Schroeder Split",
        overview: "Splits the optimization into low-frequency and high-frequency \
                   regions at the Schroeder frequency (transition between modal and \
                   statistical room behavior). Each region gets independent Q constraints.",
        fields: &[
            FieldDoc {
                name: "Schroeder Frequency",
                description: "The crossover between modal (below) and diffuse (above) \
                              room behavior.",
                default: "200 Hz (room-dependent)",
                tip: "Calculate from room dimensions: F = 2000 * sqrt(RT60 / V) \
                      where V is volume in m^3.",
            },
            FieldDoc {
                name: "Low-freq max Q",
                description: "Maximum Q for filters below the Schroeder frequency. \
                              Room modes are narrow, so higher Q is useful here.",
                default: "10",
                tip: "Allow Q up to 15-20 for stubborn room modes.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_PHASE_ALIGNMENT,
        title: "Phase Alignment",
        overview: "Optimizes the relative timing between drivers (woofer, midrange, \
                   tweeter) for coherent summation at the crossover frequencies.",
        fields: &[
            FieldDoc {
                name: "Frequency Range",
                description: "The band over which phase alignment is optimized.",
                default: "200-5000 Hz",
                tip: "Focus on the crossover regions where drivers overlap.",
            },
            FieldDoc {
                name: "Max Delay",
                description: "Maximum time alignment correction in milliseconds.",
                default: "5 ms",
                tip: "Keep below 10 ms to avoid audible comb filtering with \
                      the direct sound.",
            },
        ],
    },
    BlockDoc {
        id: BLOCK_MULTI_SEAT,
        title: "Multi-Seat Optimization",
        overview: "Optimizes the EQ for multiple listening positions simultaneously, \
                   finding a compromise that works reasonably at all seats.",
        fields: &[
            FieldDoc {
                name: "Strategy",
                description: "'Primary with constraints' optimizes for the main seat \
                              while limiting degradation at others. 'Average' treats \
                              all seats equally. 'Minimize variance' reduces the spread.",
                default: "primary with constraints",
                tip: "Use 'primary' when there's a clear sweet spot. Use 'minimize \
                      variance' for theater-style seating.",
            },
            FieldDoc {
                name: "Max Deviation",
                description: "Maximum allowed degradation at secondary seats (dB).",
                default: "3 dB",
                tip: "Tighter constraint (1-2 dB) gives more uniform results but \
                      may limit the primary seat improvement.",
            },
        ],
    },
];

/// Look up the documentation for a block by its id.
pub fn block_doc(id: &str) -> &'static BlockDoc {
    BLOCK_DOCS
        .iter()
        .find(|b| b.id == id)
        .unwrap_or(&BLOCK_DOCS[0]) // fallback to overview
}
