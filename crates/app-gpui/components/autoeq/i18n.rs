//! Internationalization (i18n) for AutoEQ form documentation.
//!
//! Supports English, German, French, and Spanish translations.

use gpui::*;
use gpui_ui_kit::i18n::Language;
use std::collections::HashMap;

/// Translation keys for AutoEQ documentation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocKey {
    // Block titles
    DocOverviewTitle,
    DocGoalsTitle,
    DocEqDesignTitle,
    DocRangesTitle,
    DocPeqModelTitle,
    DocOptimizerTitle,
    DocDeParamsTitle,
    DocRefinementTitle,
    DocTargetTiltTitle,
    DocExcursionTitle,
    DocSchroederTitle,
    DocPhaseAlignmentTitle,
    DocMultiSeatTitle,

    // Block overviews
    DocOverviewOverview,
    DocGoalsOverview,
    DocEqDesignOverview,
    DocRangesOverview,
    DocPeqModelOverview,
    DocOptimizerOverview,
    DocDeParamsOverview,
    DocRefinementOverview,
    DocTargetTiltOverview,
    DocExcursionOverview,
    DocSchroederOverview,
    DocPhaseAlignmentOverview,
    DocMultiSeatOverview,

    // Field names
    FieldSystemType,
    FieldLossFunction,
    FieldTargetCurve,
    FieldMode,
    FieldNumFilters,
    FieldFirTaps,
    FieldFirPhase,
    FieldDbRange,
    FieldQRange,
    FieldFrequencyRange,
    FieldPeqModel,
    FieldAlgorithm,
    FieldPopulation,
    FieldMaxEvaluations,
    FieldTolerance,
    FieldStrategy,
    FieldMutationF,
    FieldRecombinationCr,
    FieldAdaptiveWeights,
    FieldEnableRefine,
    FieldLocalAlgorithm,
    FieldTiltType,
    FieldSlopeDbOct,
    FieldAutoDetectF3,
    FieldFilterOrder,
    FieldSchroederFrequency,
    FieldLowFreqMaxQ,
    FieldFrequencyRange2,
    FieldMaxDelay,
    FieldStrategy2,
    FieldMaxDeviation,

    // Field descriptions
    DescSystemType,
    DescLossFunction,
    DescTargetCurve,
    DescMode,
    DescNumFilters,
    DescFirTaps,
    DescFirPhase,
    DescDbRange,
    DescQRange,
    DescFrequencyRange,
    DescPeqModel,
    DescAlgorithm,
    DescPopulation,
    DescMaxEvaluations,
    DescTolerance,
    DescStrategy,
    DescMutationF,
    DescRecombinationCr,
    DescAdaptiveWeights,
    DescEnableRefine,
    DescLocalAlgorithm,
    DescTiltType,
    DescSlopeDbOct,
    DescAutoDetectF3,
    DescFilterOrder,
    DescSchroederFrequency,
    DescLowFreqMaxQ,
    DescFrequencyRange2,
    DescMaxDelay,
    DescStrategy2,
    DescMaxDeviation,

    // Field defaults
    DefaultSystemType,
    DefaultLossFunction,
    DefaultTargetCurve,
    DefaultMode,
    DefaultNumFilters,
    DefaultFirTaps,
    DefaultFirPhase,
    DefaultDbRange,
    DefaultQRange,
    DefaultFrequencyRange,
    DefaultPeqModel,
    DefaultAlgorithm,
    DefaultPopulation,
    DefaultMaxEvaluations,
    DefaultTolerance,
    DefaultStrategy,
    DefaultMutationF,
    DefaultRecombinationCr,
    DefaultAdaptiveWeights,
    DefaultEnableRefine,
    DefaultLocalAlgorithm,
    DefaultTiltType,
    DefaultSlopeDbOct,
    DefaultAutoDetectF3,
    DefaultFilterOrder,
    DefaultSchroederFrequency,
    DefaultLowFreqMaxQ,
    DefaultFrequencyRange2,
    DefaultMaxDelay,
    DefaultStrategy2,
    DefaultMaxDeviation,

    // Field tips
    TipSystemType,
    TipLossFunction,
    TipTargetCurve,
    TipMode,
    TipNumFilters,
    TipFirTaps,
    TipFirPhase,
    TipDbRange,
    TipQRange,
    TipFrequencyRange,
    TipPeqModel,
    TipAlgorithm,
    TipPopulation,
    TipMaxEvaluations,
    TipTolerance,
    TipStrategy,
    TipMutationF,
    TipRecombinationCr,
    TipAdaptiveWeights,
    TipEnableRefine,
    TipLocalAlgorithm,
    TipTiltType,
    TipSlopeDbOct,
    TipAutoDetectF3,
    TipFilterOrder,
    TipSchroederFrequency,
    TipLowFreqMaxQ,
    TipFrequencyRange2,
    TipMaxDelay,
    TipStrategy2,
    TipMaxDeviation,

    // Labels
    LabelDefault,
    LabelTip,
}

/// Translations map type
type TranslationMap = HashMap<(Language, DocKey), &'static str>;

/// Translation storage
pub struct AutoEqTranslations {
    translations: TranslationMap,
}

impl AutoEqTranslations {
    /// Create new empty translations
    pub fn new() -> Self {
        let mut t = HashMap::new();
        Self::add_english(&mut t);
        Self::add_french(&mut t);
        Self::add_german(&mut t);
        Self::add_spanish(&mut t);
        Self { translations: t }
    }

    /// Get translation for a language and key
    pub fn get(&self, lang: Language, key: DocKey) -> &'static str {
        self.translations
            .get(&(lang, key))
            .copied()
            .or_else(|| self.translations.get(&(Language::English, key)).copied())
            .unwrap_or("???")
    }

    fn add_english(t: &mut TranslationMap) {
        use DocKey::*;

        // Block titles
        t.insert((Language::English, DocOverviewTitle), "AutoEQ Parameters");
        t.insert((Language::English, DocGoalsTitle), "Goals & Configuration");
        t.insert((Language::English, DocEqDesignTitle), "EQ Design");
        t.insert((Language::English, DocRangesTitle), "Parameter Ranges");
        t.insert((Language::English, DocPeqModelTitle), "PEQ Model");
        t.insert((Language::English, DocOptimizerTitle), "Optimizer Settings");
        t.insert(
            (Language::English, DocDeParamsTitle),
            "Differential Evolution",
        );
        t.insert((Language::English, DocRefinementTitle), "Refinement");
        t.insert((Language::English, DocTargetTiltTitle), "Target Tilt");
        t.insert(
            (Language::English, DocExcursionTitle),
            "Excursion Protection",
        );
        t.insert((Language::English, DocSchroederTitle), "Schroeder Split");
        t.insert(
            (Language::English, DocPhaseAlignmentTitle),
            "Phase Alignment",
        );
        t.insert(
            (Language::English, DocMultiSeatTitle),
            "Multi-Seat Optimization",
        );

        // Block overviews
        t.insert((Language::English, DocOverviewOverview), "Configure the parametric EQ optimizer. The form is split into three areas: goals (what to optimize for), EQ design (filter constraints), and optimizer settings (how the search is performed). Hover over a section on the left to see detailed help here.");
        t.insert((Language::English, DocGoalsOverview), "Defines what the optimizer tries to achieve and the target response it matches against.");
        t.insert(
            (Language::English, DocEqDesignOverview),
            "Controls the type and number of filters the optimizer can use.",
        );
        t.insert((Language::English, DocRangesOverview), "Bounds on gain, Q factor, and frequency for each filter. Tighter bounds speed up the search but may prevent the optimizer from finding the best solution.");
        t.insert((Language::English, DocPeqModelOverview), "Determines which filter types the optimizer can use. More flexible models can achieve better results but increase search complexity.");
        t.insert(
            (Language::English, DocOptimizerOverview),
            "Controls the search algorithm, its budget, and convergence criteria.",
        );
        t.insert((Language::English, DocDeParamsOverview), "Fine-tuning parameters for the DE algorithm. These control how aggressively the search explores vs. exploits known good solutions.");
        t.insert((Language::English, DocRefinementOverview), "After the global search, a local optimizer polishes the result. This typically improves the solution by 0.5-2 dB.");
        t.insert((Language::English, DocTargetTiltOverview), "Applies a frequency-dependent slope to the target curve. Useful for matching a preferred house curve or compensating for room gain.");
        t.insert((Language::English, DocExcursionOverview), "Adds a highpass filter to protect woofers from excessive low-frequency boost that could cause mechanical damage.");
        t.insert((Language::English, DocSchroederOverview), "Splits the optimization into low-frequency and high-frequency regions at the Schroeder frequency (transition between modal and statistical room behavior). Each region gets independent Q constraints.");
        t.insert((Language::English, DocPhaseAlignmentOverview), "Optimizes the relative timing between drivers (woofer, midrange, tweeter) for coherent summation at the crossover frequencies.");
        t.insert((Language::English, DocMultiSeatOverview), "Optimizes the EQ for multiple listening positions simultaneously, finding a compromise that works reasonably at all seats.");

        // Field names
        t.insert((Language::English, FieldSystemType), "System Type");
        t.insert((Language::English, FieldLossFunction), "Loss Function");
        t.insert((Language::English, FieldTargetCurve), "Target Curve");
        t.insert((Language::English, FieldMode), "Mode");
        t.insert((Language::English, FieldNumFilters), "Num Filters");
        t.insert((Language::English, FieldFirTaps), "FIR Taps");
        t.insert((Language::English, FieldFirPhase), "FIR Phase");
        t.insert((Language::English, FieldDbRange), "dB Range (min/max)");
        t.insert((Language::English, FieldQRange), "Q Range (min/max)");
        t.insert(
            (Language::English, FieldFrequencyRange),
            "Frequency Range (min/max)",
        );
        t.insert((Language::English, FieldPeqModel), "PEQ Model");
        t.insert((Language::English, FieldAlgorithm), "Algorithm");
        t.insert((Language::English, FieldPopulation), "Population");
        t.insert((Language::English, FieldMaxEvaluations), "Max Evaluations");
        t.insert((Language::English, FieldTolerance), "Tolerance");
        t.insert((Language::English, FieldStrategy), "Strategy");
        t.insert((Language::English, FieldMutationF), "Mutation F");
        t.insert(
            (Language::English, FieldRecombinationCr),
            "Recombination CR",
        );
        t.insert(
            (Language::English, FieldAdaptiveWeights),
            "Adaptive Weights",
        );
        t.insert((Language::English, FieldEnableRefine), "Enable Refine");
        t.insert((Language::English, FieldLocalAlgorithm), "Local Algorithm");
        t.insert((Language::English, FieldTiltType), "Tilt Type");
        t.insert((Language::English, FieldSlopeDbOct), "Slope (dB/octave)");
        t.insert((Language::English, FieldAutoDetectF3), "Auto-detect F3");
        t.insert((Language::English, FieldFilterOrder), "Filter Order");
        t.insert(
            (Language::English, FieldSchroederFrequency),
            "Schroeder Frequency",
        );
        t.insert((Language::English, FieldLowFreqMaxQ), "Low-freq max Q");
        t.insert((Language::English, FieldFrequencyRange2), "Frequency Range");
        t.insert((Language::English, FieldMaxDelay), "Max Delay");
        t.insert((Language::English, FieldStrategy2), "Strategy");
        t.insert((Language::English, FieldMaxDeviation), "Max Deviation");

        // Field descriptions
        t.insert((Language::English, DescSystemType), "The loudspeaker topology. Affects how measurements are combined and which loss functions are available.");
        t.insert((Language::English, DescLossFunction), "'Flat' minimizes the RMS deviation from the target curve. 'Score' optimizes the Harman/Olive listener preference score, which allows a controlled bass shelf.");
        t.insert((Language::English, DescTargetCurve), "The reference frequency response the optimizer tries to match. Harman curves are research-based preference targets. 'Flat' targets a ruler-flat response.");
        t.insert((Language::English, DescMode), "IIR uses parametric biquad filters (low latency, minimal phase). FIR uses a finite impulse response (linear phase, higher latency). Mixed combines both.");
        t.insert((Language::English, DescNumFilters), "Maximum number of parametric EQ bands. More filters can match the target more closely but risk overfitting and audible ringing.");
        t.insert((Language::English, DescFirTaps), "Length of the FIR filter in samples. More taps give finer frequency resolution but increase latency and CPU cost.");
        t.insert((Language::English, DescFirPhase), "'Linear' preserves transients but adds latency. 'Minimum' concentrates energy at the start (lower latency). 'Kirkeby' is an inverse-filter approach for room correction.");
        t.insert(
            (Language::English, DescDbRange),
            "Maximum boost and cut per filter band in decibels.",
        );
        t.insert(
            (Language::English, DescQRange),
            "Bandwidth of each filter. Low Q = wide and gentle, high Q = narrow and surgical.",
        );
        t.insert((Language::English, DescFrequencyRange), "The frequency bounds for filter placement. Filters will only be placed within this range.");
        t.insert((Language::English, DescPeqModel), "'pk' = peak filters only. 'hp-pk' adds a highpass. 'hp-pk-lp' adds both highpass and lowpass. 'ls-pk-hs' adds shelf filters. 'free' allows any combination.");
        t.insert((Language::English, DescAlgorithm), "The optimization method. DE (Differential Evolution) is a robust global search. COBYLA/BOBYQA are fast local methods. PSO/RGA/TLBO are alternative metaheuristics.");
        t.insert((Language::English, DescPopulation), "Number of candidate solutions in the population-based search. Larger populations explore more but take longer.");
        t.insert((Language::English, DescMaxEvaluations), "Maximum number of objective function evaluations. The optimizer stops after this many, even if not converged.");
        t.insert((Language::English, DescTolerance), "Convergence threshold. The optimizer stops early when improvement drops below this value.");
        t.insert((Language::English, DescStrategy), "The mutation strategy. 'currenttobest1bin' balances exploration and exploitation. 'best1bin' converges faster but may miss global optima.");
        t.insert((Language::English, DescMutationF), "Differential weight — controls step size in the search space. Higher = more exploration, lower = finer tuning.");
        t.insert((Language::English, DescRecombinationCr), "Crossover probability — how much of a trial vector comes from the mutant vs. the parent.");
        t.insert((Language::English, DescAdaptiveWeights), "When > 0, the mutation F and CR are self-adapted during the search. The weight controls how fast adaptation occurs.");
        t.insert(
            (Language::English, DescEnableRefine),
            "Run a local optimizer after the global search finishes.",
        );
        t.insert((Language::English, DescLocalAlgorithm), "The local optimizer used for refinement. COBYLA is derivative-free and robust. BOBYQA is faster but less stable near constraints.");
        t.insert((Language::English, DescTiltType), "'Harman' applies the Harman research-based slope. 'Custom' lets you set the slope manually. 'Flat' disables tilt.");
        t.insert(
            (Language::English, DescSlopeDbOct),
            "The rate of tilt. Negative = warmer (more bass), positive = brighter (more treble).",
        );
        t.insert((Language::English, DescAutoDetectF3), "Automatically detect the speaker's -3 dB point from the measurement and place the protection filter there.");
        t.insert((Language::English, DescFilterOrder), "Steepness of the protection highpass. Higher order = steeper rolloff = more protection but more phase shift.");
        t.insert(
            (Language::English, DescSchroederFrequency),
            "The crossover between modal (below) and diffuse (above) room behavior.",
        );
        t.insert((Language::English, DescLowFreqMaxQ), "Maximum Q for filters below the Schroeder frequency. Room modes are narrow, so higher Q is useful here.");
        t.insert(
            (Language::English, DescFrequencyRange2),
            "The band over which phase alignment is optimized.",
        );
        t.insert(
            (Language::English, DescMaxDelay),
            "Maximum time alignment correction in milliseconds.",
        );
        t.insert((Language::English, DescStrategy2), "'Primary with constraints' optimizes for the main seat while limiting degradation at others. 'Average' treats all seats equally. 'Minimize variance' reduces the spread.");
        t.insert(
            (Language::English, DescMaxDeviation),
            "Maximum allowed degradation at secondary seats (dB).",
        );

        // Field defaults
        t.insert((Language::English, DefaultSystemType), "stereo");
        t.insert((Language::English, DefaultLossFunction), "flat");
        t.insert(
            (Language::English, DefaultTargetCurve),
            "Harman 2018 (headphones) / flat (speakers)",
        );
        t.insert((Language::English, DefaultMode), "iir");
        t.insert((Language::English, DefaultNumFilters), "7");
        t.insert((Language::English, DefaultFirTaps), "4096");
        t.insert((Language::English, DefaultFirPhase), "linear");
        t.insert((Language::English, DefaultDbRange), "-12 to +6 dB");
        t.insert((Language::English, DefaultQRange), "0.5 to 10");
        t.insert(
            (Language::English, DefaultFrequencyRange),
            "20 Hz to 20,000 Hz",
        );
        t.insert((Language::English, DefaultPeqModel), "pk");
        t.insert((Language::English, DefaultAlgorithm), "autoeq:de");
        t.insert((Language::English, DefaultPopulation), "100");
        t.insert((Language::English, DefaultMaxEvaluations), "5000");
        t.insert((Language::English, DefaultTolerance), "1e-6");
        t.insert((Language::English, DefaultStrategy), "currenttobest1bin");
        t.insert((Language::English, DefaultMutationF), "0.5");
        t.insert((Language::English, DefaultRecombinationCr), "0.9");
        t.insert(
            (Language::English, DefaultAdaptiveWeights),
            "0.0 (disabled)",
        );
        t.insert((Language::English, DefaultEnableRefine), "enabled");
        t.insert((Language::English, DefaultLocalAlgorithm), "cobyla");
        t.insert((Language::English, DefaultTiltType), "flat");
        t.insert((Language::English, DefaultSlopeDbOct), "0.0");
        t.insert((Language::English, DefaultAutoDetectF3), "enabled");
        t.insert((Language::English, DefaultFilterOrder), "4 (24 dB/oct)");
        t.insert(
            (Language::English, DefaultSchroederFrequency),
            "200 Hz (room-dependent)",
        );
        t.insert((Language::English, DefaultLowFreqMaxQ), "10");
        t.insert((Language::English, DefaultFrequencyRange2), "200-5000 Hz");
        t.insert((Language::English, DefaultMaxDelay), "5 ms");
        t.insert(
            (Language::English, DefaultStrategy2),
            "primary with constraints",
        );
        t.insert((Language::English, DefaultMaxDeviation), "3 dB");

        // Field tips
        t.insert((Language::English, TipSystemType), "Use 'multisub' when optimizing a system with independently placed subwoofers. 'DBA' is for double bass arrays.");
        t.insert((Language::English, TipLossFunction), "Start with 'flat' for accuracy. Switch to 'score' if you want perceptually-tuned bass emphasis.");
        t.insert((Language::English, TipTargetCurve), "For headphones, Harman 2018 is the most validated target. For speakers, 'flat' is typical unless you have a preferred house curve.");
        t.insert((Language::English, TipMode), "Use IIR for real-time playback. FIR for offline mastering or when phase linearity matters.");
        t.insert((Language::English, TipNumFilters), "5-9 filters is a good balance. Above 12, diminishing returns set in and the result may sound worse.");
        t.insert((Language::English, TipFirTaps), "At 48 kHz, 4096 taps = ~85 ms latency. Use 2048 for lower latency, 8192+ for surgical room correction.");
        t.insert((Language::English, TipFirPhase), "Linear phase is safest for headphones. Minimum phase for speakers where pre-ringing matters.");
        t.insert((Language::English, TipDbRange), "Limiting boost to +6 dB prevents excessive resonances. Allow more cut than boost — cutting is always safer.");
        t.insert((Language::English, TipQRange), "Keep max Q below 10 to avoid ringing. Q around 1-2 gives natural-sounding corrections.");
        t.insert((Language::English, TipFrequencyRange), "For room EQ, restrict to the region where you have reliable measurements (e.g., 20-500 Hz for subwoofers).");
        t.insert((Language::English, TipPeqModel), "For headphone EQ, 'pk' is usually sufficient. For speakers, 'hp-pk-lp' or 'ls-pk-hs' gives better low/high frequency control.");
        t.insert((Language::English, TipAlgorithm), "'autoeq:de' is the best default — it's a tuned DE variant. Use 'nlopt:cobyla' for fast local refinement only.");
        t.insert(
            (Language::English, TipPopulation),
            "50-200 for quick results, 500+ for thorough searches on difficult targets.",
        );
        t.insert(
            (Language::English, TipMaxEvaluations),
            "5000 is a good balance. Increase to 20000+ for complex multi-sub optimization.",
        );
        t.insert((Language::English, TipTolerance), "Lower tolerance = longer search but potentially better result. 1e-4 is fine for quick previews.");
        t.insert((Language::English, TipStrategy), "Leave at default unless you understand DE theory. 'lshade' is a self-adaptive variant that needs no tuning.");
        t.insert(
            (Language::English, TipMutationF),
            "Range 0.4-0.9 works well. Below 0.3 can stagnate.",
        );
        t.insert((Language::English, TipRecombinationCr), "High CR (0.8-1.0) for correlated parameters. Low CR (0.1-0.3) for independent parameters.");
        t.insert(
            (Language::English, TipAdaptiveWeights),
            "Set to 0.1-0.3 for automatic tuning. Useful when you don't know good F/CR values.",
        );
        t.insert(
            (Language::English, TipEnableRefine),
            "Almost always keep this on. Only disable for speed during quick previews.",
        );
        t.insert(
            (Language::English, TipLocalAlgorithm),
            "COBYLA is the safest choice. Try BOBYQA if COBYLA is slow.",
        );
        t.insert(
            (Language::English, TipTiltType),
            "Start with 'flat'. Add tilt only if the flat result sounds too bright or too warm.",
        );
        t.insert(
            (Language::English, TipSlopeDbOct),
            "A slope of -0.5 dB/octave gives a gentle warm tilt.",
        );
        t.insert(
            (Language::English, TipAutoDetectF3),
            "Disable and set manually if the auto-detection picks the wrong frequency.",
        );
        t.insert(
            (Language::English, TipFilterOrder),
            "Order 2 is gentle, order 4 is typical, order 6 is aggressive.",
        );
        t.insert(
            (Language::English, TipSchroederFrequency),
            "Calculate from room dimensions: F = 2000 * sqrt(RT60 / V) where V is volume in m^3.",
        );
        t.insert(
            (Language::English, TipLowFreqMaxQ),
            "Allow Q up to 15-20 for stubborn room modes.",
        );
        t.insert(
            (Language::English, TipFrequencyRange2),
            "Focus on the crossover regions where drivers overlap.",
        );
        t.insert(
            (Language::English, TipMaxDelay),
            "Keep below 10 ms to avoid audible comb filtering with the direct sound.",
        );
        t.insert((Language::English, TipStrategy2), "Use 'primary' when there's a clear sweet spot. Use 'minimize variance' for theater-style seating.");
        t.insert((Language::English, TipMaxDeviation), "Tighter constraint (1-2 dB) gives more uniform results but may limit the primary seat improvement.");

        // Labels
        t.insert((Language::English, LabelDefault), "Default:");
        t.insert((Language::English, LabelTip), "Tip:");
    }

    fn add_french(t: &mut TranslationMap) {
        use DocKey::*;

        // Block titles
        t.insert((Language::French, DocOverviewTitle), "Paramètres AutoEQ");
        t.insert(
            (Language::French, DocGoalsTitle),
            "Objectifs et Configuration",
        );
        t.insert((Language::French, DocEqDesignTitle), "Conception EQ");
        t.insert((Language::French, DocRangesTitle), "Plages de Paramètres");
        t.insert((Language::French, DocPeqModelTitle), "Modèle PEQ");
        t.insert(
            (Language::French, DocOptimizerTitle),
            "Paramètres de l'Optimiseur",
        );
        t.insert(
            (Language::French, DocDeParamsTitle),
            "Évolution Différentielle",
        );
        t.insert((Language::French, DocRefinementTitle), "Rafinement");
        t.insert(
            (Language::French, DocTargetTiltTitle),
            "Inclinaison de la Cible",
        );
        t.insert(
            (Language::French, DocExcursionTitle),
            "Protection d'Excursion",
        );
        t.insert(
            (Language::French, DocSchroederTitle),
            "Division de Schroeder",
        );
        t.insert(
            (Language::French, DocPhaseAlignmentTitle),
            "Alignement de Phase",
        );
        t.insert(
            (Language::French, DocMultiSeatTitle),
            "Optimisation Multi-Sièges",
        );

        // Block overviews
        t.insert((Language::French, DocOverviewOverview), "Configurez l'optimiseur EQ paramétrique. Le formulaire est divisé en trois zones : objectifs (ce qu'il faut optimiser), conception EQ (contraintes des filtres) et paramètres de l'optimiseur (comment la recherche est effectuée). Survolez une section sur la gauche pour voir l'aide détaillée ici.");
        t.insert((Language::French, DocGoalsOverview), "Définit ce que l'optimiseur essaie d'atteindre et la réponse cible qu'il doit égaliser.");
        t.insert(
            (Language::French, DocEqDesignOverview),
            "Contrôle le type et le nombre de filtres que l'optimiseur peut utiliser.",
        );
        t.insert((Language::French, DocRangesOverview), "Limites de gain, de facteur Q et de fréquence pour chaque filtre. Des limites plus serrées accélèrent la recherche mais peuvent empêcher l'optimiseur de trouver la meilleure solution.");
        t.insert((Language::French, DocPeqModelOverview), "Détermine quels types de filtres l'optimiseur peut utiliser. Des modèles plus flexibles peuvent obtenir de meilleurs résultats mais augmentent la complexité de la recherche.");
        t.insert(
            (Language::French, DocOptimizerOverview),
            "Contrôle l'algorithme de recherche, son budget et les critères de convergence.",
        );
        t.insert((Language::French, DocDeParamsOverview), "Paramètres de précision pour l'algorithme DE. Ils contrôlent l'agressivité de la recherche entre exploration et exploitation des bonnes solutions.");
        t.insert((Language::French, DocRefinementOverview), "Après la recherche globale, un optimiseur local polit le résultat. Cela améliore généralement la solution de 0,5 à 2 dB.");
        t.insert((Language::French, DocTargetTiltOverview), "Applique une pente dépendante de la fréquence à la courbe cible. Utile pour correspondre à une courbe de maison préférée ou compenser le gain de la pièce.");
        t.insert((Language::French, DocExcursionOverview), "Ajoute un filtre passe-haut pour protéger les woofers d'un boostLF excessif qui pourrait causer des dommages mécaniques.");
        t.insert((Language::French, DocSchroederOverview), "Divise l'optimisation en régions BF et HF à la fréquence de Schroeder (transition entre comportement modal et statistique de la pièce). Chaque région reçoit des contraintes Q indépendantes.");
        t.insert((Language::French, DocPhaseAlignmentOverview), "Optimise le minutage relatif entre les haut-parleurs (woofer, médium, tweeter) pour une somme cohérente aux fréquences de crossover.");
        t.insert((Language::French, DocMultiSeatOverview), "Optimise l'EQ pour plusieurs positions d'écoute simultanément, trouvant un compromis qui fonctionne raisonnablement à tous les sièges.");

        // Field names
        t.insert((Language::French, FieldSystemType), "Type de Système");
        t.insert((Language::French, FieldLossFunction), "Fonction de Perte");
        t.insert((Language::French, FieldTargetCurve), "Courbe Cible");
        t.insert((Language::French, FieldMode), "Mode");
        t.insert((Language::French, FieldNumFilters), "Nb de Filtres");
        t.insert((Language::French, FieldFirTaps), "Taps FIR");
        t.insert((Language::French, FieldFirPhase), "Phase FIR");
        t.insert((Language::French, FieldDbRange), "Plage dB (min/max)");
        t.insert((Language::French, FieldQRange), "Plage Q (min/max)");
        t.insert(
            (Language::French, FieldFrequencyRange),
            "Plage de Fréquence (min/max)",
        );
        t.insert((Language::French, FieldPeqModel), "Modèle PEQ");
        t.insert((Language::French, FieldAlgorithm), "Algorithme");
        t.insert((Language::French, FieldPopulation), "Population");
        t.insert((Language::French, FieldMaxEvaluations), "Évaluations Max");
        t.insert((Language::French, FieldTolerance), "Tolérance");
        t.insert((Language::French, FieldStrategy), "Stratégie");
        t.insert((Language::French, FieldMutationF), "Mutation F");
        t.insert((Language::French, FieldRecombinationCr), "Recombinaison CR");
        t.insert((Language::French, FieldAdaptiveWeights), "Poids Adaptatifs");
        t.insert(
            (Language::French, FieldEnableRefine),
            "Activer le Raffinement",
        );
        t.insert((Language::French, FieldLocalAlgorithm), "Algorithme Local");
        t.insert((Language::French, FieldTiltType), "Type d'Inclinaison");
        t.insert((Language::French, FieldSlopeDbOct), "Pente (dB/octave)");
        t.insert((Language::French, FieldAutoDetectF3), "Détection Auto F3");
        t.insert((Language::French, FieldFilterOrder), "Ordre du Filtre");
        t.insert(
            (Language::French, FieldSchroederFrequency),
            "Fréquence de Schroeder",
        );
        t.insert((Language::French, FieldLowFreqMaxQ), "Q Max BF");
        t.insert(
            (Language::French, FieldFrequencyRange2),
            "Plage de Fréquence",
        );
        t.insert((Language::French, FieldMaxDelay), "Délai Max");
        t.insert((Language::French, FieldStrategy2), "Stratégie");
        t.insert((Language::French, FieldMaxDeviation), "Déviation Max");

        // Field descriptions
        t.insert((Language::French, DescSystemType), "La topologie de l'enceinte. Affecte la façon dont les mesures sont combinées et les fonctions de perte disponibles.");
        t.insert((Language::French, DescLossFunction), "'Flat' minimise l'écart RMS par rapport à la courbe cible. 'Score' optimise le score de préférence Harman/Olive, qui permet un grave shelf contrôlé.");
        t.insert((Language::French, DescTargetCurve), "La réponse en fréquence de référence que l'optimiseur essaie d'égaliser. Les courbes Harman sont des cibles de préférence basées sur la recherche. 'Flat' vise une réponse parfaitement plate.");
        t.insert((Language::French, DescMode), "IIR utilise des filtres biquad paramétriques (faible latence, phase minimale). FIR utilise une réponse impulsionnelle finie (phase linéaire, latence plus élevée). Mixed combine les deux.");
        t.insert((Language::French, DescNumFilters), "Nombre maximum de bandes EQ paramétriques. Plus de filtres peuvent égaliser la cible plus précisément mais risquent le surapprentissage et le ronronnement audible.");
        t.insert((Language::French, DescFirTaps), "Longueur du filtre FIR en échantillons. Plus de taps donnent une résolution fréquentielle plus fine mais augmentent la latence et le coût CPU.");
        t.insert((Language::French, DescFirPhase), "'Linear' préserve les transitoires mais ajoute de la latence. 'Minimum' concentre l'énergie au début (latence plus faible). 'Kirkeby' est une approche par filtrage inverse pour la correction de pièce.");
        t.insert(
            (Language::French, DescDbRange),
            "Boost et coupe maximum par bande de filtre en décibels.",
        );
        t.insert((Language::French, DescQRange), "Bande passante de chaque filtre. Q faible = large et doux, Q élevé = étroit et chirurgical.");
        t.insert((Language::French, DescFrequencyRange), "Les limites de fréquence pour le placement des filtres. Les filtres ne seront placés que dans cette plage.");
        t.insert((Language::French, DescPeqModel), "'pk' = filtres peak uniquement. 'hp-pk' ajoute un passe-haut. 'hp-pk-lp' ajoute un passe-haut et un passe-bas. 'ls-pk-hs' ajoute des filtres shelf. 'free' permet toute combinaison.");
        t.insert((Language::French, DescAlgorithm), "La méthode d'optimisation. DE (Differential Evolution) est une recherche globale robuste. COBYLA/BOBYQA sont des méthodes locales rapides. PSO/RGA/TLBO sont des métaheuristiques alternatives.");
        t.insert((Language::French, DescPopulation), "Nombre de solutions candidates dans la recherche basée sur la population. Des populations plus grandes explorent plus mais prennent plus de temps.");
        t.insert((Language::French, DescMaxEvaluations), "Nombre maximum d'évaluations de la fonction objectif. L'optimiseur s'arrête après ce nombre, même s'il n'a pas convergé.");
        t.insert((Language::French, DescTolerance), "Seuil de convergence. L'optimiseur s'arrête tôt lorsque l'amélioration descend en dessous de cette valeur.");
        t.insert((Language::French, DescStrategy), "La stratégie de mutation. 'currenttobest1bin' équilibre exploration et exploitation. 'best1bin' converge plus vite mais peut manquer les optima globaux.");
        t.insert((Language::French, DescMutationF), "Poids différentiel — contrôle la taille du pas dans l'espace de recherche. Plus élevé = plus d'exploration, plus faible = ajustement plus fin.");
        t.insert((Language::French, DescRecombinationCr), "Probabilité de crossover — combien du vecteur d'essai provient du mutant vs. du parent.");
        t.insert((Language::French, DescAdaptiveWeights), "Quand > 0, la mutation F et CR s'auto-adaptent pendant la recherche. Le poids contrôle la vitesse d'adaptation.");
        t.insert(
            (Language::French, DescEnableRefine),
            "Exécuter un optimiseur local après la fin de la recherche globale.",
        );
        t.insert((Language::French, DescLocalAlgorithm), "L'optimiseur local utilisé pour le raffinement. COBYLA est sans dérivé et robuste. BOBYQA est plus rapide mais moins stable près des contraintes.");
        t.insert((Language::French, DescTiltType), "'Harman' applique la pente recommandée par Harman. 'Custom' vous permet de régler la pente manuellement. 'Flat' désactive l'inclinaison.");
        t.insert((Language::French, DescSlopeDbOct), "Le taux d'inclinaison. Négatif = plus chaleureux (plus de grave), positif = plus brillant (plus d'aigu).");
        t.insert((Language::French, DescAutoDetectF3), "Détecte automatiquement le point -3 dB du haut-parleur à partir de la mesure et place le filtre de protection à cet endroit.");
        t.insert((Language::French, DescFilterOrder), "Pente du passe-haut de protection. Ordre supérieur = pente plus raide = plus de protection mais plus de déphasage.");
        t.insert((Language::French, DescSchroederFrequency), "Le crossover entre le comportement modal (en dessous) et diffus (au-dessus) de la pièce.");
        t.insert((Language::French, DescLowFreqMaxQ), "Q maximum pour les filtres en dessous de la fréquence de Schroeder. Les modes de pièce sont étroits, donc un Q plus élevé est utile ici.");
        t.insert(
            (Language::French, DescFrequencyRange2),
            "La bande sur laquelle l'alignement de phase est optimisé.",
        );
        t.insert(
            (Language::French, DescMaxDelay),
            "Correction d'alignement de temps maximum en millisecondes.",
        );
        t.insert((Language::French, DescStrategy2), "'Primary with constraints' optimise pour le siège principal tout en limitant la dégradation aux autres. 'Average' traite tous les sièges également. 'Minimize variance' réduit l'étalement.");
        t.insert(
            (Language::French, DescMaxDeviation),
            "Dégradation maximale autorisée aux sièges secondaires (dB).",
        );

        // Field defaults
        t.insert((Language::French, DefaultSystemType), "stereo");
        t.insert((Language::French, DefaultLossFunction), "flat");
        t.insert(
            (Language::French, DefaultTargetCurve),
            "Harman 2018 (casques) / flat (enceintes)",
        );
        t.insert((Language::French, DefaultMode), "iir");
        t.insert((Language::French, DefaultNumFilters), "7");
        t.insert((Language::French, DefaultFirTaps), "4096");
        t.insert((Language::French, DefaultFirPhase), "linear");
        t.insert((Language::French, DefaultDbRange), "-12 à +6 dB");
        t.insert((Language::French, DefaultQRange), "0,5 à 10");
        t.insert(
            (Language::French, DefaultFrequencyRange),
            "20 Hz à 20 000 Hz",
        );
        t.insert((Language::French, DefaultPeqModel), "pk");
        t.insert((Language::French, DefaultAlgorithm), "autoeq:de");
        t.insert((Language::French, DefaultPopulation), "100");
        t.insert((Language::French, DefaultMaxEvaluations), "5000");
        t.insert((Language::French, DefaultTolerance), "1e-6");
        t.insert((Language::French, DefaultStrategy), "currenttobest1bin");
        t.insert((Language::French, DefaultMutationF), "0,5");
        t.insert((Language::French, DefaultRecombinationCr), "0,9");
        t.insert(
            (Language::French, DefaultAdaptiveWeights),
            "0,0 (désactivé)",
        );
        t.insert((Language::French, DefaultEnableRefine), "activé");
        t.insert((Language::French, DefaultLocalAlgorithm), "cobyla");
        t.insert((Language::French, DefaultTiltType), "flat");
        t.insert((Language::French, DefaultSlopeDbOct), "0,0");
        t.insert((Language::French, DefaultAutoDetectF3), "activé");
        t.insert((Language::French, DefaultFilterOrder), "4 (24 dB/oct)");
        t.insert(
            (Language::French, DefaultSchroederFrequency),
            "200 Hz (dépend de la pièce)",
        );
        t.insert((Language::French, DefaultLowFreqMaxQ), "10");
        t.insert((Language::French, DefaultFrequencyRange2), "200-5000 Hz");
        t.insert((Language::French, DefaultMaxDelay), "5 ms");
        t.insert(
            (Language::French, DefaultStrategy2),
            "primary with constraints",
        );
        t.insert((Language::French, DefaultMaxDeviation), "3 dB");

        // Field tips
        t.insert((Language::French, TipSystemType), "Utilisez 'multisub' lors de l'optimisation d'un système avec des subwoofers placées indépendamment. 'DBA' est pour les arrays à double basse.");
        t.insert((Language::French, TipLossFunction), "Commencez par 'flat' pour la précision. Passez à 'score' si vous voulez un emphasis des graves perceptuelle.");
        t.insert((Language::French, TipTargetCurve), "Pour les casques, Harman 2018 est la cible la plus validée. Pour lesenceintes, 'flat' est typique sauf si vous avez une courbe de maison préférée.");
        t.insert((Language::French, TipMode), "Utilisez IIR pour la lecture en temps réel. FIR pour le mastering hors ligne ou quand la linéarité de phase importe.");
        t.insert((Language::French, TipNumFilters), "5-9 filtres est un bon équilibre. Au-dessus de 12, les rendements décroissants s'installent et le résultat peut sembler pire.");
        t.insert((Language::French, TipFirTaps), "À 48 kHz, 4096 taps = ~85 ms de latence. Utilisez 2048 pour une latence plus faible, 8192+ pour une correction de pièce chirurgicale.");
        t.insert((Language::French, TipFirPhase), "La phase linéaire est plus sûre pour les casques. Phase minimum pour lesenceintes où le pré-ringing compte.");
        t.insert((Language::French, TipDbRange), "Limiter le boost à +6 dB empêche les résonances excessives. Permettez plus de coupe que de boost — la coupe est toujours plus sûre.");
        t.insert((Language::French, TipQRange), "Gardez le Q max en dessous de 10 pour éviter le ronronnement. Un Q autour de 1-2 donne des corrections naturelles.");
        t.insert((Language::French, TipFrequencyRange), "Pour l'EQ de pièce, restreignez à la région où vous avez des mesures fiables (ex: 20-500 Hz pour les subwoofers).");
        t.insert((Language::French, TipPeqModel), "Pour l'EQ de casque, 'pk' est usually suffisant. Pour lesenceintes, 'hp-pk-lp' ou 'ls-pk-hs' donne un meilleur contrôle des basses/hautes fréquences.");
        t.insert((Language::French, TipAlgorithm), "'autoeq:de' est le meilleur choix — c'est une variante DE affinée. Utilisez 'nlopt:cobyla' pour un raffinement local rapide uniquement.");
        t.insert((Language::French, TipPopulation), "50-200 pour des résultats rapides, 500+ pour des recherches approfondies sur des cibles difficiles.");
        t.insert((Language::French, TipMaxEvaluations), "5000 est un bon équilibre. Augmentez à 20000+ pour une optimisation multi-sub complexe.");
        t.insert((Language::French, TipTolerance), "Tolérance plus basse = recherche plus longue mais résultat potentiellement meilleur. 1e-4 est suffisant pour des aperçus rapides.");
        t.insert((Language::French, TipStrategy), "Laissez par défaut sauf si vous comprenez la théorie DE. 'lshade' est une variante auto-adaptative qui ne nécessite aucun ajustement.");
        t.insert(
            (Language::French, TipMutationF),
            "La plage 0,4-0,9 fonctionne bien. En dessous de 0,3 peut stagner.",
        );
        t.insert((Language::French, TipRecombinationCr), "CR élevé (0,8-1,0) pour des paramètres corrélés. CR faible (0,1-0,3) pour des paramètres indépendants.");
        t.insert((Language::French, TipAdaptiveWeights), "Réglez sur 0,1-0,3 pour un accord automatique. Utile quand vous ne connaissez pas les bonnes valeurs de F/CR.");
        t.insert((Language::French, TipEnableRefine), "Gardez presque toujours activé. Désactivez uniquement pour la vitesse lors des aperçus rapides.");
        t.insert(
            (Language::French, TipLocalAlgorithm),
            "COBYLA est le choix le plus sûr. Essayez BOBYQA si COBYLA est lent.",
        );
        t.insert((Language::French, TipTiltType), "Commencez par 'flat'. Ajoutez une inclinaison seulement si le résultat plat semble trop brillant ou trop chaud.");
        t.insert(
            (Language::French, TipSlopeDbOct),
            "Une pente de -0,5 dB/octave donne une inclinaison chaude douce.",
        );
        t.insert((Language::French, TipAutoDetectF3), "Désactivez et réglez manuellement si la détection automatique choisit la mauvaise fréquence.");
        t.insert(
            (Language::French, TipFilterOrder),
            "Ordre 2 est doux, ordre 4 est typique, ordre 6 est agressif.",
        );
        t.insert((Language::French, TipSchroederFrequency), "Calculez à partir des dimensions de la pièce : F = 2000 * sqrt(RT60 / V) où V est le volume en m³.");
        t.insert(
            (Language::French, TipLowFreqMaxQ),
            "Permettez Q jusqu'à 15-20 pour des modes de piècestubborn.",
        );
        t.insert(
            (Language::French, TipFrequencyRange2),
            "Concentrez-vous sur les régions de crossover où les haut-parleurs se chevauchent.",
        );
        t.insert((Language::French, TipMaxDelay), "Gardez en dessous de 10 ms pour éviter un filtrage en peigne audible avec le son direct.");
        t.insert((Language::French, TipStrategy2), "Utilisez 'primary' quand il y a un sweet spot clair. Utilisez 'minimize variance' pour un seating de type cinéma.");
        t.insert((Language::French, TipMaxDeviation), "Contrainte plus serrée (1-2 dB) donne des résultats plus uniformes mais peut limiter l'amélioration du siège principal.");

        // Labels
        t.insert((Language::French, LabelDefault), "Par défaut :");
        t.insert((Language::French, LabelTip), "Astuce :");
    }

    fn add_german(t: &mut TranslationMap) {
        use DocKey::*;

        // Block titles
        t.insert((Language::German, DocOverviewTitle), "AutoEQ Parameter");
        t.insert((Language::German, DocGoalsTitle), "Ziele und Konfiguration");
        t.insert((Language::German, DocEqDesignTitle), "EQ-Design");
        t.insert((Language::German, DocRangesTitle), "Parameterbereiche");
        t.insert((Language::German, DocPeqModelTitle), "PEQ-Modell");
        t.insert(
            (Language::German, DocOptimizerTitle),
            "Optimizer-Einstellungen",
        );
        t.insert(
            (Language::German, DocDeParamsTitle),
            "Differenzielle Evolution",
        );
        t.insert((Language::German, DocRefinementTitle), "Verfeinerung");
        t.insert((Language::German, DocTargetTiltTitle), "Ziel-Neigung");
        t.insert((Language::German, DocExcursionTitle), "Exkursionsschutz");
        t.insert(
            (Language::German, DocSchroederTitle),
            "Schroeder-Aufteilung",
        );
        t.insert(
            (Language::German, DocPhaseAlignmentTitle),
            "Phasenausrichtung",
        );
        t.insert(
            (Language::German, DocMultiSeatTitle),
            "Multi-Sitz-Optimierung",
        );

        // Block overviews
        t.insert((Language::German, DocOverviewOverview), "Konfigurieren Sie den parametrischen EQ-Optimierer. Das Formular ist in drei Bereiche unterteilt: Ziele (was optimiert werden soll), EQ-Design (Filtereinschränkungen) und Optimierer-Einstellungen (wie die Suche durchgeführt wird). Fahren Sie mit der Maus über einen Abschnitt auf der linken Seite, um hier detaillierte Hilfe zu sehen.");
        t.insert((Language::German, DocGoalsOverview), "Definiert, was der Optimierer zu erreichen versucht und die Zielfrequenzgang, gegen die er optimiert.");
        t.insert(
            (Language::German, DocEqDesignOverview),
            "Steuert den Typ und die Anzahl der Filter, die der Optimierer verwenden kann.",
        );
        t.insert((Language::German, DocRangesOverview), "Grenzen für Verstärkung, Q-Faktor und Frequenz für jeden Filter. Engere Grenzen beschleunigen die Suche, können aber verhindern, dass der Optimierer die beste Lösung findet.");
        t.insert((Language::German, DocPeqModelOverview), "Bestimmt, welche Filtertypen der Optimierer verwenden kann. Flexiblere Modelle können bessere Ergebnisse erzielen, erhöhen aber die Suchkomplexität.");
        t.insert(
            (Language::German, DocOptimizerOverview),
            "Steuert den Suchalgorithmus, sein Budget und die Konvergenzkriterien.",
        );
        t.insert((Language::German, DocDeParamsOverview), "Feinabstimmungsparameter für den DE-Algorithmus. Diese steuern, wie aggressiv die Suche zwischen Erkundung und Nutzung bekannter guter Lösungen wechselt.");
        t.insert((Language::German, DocRefinementOverview), "Nach der globalen Suche poliert ein lokaler Optimierer das Ergebnis. Dies verbessert die Lösung typischerweise um 0,5-2 dB.");
        t.insert((Language::German, DocTargetTiltOverview), "Wendet eine frequenzabhängige Steigung auf die Zielkurve an. Nützlich zum Anpassen an eine bevorzugte Hauskurve oder zum Kompensieren von Raumgewinn.");
        t.insert((Language::German, DocExcursionOverview), "Fügt einen Hochpassfilter hinzu, um Woofer vor übermäßigem Tiefton-Boost zu schützen, der mechanische Schäden verursachen könnte.");
        t.insert((Language::German, DocSchroederOverview), "Teilt die Optimierung in Niederfrequenz- und Hochfrequenzbereiche an der Schroeder-Frequenz (Übergang zwischen modalem und statistischem Raumverhalten). Jeder Bereich erhält unabhängige Q-Einschränkungen.");
        t.insert((Language::German, DocPhaseAlignmentOverview), "Optimiert das relative Timing zwischen Treibern (Woofer, Mittelton, Hochtöner) für kohärente Summation an den Übergangsfrequenzen.");
        t.insert((Language::German, DocMultiSeatOverview), "Optimiert den EQ für mehrere Hörpositionen gleichzeitig und findet einen Kompromiss, der an allen Sitzen vernünftig funktioniert.");

        // Field names
        t.insert((Language::German, FieldSystemType), "Systemtyp");
        t.insert((Language::German, FieldLossFunction), "Verlustfunktion");
        t.insert((Language::German, FieldTargetCurve), "Zielkurve");
        t.insert((Language::German, FieldMode), "Modus");
        t.insert((Language::German, FieldNumFilters), "Anz. Filter");
        t.insert((Language::German, FieldFirTaps), "FIR-Taps");
        t.insert((Language::German, FieldFirPhase), "FIR-Phase");
        t.insert((Language::German, FieldDbRange), "dB-Bereich (min/max)");
        t.insert((Language::German, FieldQRange), "Q-Bereich (min/max)");
        t.insert(
            (Language::German, FieldFrequencyRange),
            "Frequenzbereich (min/max)",
        );
        t.insert((Language::German, FieldPeqModel), "PEQ-Modell");
        t.insert((Language::German, FieldAlgorithm), "Algorithmus");
        t.insert((Language::German, FieldPopulation), "Population");
        t.insert((Language::German, FieldMaxEvaluations), "Max. Auswertungen");
        t.insert((Language::German, FieldTolerance), "Toleranz");
        t.insert((Language::German, FieldStrategy), "Strategie");
        t.insert((Language::German, FieldMutationF), "Mutation F");
        t.insert((Language::German, FieldRecombinationCr), "Rekombination CR");
        t.insert(
            (Language::German, FieldAdaptiveWeights),
            "Adaptive Gewichte",
        );
        t.insert(
            (Language::German, FieldEnableRefine),
            "Verfeinerung aktivieren",
        );
        t.insert(
            (Language::German, FieldLocalAlgorithm),
            "Lokaler Algorithmus",
        );
        t.insert((Language::German, FieldTiltType), "Neigungstyp");
        t.insert((Language::German, FieldSlopeDbOct), "Steigung (dB/Oktave)");
        t.insert((Language::German, FieldAutoDetectF3), "Auto-Erkennung F3");
        t.insert((Language::German, FieldFilterOrder), "Filterordnung");
        t.insert(
            (Language::German, FieldSchroederFrequency),
            "Schroeder-Frequenz",
        );
        t.insert((Language::German, FieldLowFreqMaxQ), "NF Max Q");
        t.insert((Language::German, FieldFrequencyRange2), "Frequenzbereich");
        t.insert((Language::German, FieldMaxDelay), "Max. Verzögerung");
        t.insert((Language::German, FieldStrategy2), "Strategie");
        t.insert((Language::German, FieldMaxDeviation), "Max. Abweichung");

        // Field descriptions
        t.insert((Language::German, DescSystemType), "Die Lautsprecher-Topologie. Beeinflusst, wie Messungen kombiniert werden und welche Verlustfunktionen verfügbar sind.");
        t.insert((Language::German, DescLossFunction), "'Flat' minimiert die RMS-Abweichung von der Zielkurve. 'Score' optimiert die Harman/Olive Hörpräferenz-Bewertung, die einen kontrollierten Bass-Shelf erlaubt.");
        t.insert((Language::German, DescTargetCurve), "Der Referenzfrequenzgang, den der Optimierer zu erreichen versucht. Harman-Kurven sind forschungsbasierte Präferenzziele. 'Flat' zielt auf einen perfekt flachen Frequenzgang.");
        t.insert((Language::German, DescMode), "IIR verwendet parametrische Biquad-Filter (geringe Latenz, minimale Phase). FIR verwendet eine endliche Impulsantwort (lineare Phase, höhere Latenz). Mixed kombiniert beides.");
        t.insert((Language::German, DescNumFilters), "Maximale Anzahl parametrischer EQ-Bänder. Mehr Filter können das Ziel genauer erreichen, aber Risiko von Überanpassung und hörbarem Klingeln.");
        t.insert((Language::German, DescFirTaps), "Länge des FIR-Filters in Samples. Mehr Taps geben feinere Frequenzauflösung, erhöhen aber Latenz und CPU-Kosten.");
        t.insert((Language::German, DescFirPhase), "'Linear' erhält Transientien, fügt aber Latenz hinzu. 'Minimum' konzentriert Energie am Anfang (geringere Latenz). 'Kirkeby' ist ein Inverse-Filter-Ansatz für Raumkorrektur.");
        t.insert(
            (Language::German, DescDbRange),
            "Maximaler Boost und Cut pro Filterband in Dezibel.",
        );
        t.insert((Language::German, DescQRange), "Bandbreite jedes Filters. Niedriges Q = breit und sanft, hohes Q = eng und chirurgisch.");
        t.insert((Language::German, DescFrequencyRange), "Die Frequenzgrenzen für die Filterplatzierung. Filter werden nur innerhalb dieses Bereichs platziert.");
        t.insert((Language::German, DescPeqModel), "'pk' = nur Peak-Filter. 'hp-pk' fügt einen Hochpass hinzu. 'hp-pk-lp' fügt Hoch- und Tiefpass hinzu. 'ls-pk-hs' fügt Shelf-Filter hinzu. 'free' erlaubt jede Kombination.");
        t.insert((Language::German, DescAlgorithm), "Die Optimierungsmethode. DE (Differential Evolution) ist eine robuste globale Suche. COBYLA/BOBYQA sind schnelle lokale Methoden. PSO/RGA/TLBO sind alternative Metaheuristiken.");
        t.insert((Language::German, DescPopulation), "Anzahl der Kandidatenlösungen in der populationsbasierten Suche. Größere Populationen erkunden mehr, dauern aber länger.");
        t.insert((Language::German, DescMaxEvaluations), "Maximale Anzahl an Zielfunktionsauswertungen. Der Optimierer stoppt nach dieser Anzahl, auch wenn er nicht konvergiert ist.");
        t.insert((Language::German, DescTolerance), "Konvergenzschwelle. Der Optimierer stoppt früh, wenn die Verbesserung unter diesem Wert liegt.");
        t.insert((Language::German, DescStrategy), "Die Mutationsstrategie. 'currenttobest1bin' balanciert Erkundung und Nutzung. 'best1bin' konvergiert schneller, kann aber globale Optima verfehlen.");
        t.insert((Language::German, DescMutationF), "Differentielles Gewicht — steuert Schrittgröße im Suchraum. Höher = mehr Erkundung, niedriger = feinere Einstellung.");
        t.insert((Language::German, DescRecombinationCr), "Crossover-Wahrscheinlichkeit — wie viel des Versuchsvektors vom Mutanten vs. vom Elternteil stammt.");
        t.insert((Language::German, DescAdaptiveWeights), "Wenn > 0, werden Mutation F und CR während der Suche selbstadaptiert. Das Gewicht steuert, wie schnell die Adaptation erfolgt.");
        t.insert(
            (Language::German, DescEnableRefine),
            "Führen Sie einen lokalen Optimierer aus, nachdem die globale Suche beendet ist.",
        );
        t.insert((Language::German, DescLocalAlgorithm), "Der lokale Optimierer für die Verfeinerung. COBYLA ist robust und derivatfrei. BOBYQA ist schneller, aber weniger stabil nahe Einschränkungen.");
        t.insert((Language::German, DescTiltType), "'Harman' wendet die Harman-forschungsbasierte Steigung an. 'Custom' ermöglicht manuelle Einstellung. 'Flat' deaktiviert die Neigung.");
        t.insert(
            (Language::German, DescSlopeDbOct),
            "Die Neigungsrate. Negativ = wärmer (mehr Bass), positiv = heller (mehr Hochton).",
        );
        t.insert((Language::German, DescAutoDetectF3), "Erkennt automatisch den -3 dB-Punkt des Lautsprechers aus der Messung und platziert den Schutzfilter dort.");
        t.insert((Language::German, DescFilterOrder), "Steilheit des Schutz-Hochpasses. Höhere Ordnung = steilerer Abfall = mehr Schutz, aber mehr Phasenverschiebung.");
        t.insert(
            (Language::German, DescSchroederFrequency),
            "Der Übergang zwischen modalem (darunter) und diffusem (darüber) Raumverhalten.",
        );
        t.insert((Language::German, DescLowFreqMaxQ), "Max Q für Filter unterhalb der Schroeder-Frequenz. Raummoden sind eng, daher ist höheres Q hier nützlich.");
        t.insert(
            (Language::German, DescFrequencyRange2),
            "Der Bereich, über den die Phasenausrichtung optimiert wird.",
        );
        t.insert(
            (Language::German, DescMaxDelay),
            "Maximale Zeitkorrektur in Millisekunden.",
        );
        t.insert((Language::German, DescStrategy2), "'Primary with constraints' optimiert für den Hauptsitz und begrenzt die Verschlechterung an anderen. 'Average' behandelt alle Sitze gleich. 'Minimize variance' reduziert die Streuung.");
        t.insert(
            (Language::German, DescMaxDeviation),
            "Maximal erlaubte Verschlechterung an sekundären Sitzen (dB).",
        );

        // Field defaults
        t.insert((Language::German, DefaultSystemType), "stereo");
        t.insert((Language::German, DefaultLossFunction), "flat");
        t.insert(
            (Language::German, DefaultTargetCurve),
            "Harman 2018 (Kopfhörer) / flat (Lautsprecher)",
        );
        t.insert((Language::German, DefaultMode), "iir");
        t.insert((Language::German, DefaultNumFilters), "7");
        t.insert((Language::German, DefaultFirTaps), "4096");
        t.insert((Language::German, DefaultFirPhase), "linear");
        t.insert((Language::German, DefaultDbRange), "-12 bis +6 dB");
        t.insert((Language::German, DefaultQRange), "0,5 bis 10");
        t.insert(
            (Language::German, DefaultFrequencyRange),
            "20 Hz bis 20.000 Hz",
        );
        t.insert((Language::German, DefaultPeqModel), "pk");
        t.insert((Language::German, DefaultAlgorithm), "autoeq:de");
        t.insert((Language::German, DefaultPopulation), "100");
        t.insert((Language::German, DefaultMaxEvaluations), "5000");
        t.insert((Language::German, DefaultTolerance), "1e-6");
        t.insert((Language::German, DefaultStrategy), "currenttobest1bin");
        t.insert((Language::German, DefaultMutationF), "0,5");
        t.insert((Language::German, DefaultRecombinationCr), "0,9");
        t.insert(
            (Language::German, DefaultAdaptiveWeights),
            "0,0 (deaktiviert)",
        );
        t.insert((Language::German, DefaultEnableRefine), "aktiviert");
        t.insert((Language::German, DefaultLocalAlgorithm), "cobyla");
        t.insert((Language::German, DefaultTiltType), "flat");
        t.insert((Language::German, DefaultSlopeDbOct), "0,0");
        t.insert((Language::German, DefaultAutoDetectF3), "aktiviert");
        t.insert((Language::German, DefaultFilterOrder), "4 (24 dB/Okt)");
        t.insert(
            (Language::German, DefaultSchroederFrequency),
            "200 Hz (raumabhängig)",
        );
        t.insert((Language::German, DefaultLowFreqMaxQ), "10");
        t.insert((Language::German, DefaultFrequencyRange2), "200-5000 Hz");
        t.insert((Language::German, DefaultMaxDelay), "5 ms");
        t.insert(
            (Language::German, DefaultStrategy2),
            "primary with constraints",
        );
        t.insert((Language::German, DefaultMaxDeviation), "3 dB");

        // Field tips
        t.insert((Language::German, TipSystemType), "Verwenden Sie 'multisub' bei der Optimierung eines Systems mit unabhängig platzierten Subwoofern. 'DBA' ist für Doppelbass-Arrays.");
        t.insert((Language::German, TipLossFunction), "Beginnen Sie mit 'flat' für Genauigkeit. Wechseln Sie zu 'score', wenn Sie perceptuell abgestimmte Bassanhebung möchten.");
        t.insert((Language::German, TipTargetCurve), "Für Kopfhörer ist Harman 2018 das am besten validierte Ziel. Für Lautsprecher ist 'flat' typisch, es sei denn, Sie haben eine bevorzugte Hauskurve.");
        t.insert((Language::German, TipMode), "Verwenden Sie IIR für Wiedergabe in Echtzeit. FIR für Offline-Mastering oder wenn Phasenlinearität wichtig ist.");
        t.insert((Language::German, TipNumFilters), "5-9 Filter ist ein guter Kompromiss. Über 12 treten diminishing Returns ein und das Ergebnis kann schlechter klingen.");
        t.insert((Language::German, TipFirTaps), "Bei 48 kHz sind 4096 Taps = ~85 ms Latenz. Verwenden Sie 2048 für geringere Latenz, 8192+ für chirurgische Raumkorrektur.");
        t.insert((Language::German, TipFirPhase), "Lineare Phase ist am sichersten für Kopfhörer. Minimum-Phase für Lautsprecher, wo Pre-Ringing wichtig ist.");
        t.insert((Language::German, TipDbRange), "Boost auf +6 dB begrenzen verhindert übermäßige Resonanzen. Erlauben Sie mehr Cut als Boost — Cut ist immer sicherer.");
        t.insert((Language::German, TipQRange), "Halten Sie max Q unter 10, um Klingeln zu vermeiden. Q um 1-2 gibt natürlich klingende Korrekturen.");
        t.insert((Language::German, TipFrequencyRange), "Für Raum-EQ einschränken auf den Bereich, wo Sie zuverlässige Messungen haben (z.B. 20-500 Hz für Subwoofer).");
        t.insert((Language::German, TipPeqModel), "Für Kopfhörer-EQ ist 'pk' normalerweise ausreichend. Für Lautsprecher geben 'hp-pk-lp' oder 'ls-pk-hs' bessere Kontrolle über Bass/Hochton.");
        t.insert((Language::German, TipAlgorithm), "'autoeq:de' ist die beste Standardwahl — es ist eine abgestimmte DE-Variante. Verwenden Sie 'nlopt:cobyla' nur für schnelle lokale Verfeinerung.");
        t.insert(
            (Language::German, TipPopulation),
            "50-200 für schnelle Ergebnisse, 500+ für gründliche Suchen bei schwierigen Zielen.",
        );
        t.insert((Language::German, TipMaxEvaluations), "5000 ist ein guter Kompromiss. Erhöhen Sie auf 20000+ für komplexe Multi-Sub-Optimierung.");
        t.insert((Language::German, TipTolerance), "Niedrigere Toleranz = längere Suche, aber potentiell besseres Ergebnis. 1e-4 ist ausreichend für schnelle Vorschauen.");
        t.insert((Language::German, TipStrategy), "Standard belassen, außer Sie verstehen DE-Theorie. 'lshade' ist eine selbstadaptive Variante, die keine Abstimmung braucht.");
        t.insert(
            (Language::German, TipMutationF),
            "Bereich 0,4-0,9 funktioniert gut. Unter 0,3 kann stagnieren.",
        );
        t.insert((Language::German, TipRecombinationCr), "Hohes CR (0,8-1,0) für korrelierte Parameter. Niedriges CR (0,1-0,3) für unabhängige Parameter.");
        t.insert((Language::German, TipAdaptiveWeights), "Auf 0,1-0,3 setzen für automatische Abstimmung. Nützlich, wenn Sie keine guten F/CR-Werte kennen.");
        t.insert((Language::German, TipEnableRefine), "Fast immer aktiviert lassen. Nur für Geschwindigkeit bei schnellen Vorschauen deaktivieren.");
        t.insert(
            (Language::German, TipLocalAlgorithm),
            "COBYLA ist die sicherste Wahl. Versuchen Sie BOBYQA, wenn COBYLA langsam ist.",
        );
        t.insert((Language::German, TipTiltType), "Beginnen Sie mit 'flat'. Fügen Sie nur Neigung hinzu, wenn das flache Ergebnis zu hell oder zu warm klingt.");
        t.insert(
            (Language::German, TipSlopeDbOct),
            "Eine Steigung von -0,5 dB/Oktave gibt eine sanfte warme Neigung.",
        );
        t.insert((Language::German, TipAutoDetectF3), "Deaktivieren und manuell einstellen, wenn die Autoerkennung die falsche Frequenz wählt.");
        t.insert(
            (Language::German, TipFilterOrder),
            "Ordnung 2 ist sanft, Ordnung 4 ist typisch, Ordnung 6 ist aggressiv.",
        );
        t.insert((Language::German, TipSchroederFrequency), "Aus Raumabmessungen berechnen: F = 2000 * sqrt(RT60 / V) wobei V das Volumen in m³ ist.");
        t.insert(
            (Language::German, TipLowFreqMaxQ),
            "Erlauben Sie Q bis zu 15-20 für hartnäckige Raummoden.",
        );
        t.insert(
            (Language::German, TipFrequencyRange2),
            "Konzentrieren Sie sich auf die Übergangsbereiche, wo Treiber überlappen.",
        );
        t.insert((Language::German, TipMaxDelay), "Unter 10 ms halten, um hörbares Fingerprint-Kammerfiltern mit dem Direktschall zu vermeiden.");
        t.insert((Language::German, TipStrategy2), "Verwenden Sie 'primary', wenn es einen klaren Sweetspot gibt. Verwenden Sie 'minimize variance' für Kino-Sitzordnung.");
        t.insert((Language::German, TipMaxDeviation), "Engerer Spielraum (1-2 dB) gibt einheitlichere Ergebnisse, kann aber die Verbesserung des Primärsitzes begrenzen.");

        // Labels
        t.insert((Language::German, LabelDefault), "Standard:");
        t.insert((Language::German, LabelTip), "Tipp:");
    }

    fn add_spanish(t: &mut TranslationMap) {
        use DocKey::*;

        // Block titles
        t.insert((Language::Spanish, DocOverviewTitle), "Parámetros AutoEQ");
        t.insert(
            (Language::Spanish, DocGoalsTitle),
            "Objetivos y Configuración",
        );
        t.insert((Language::Spanish, DocEqDesignTitle), "Diseño de EQ");
        t.insert((Language::Spanish, DocRangesTitle), "Rangos de Parámetros");
        t.insert((Language::Spanish, DocPeqModelTitle), "Modelo PEQ");
        t.insert(
            (Language::Spanish, DocOptimizerTitle),
            "Configuración del Optimizador",
        );
        t.insert(
            (Language::Spanish, DocDeParamsTitle),
            "Evolución Diferencial",
        );
        t.insert((Language::Spanish, DocRefinementTitle), "Refinamiento");
        t.insert(
            (Language::Spanish, DocTargetTiltTitle),
            "Inclinación del Objetivo",
        );
        t.insert(
            (Language::Spanish, DocExcursionTitle),
            "Protección de Excursión",
        );
        t.insert(
            (Language::Spanish, DocSchroederTitle),
            "División de Schroeder",
        );
        t.insert(
            (Language::Spanish, DocPhaseAlignmentTitle),
            "Alineación de Fase",
        );
        t.insert(
            (Language::Spanish, DocMultiSeatTitle),
            "Optimización Multi-Asiento",
        );

        // Block overviews
        t.insert((Language::Spanish, DocOverviewOverview), "Configure el optimizador de EQ paramétrico. El formulario se divide en tres áreas: objetivos (qué optimizar), diseño de EQ (restricciones de filtros) y configuración del optimizador (cómo se realiza la búsqueda). Pase el mouse sobre una sección a la izquierda para ver la ayuda detallada aquí.");
        t.insert((Language::Spanish, DocGoalsOverview), "Define lo que el optimizador intenta lograr y la respuesta objetivo contra la que iguala.");
        t.insert(
            (Language::Spanish, DocEqDesignOverview),
            "Controla el tipo y número de filtros que el optimizador puede usar.",
        );
        t.insert((Language::Spanish, DocRangesOverview), "Límites de ganancia, factor Q y frecuencia para cada filtro. Límites más estrictos aceleran la búsqueda pero pueden impedir que el optimizador encuentre la mejor solución.");
        t.insert((Language::Spanish, DocPeqModelOverview), "Determina qué tipos de filtros puede usar el optimizador. Modelos más flexibles pueden lograr mejores resultados pero aumentan la complejidad de la búsqueda.");
        t.insert(
            (Language::Spanish, DocOptimizerOverview),
            "Controla el algoritmo de búsqueda, su presupuesto y criterios de convergencia.",
        );
        t.insert((Language::Spanish, DocDeParamsOverview), "Parámetros de ajustefino para el algoritmo DE. Estos controlan qué tan agresivamente la búsqueda explora vs. explota soluciones buenas conocidas.");
        t.insert((Language::Spanish, DocRefinementOverview), "Después de la búsqueda global, un optimizador local pule el resultado. Esto típicamente mejora la solución en 0.5-2 dB.");
        t.insert((Language::Spanish, DocTargetTiltOverview), "Aplica una pendiente dependiente de la frecuencia a la curva objetivo. Útil para igualar una curva de casa preferida o compensar la ganancia de la sala.");
        t.insert((Language::Spanish, DocExcursionOverview), "Añade un filtro paso alto para proteger los woofers de un boost excesivo en bajas frecuencias que podría causar daño mecánico.");
        t.insert((Language::Spanish, DocSchroederOverview), "Divide la optimización en regiones de baja y alta frecuencia en la frecuencia de Schroeder (transición entre comportamiento modal y estadístico de la sala). Cada región recibe restricciones Q independientes.");
        t.insert((Language::Spanish, DocPhaseAlignmentOverview), "Optimiza el tiempo relativo entre los altavoces (woofer, medio, tweeter) para una suma coherente en las frecuencias de cruce.");
        t.insert((Language::Spanish, DocMultiSeatOverview), "Optimiza el EQ para múltiples posiciones de escucha simultáneamente, encontrando un compromiso que funcione razonablemente en todos los asientos.");

        // Field names
        t.insert((Language::Spanish, FieldSystemType), "Tipo de Sistema");
        t.insert((Language::Spanish, FieldLossFunction), "Función de Pérdida");
        t.insert((Language::Spanish, FieldTargetCurve), "Curva Objetivo");
        t.insert((Language::Spanish, FieldMode), "Modo");
        t.insert((Language::Spanish, FieldNumFilters), "Núm. de Filtros");
        t.insert((Language::Spanish, FieldFirTaps), "Taps FIR");
        t.insert((Language::Spanish, FieldFirPhase), "Fase FIR");
        t.insert((Language::Spanish, FieldDbRange), "Rango dB (min/max)");
        t.insert((Language::Spanish, FieldQRange), "Rango Q (min/max)");
        t.insert(
            (Language::Spanish, FieldFrequencyRange),
            "Rango de Frecuencia (min/max)",
        );
        t.insert((Language::Spanish, FieldPeqModel), "Modelo PEQ");
        t.insert((Language::Spanish, FieldAlgorithm), "Algoritmo");
        t.insert((Language::Spanish, FieldPopulation), "Población");
        t.insert(
            (Language::Spanish, FieldMaxEvaluations),
            "Evaluaciones Máx.",
        );
        t.insert((Language::Spanish, FieldTolerance), "Tolerancia");
        t.insert((Language::Spanish, FieldStrategy), "Estrategia");
        t.insert((Language::Spanish, FieldMutationF), "Mutación F");
        t.insert(
            (Language::Spanish, FieldRecombinationCr),
            "Recombinación CR",
        );
        t.insert(
            (Language::Spanish, FieldAdaptiveWeights),
            "Pesos Adaptativos",
        );
        t.insert(
            (Language::Spanish, FieldEnableRefine),
            "Activar Refinamiento",
        );
        t.insert((Language::Spanish, FieldLocalAlgorithm), "Algoritmo Local");
        t.insert((Language::Spanish, FieldTiltType), "Tipo de Inclinación");
        t.insert(
            (Language::Spanish, FieldSlopeDbOct),
            "Pendiente (dB/octava)",
        );
        t.insert((Language::Spanish, FieldAutoDetectF3), "Detección Auto F3");
        t.insert((Language::Spanish, FieldFilterOrder), "Orden del Filtro");
        t.insert(
            (Language::Spanish, FieldSchroederFrequency),
            "Frecuencia de Schroeder",
        );
        t.insert((Language::Spanish, FieldLowFreqMaxQ), "Q Máx. BF");
        t.insert(
            (Language::Spanish, FieldFrequencyRange2),
            "Rango de Frecuencia",
        );
        t.insert((Language::Spanish, FieldMaxDelay), "Retraso Máx.");
        t.insert((Language::Spanish, FieldStrategy2), "Estrategia");
        t.insert((Language::Spanish, FieldMaxDeviation), "Desviación Máx.");

        // Field descriptions
        t.insert((Language::Spanish, DescSystemType), "La topología del altavoz. Afecta cómo se combinan las mediciones y qué funciones de pérdida están disponibles.");
        t.insert((Language::Spanish, DescLossFunction), "'Flat' minimiza la desviación RMS de la curva objetivo. 'Score' optimiza la puntuación de preferencia de audición Harman/Olive, que permite un estante de graves controlado.");
        t.insert((Language::Spanish, DescTargetCurve), "La respuesta de frecuencia de referencia que el optimizador intenta igualar. Las curvas Harman son objetivos de preferencia basados en investigación. 'Flat' busca una respuesta perfectamente plana.");
        t.insert((Language::Spanish, DescMode), "IIR usa filtros biquad paramétricos (baja latencia, fase mínima). FIR usa una respuesta de impulsofinita (fase lineal, mayor latencia). Mixed combina ambos.");
        t.insert((Language::Spanish, DescNumFilters), "Número máximo de bandas de EQ paramétrico. Más filtros pueden igualar el objetivo más cerca, pero riesgo de sobreajuste y zumbido audible.");
        t.insert((Language::Spanish, DescFirTaps), "Longitud del filtro FIR en muestras. Más taps dan mayor resolución de frecuencia pero aumentan la latencia y el costo de CPU.");
        t.insert((Language::Spanish, DescFirPhase), "'Linear' preserva los transitorios pero agrega latencia. 'Minimum' concentra energía al inicio (menor latencia). 'Kirkeby' es un enfoque de filtro inverso para corrección de sala.");
        t.insert(
            (Language::Spanish, DescDbRange),
            "Boost y corte máximo por banda de filtro en decibelios.",
        );
        t.insert((Language::Spanish, DescQRange), "Ancho de banda de cada filtro. Q bajo = amplio y suave, Q alto = estrecho y quirúrgico.");
        t.insert((Language::Spanish, DescFrequencyRange), "Los límites de frecuencia para la colocación de filtros. Los filtros solo se colocarán dentro de este rango.");
        t.insert((Language::Spanish, DescPeqModel), "'pk' = solo filtros peak. 'hp-pk' añade un paso alto. 'hp-pk-lp' añade paso alto y paso bajo. 'ls-pk-hs' añade filtros estante. 'free' permite cualquier combinación.");
        t.insert((Language::Spanish, DescAlgorithm), "El método de optimización. DE (Evolución Diferencial) es una búsqueda global robusta. COBYLA/BOBYQA son métodos locales rápidos. PSO/RGA/TLBO son metaheurísticas alternativas.");
        t.insert((Language::Spanish, DescPopulation), "Número de soluciones candidatas en la búsqueda basada en población. Poblaciones más grandes exploran más pero toman más tiempo.");
        t.insert((Language::Spanish, DescMaxEvaluations), "Número máximo de evaluaciones de la función objetivo. El optimizador se detiene después de este número, incluso si no ha convergido.");
        t.insert((Language::Spanish, DescTolerance), "Umbral de convergencia. El optimizador se detiene temprano cuando la mejora cae por debajo de este valor.");
        t.insert((Language::Spanish, DescStrategy), "La estrategia de mutación. 'currenttobest1bin' balancea exploración y explotación. 'best1bin' converge más rápido pero puede perder óptimos globales.");
        t.insert((Language::Spanish, DescMutationF), "Peso diferencial — controla el tamaño del paso en el espacio de búsqueda. Mayor = más exploración, menor = ajuste más fino.");
        t.insert(
            (Language::Spanish, DescRecombinationCr),
            "Probabilidad de cruce — cuánto del vector de prueba viene del mutante vs. del padre.",
        );
        t.insert((Language::Spanish, DescAdaptiveWeights), "Cuando > 0, la mutación F y CR se autoadaptan durante la búsqueda. El peso controla qué tan rápida es la adaptación.");
        t.insert(
            (Language::Spanish, DescEnableRefine),
            "Ejecutar un optimizador local después de que termina la búsqueda global.",
        );
        t.insert((Language::Spanish, DescLocalAlgorithm), "El optimizador local usado para refinamiento. COBYLA es robusto y sin derivadas. BOBYQA es más rápido pero menos estable cerca de restricciones.");
        t.insert((Language::Spanish, DescTiltType), "'Harman' aplica la pendiente de investigación Harman. 'Custom' le permite establecer la pendiente manualmente. 'Flat' desactiva la inclinación.");
        t.insert((Language::Spanish, DescSlopeDbOct), "La tasa de inclinación. Negativo = más cálido (más graves), positivo = más brillante (más agudos).");
        t.insert((Language::Spanish, DescAutoDetectF3), "Detecta automáticamente el punto de -3 dB del altavoz a partir de la medición y coloca el filtro de protección allí.");
        t.insert((Language::Spanish, DescFilterOrder), "Pendiente del paso alto de protección. Orden mayor = caída más pronunciada = más protección pero más cambio de fase.");
        t.insert(
            (Language::Spanish, DescSchroederFrequency),
            "El cruce entre comportamiento modal (abajo) y difuso (arriba) de la sala.",
        );
        t.insert((Language::Spanish, DescLowFreqMaxQ), "Q máximo para filtros debajo de la frecuencia de Schroeder. Los modos de sala son estrechos, por lo que Q más alto es útil aquí.");
        t.insert(
            (Language::Spanish, DescFrequencyRange2),
            "La banda sobre la cual se optimiza la alineación de fase.",
        );
        t.insert(
            (Language::Spanish, DescMaxDelay),
            "Corrección máxima de alineación de tiempo en milisegundos.",
        );
        t.insert((Language::Spanish, DescStrategy2), "'Primary with constraints' optimiza para el asiento principal mientras limita la degradación en otros. 'Average' trata todos los asientos igualmente. 'Minimize variance' reduce la dispersión.");
        t.insert(
            (Language::Spanish, DescMaxDeviation),
            "Degradación máxima permitida en asientos secundarios (dB).",
        );

        // Field defaults
        t.insert((Language::Spanish, DefaultSystemType), "stereo");
        t.insert((Language::Spanish, DefaultLossFunction), "flat");
        t.insert(
            (Language::Spanish, DefaultTargetCurve),
            "Harman 2018 (auriculares) / flat (altavoces)",
        );
        t.insert((Language::Spanish, DefaultMode), "iir");
        t.insert((Language::Spanish, DefaultNumFilters), "7");
        t.insert((Language::Spanish, DefaultFirTaps), "4096");
        t.insert((Language::Spanish, DefaultFirPhase), "linear");
        t.insert((Language::Spanish, DefaultDbRange), "-12 a +6 dB");
        t.insert((Language::Spanish, DefaultQRange), "0,5 a 10");
        t.insert(
            (Language::Spanish, DefaultFrequencyRange),
            "20 Hz a 20.000 Hz",
        );
        t.insert((Language::Spanish, DefaultPeqModel), "pk");
        t.insert((Language::Spanish, DefaultAlgorithm), "autoeq:de");
        t.insert((Language::Spanish, DefaultPopulation), "100");
        t.insert((Language::Spanish, DefaultMaxEvaluations), "5000");
        t.insert((Language::Spanish, DefaultTolerance), "1e-6");
        t.insert((Language::Spanish, DefaultStrategy), "currenttobest1bin");
        t.insert((Language::Spanish, DefaultMutationF), "0,5");
        t.insert((Language::Spanish, DefaultRecombinationCr), "0,9");
        t.insert(
            (Language::Spanish, DefaultAdaptiveWeights),
            "0,0 (desactivado)",
        );
        t.insert((Language::Spanish, DefaultEnableRefine), "activado");
        t.insert((Language::Spanish, DefaultLocalAlgorithm), "cobyla");
        t.insert((Language::Spanish, DefaultTiltType), "flat");
        t.insert((Language::Spanish, DefaultSlopeDbOct), "0,0");
        t.insert((Language::Spanish, DefaultAutoDetectF3), "activado");
        t.insert((Language::Spanish, DefaultFilterOrder), "4 (24 dB/oct)");
        t.insert(
            (Language::Spanish, DefaultSchroederFrequency),
            "200 Hz (depende de la sala)",
        );
        t.insert((Language::Spanish, DefaultLowFreqMaxQ), "10");
        t.insert((Language::Spanish, DefaultFrequencyRange2), "200-5000 Hz");
        t.insert((Language::Spanish, DefaultMaxDelay), "5 ms");
        t.insert(
            (Language::Spanish, DefaultStrategy2),
            "primary with constraints",
        );
        t.insert((Language::Spanish, DefaultMaxDeviation), "3 dB");

        // Field tips
        t.insert((Language::Spanish, TipSystemType), "Use 'multisub' al optimizar un sistema con subwoofers colocados independientemente. 'DBA' es para arrays de doble bajo.");
        t.insert((Language::Spanish, TipLossFunction), "Comience con 'flat' para precisión. Cambie a 'score' si quiere énfasis de graves perceptual.");
        t.insert((Language::Spanish, TipTargetCurve), "Para auriculares, Harman 2018 es el objetivo más validado. Para altavoces, 'flat' es típico a menos que tenga una curva de casa preferida.");
        t.insert((Language::Spanish, TipMode), "Use IIR para reproducción en tiempo real. FIR para masterización offline o cuando la linealidad de fase importa.");
        t.insert((Language::Spanish, TipNumFilters), "5-9 filtros es un buen equilibrio. Por encima de 12, los rendimientos decrecientes se instalan y el resultado puede sonar peor.");
        t.insert((Language::Spanish, TipFirTaps), "A 48 kHz, 4096 taps = ~85 ms de latencia. Use 2048 para menor latencia, 8192+ para corrección de sala quirúrgica.");
        t.insert((Language::Spanish, TipFirPhase), "Fase lineal es más segura para auriculares. Fase mínima para altavoces donde el pre-ringing importa.");
        t.insert((Language::Spanish, TipDbRange), "Limitar el boost a +6 dB previene resonancias excesivas. Permita más corte que boost — el corte siempre es más seguro.");
        t.insert((Language::Spanish, TipQRange), "Mantenga Q máximo por debajo de 10 para evitar zumbido. Q alrededor de 1-2 da correcciones de sonido natural.");
        t.insert((Language::Spanish, TipFrequencyRange), "Para EQ de sala, restrinja a la región donde tiene mediciones confiables (ej. 20-500 Hz para subwoofers).");
        t.insert((Language::Spanish, TipPeqModel), "Para EQ de auriculares, 'pk' usually es suficiente. Para altavoces, 'hp-pk-lp' o 'ls-pk-hs' da mejor control de graves/agudos.");
        t.insert((Language::Spanish, TipAlgorithm), "'autoeq:de' es la mejor opción por defecto — es una variante DE ajustada. Use 'nlopt:cobyla' solo para refinamiento local rápido.");
        t.insert((Language::Spanish, TipPopulation), "50-200 para resultados rápidos, 500+ para búsquedas exhaustivas en objetivos difíciles.");
        t.insert(
            (Language::Spanish, TipMaxEvaluations),
            "5000 es un buen equilibrio. Aumente a 20000+ para optimización multi-sub compleja.",
        );
        t.insert((Language::Spanish, TipTolerance), "Tolerancia más baja = búsqueda más larga pero resultado potencialmente mejor. 1e-4 está bien para vistas previas rápidas.");
        t.insert((Language::Spanish, TipStrategy), "Deje en predeterminado a menos que entienda la teoría de DE. 'lshade' es una variante autoadaptativa que no necesita ajuste.");
        t.insert(
            (Language::Spanish, TipMutationF),
            "Rango 0.4-0.9 funciona bien. Por debajo de 0.3 puede estancarse.",
        );
        t.insert((Language::Spanish, TipRecombinationCr), "CR alto (0.8-1.0) para parámetros correlacionados. CR bajo (0.1-0.3) para parámetros independientes.");
        t.insert((Language::Spanish, TipAdaptiveWeights), "Establezca en 0.1-0.3 para ajuste automático. Útil cuando no conoce buenos valores de F/CR.");
        t.insert((Language::Spanish, TipEnableRefine), "Casi siempre mantenga esto activado. Solo desactive para velocidad durante vistas previas rápidas.");
        t.insert(
            (Language::Spanish, TipLocalAlgorithm),
            "COBYLA es la elección más segura. Pruebe BOBYQA si COBYLA es lento.",
        );
        t.insert((Language::Spanish, TipTiltType), "Comience con 'flat'. Añada inclinación solo si el resultado plano suena demasiado brillante o demasiado cálido.");
        t.insert(
            (Language::Spanish, TipSlopeDbOct),
            "Una pendiente de -0.5 dB/octava da una inclinación cálida suave.",
        );
        t.insert((Language::Spanish, TipAutoDetectF3), "Desactive y establezca manualmente si la detección automática elige la frecuencia incorrecta.");
        t.insert(
            (Language::Spanish, TipFilterOrder),
            "Orden 2 es suave, orden 4 es típico, orden 6 es agresivo.",
        );
        t.insert((Language::Spanish, TipSchroederFrequency), "Calcule a partir de las dimensiones de la sala: F = 2000 * sqrt(RT60 / V) donde V es el volumen en m³.");
        t.insert(
            (Language::Spanish, TipLowFreqMaxQ),
            "Permita Q hasta 15-20 para modos de sala persistentes.",
        );
        t.insert(
            (Language::Spanish, TipFrequencyRange2),
            "Concentrese en las regiones de cruce donde los altavoces se superponen.",
        );
        t.insert((Language::Spanish, TipMaxDelay), "Mantenga por debajo de 10 ms para evitar filtrado de peine audible con el sonido directo.");
        t.insert((Language::Spanish, TipStrategy2), "Use 'primary' cuando hay un punto óptimo claro. Use 'minimize variance' para asientos estilo cine.");
        t.insert((Language::Spanish, TipMaxDeviation), "Restricción más estricta (1-2 dB) da resultados más uniformes pero puede limitar la mejora del asiento principal.");

        // Labels
        t.insert((Language::Spanish, LabelDefault), "Predeterminado:");
        t.insert((Language::Spanish, LabelTip), "Consejo:");
    }
}

impl Default for AutoEqTranslations {
    fn default() -> Self {
        Self::new()
    }
}

/// Global state for AutoEQ i18n
pub struct AutoEqI18nState {
    pub language: Language,
    pub translations: AutoEqTranslations,
}

impl Global for AutoEqI18nState {}

impl AutoEqI18nState {
    pub fn new() -> Self {
        Self {
            language: Language::default(),
            translations: AutoEqTranslations::new(),
        }
    }

    pub fn t(&self, key: DocKey) -> &'static str {
        self.translations.get(self.language, key)
    }
}

impl Default for AutoEqI18nState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for easy AutoEQ i18n access
pub trait AutoEqI18nExt {
    fn autoeq_t(&self, key: DocKey) -> &'static str;
    fn autoeq_language(&self) -> Language;
}

impl AutoEqI18nExt for App {
    fn autoeq_t(&self, key: DocKey) -> &'static str {
        self.try_global::<AutoEqI18nState>()
            .map(|s| s.t(key))
            .unwrap_or("???")
    }

    fn autoeq_language(&self) -> Language {
        self.try_global::<AutoEqI18nState>()
            .map(|s| s.language)
            .unwrap_or_default()
    }
}
