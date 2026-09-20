// Derived review; no second configuration model or solver action.
{
    let option_label = |value: &str, options: &[(&str, &str)]| {
        options
            .iter()
            .find(|(id, _)| *id == value)
            .map_or(value, |(_, label)| *label)
            .to_owned()
    };
    let loss_options = loss_type_options_override.unwrap_or(match optimization_type {
        OptimizationType::Headphone => HEADPHONE_LOSS_TYPE_OPTIONS,
        OptimizationType::Speaker => LOSS_TYPE_OPTIONS,
    });
    let enabled_label = |enabled| {
        if enabled {
            stage_text.enabled
        } else {
            stage_text.disabled
        }
    };
    let target_label = match optimization_type {
        OptimizationType::Headphone => HEADPHONE_TARGET_CURVE_OPTIONS
            .iter()
            .find(|(value, _)| *value == config.goals.target_curve)
            .map(|(value, label)| {
                crate::app::i18n::HeadphoneIdentityTranslations::target_label(
                    self.meta.language, value, label,
                ).to_owned()
            }),
        OptimizationType::Speaker => SPEAKER_TARGET_CURVE_OPTIONS
            .iter()
            .chain(SPINORAMA_CURVE_OPTIONS.iter())
            .find(|(value, _)| *value == config.goals.target_curve)
            .map(|(_, label)| (*label).to_owned()),
    }.unwrap_or_else(|| config.goals.target_curve.clone());
    let design = &config.eq_design;
    let algorithm = &config.algorithm;
    let mut review = VStack::new()
        .spacing(StackSpacing::Md)
        .child(Text::body(stage_text.review_hint).color(theme.description_color))
        .child(Text::section_header(stage_text.stages[0]).color(theme.header_color))
        .child(Text::body(format!(
            "{}: {}",
            translations.autoeq_form.target, target_label
        )))
        .child(Text::body(format!(
            "{}: {}",
            translations.autoeq_form.parameters.loss_function, option_label(&config.goals.loss_type, loss_options)
        )));
    if !hide_room_sections && config.room_correction.use_target_tilt {
        review = review.child(Text::body(format!(
            "{} · {} dB/oct · {} Hz",
            option_label(&config.room_correction.tilt_type, ROOMEQ_TILT_TYPE_OPTIONS),
            config.room_correction.tilt_slope,
            config.room_correction.tilt_reference_freq
        )));
    }
    review = review
        .child(Text::section_header(stage_text.stages[1]).color(theme.header_color))
        .child(Text::body(format!(
            "{} · {} Hz · {}–{} Hz",
            option_label(&design.opt_mode, OPT_MODE_OPTIONS), design.sample_rate, design.min_freq, design.max_freq
        )))
        .child(Text::body(format!(
            "{}…{} dB", design.min_db, design.max_db
        )));
    // Match the configuration form: dormant IIR settings are not part of a FIR design.
    if matches!(design.opt_mode.as_str(), "iir" | "mixed" | "mixed_phase") {
        review = review
            .child(Text::body(format!(
                "{}: {} · {}",
                translations.autoeq_form.parameters.number_filters,
                design.num_filters,
                option_label(&design.peq_model, PEQ_MODEL_OPTIONS)
            )))
            .child(Text::body(format!("Q {}…{}", design.min_q, design.max_q)));
    }
    if matches!(design.opt_mode.as_str(), "fir" | "mixed" | "mixed_phase") {
        review = review.child(Text::body(format!(
            "{} · {} taps · {:.1} ms",
            option_label(&design.fir_phase, FIR_PHASE_OPTIONS),
            design.fir_taps,
            design.fir_taps as f64 / design.sample_rate.max(1) as f64 * 1000.0
        )));
    }
    review = review.child(Text::section_header(stage_text.stages[2]).color(theme.header_color));
    if hide_room_sections && hide_multi_measurement {
        review = review.child(Text::body(stage_text.no_timing).color(theme.description_color));
    } else {
        review = review
            .child(Text::body(format!(
                "{}: {}",
                translations.autoeq_allow_delay,
                enabled_label(config.v2.allow_delay)
            )))
            .child(Text::body(format!(
                "{}: {} · {}",
                translations.autoeq_form.parameters.smoothing,
                enabled_label(algorithm.smooth || algorithm.psychoacoustic),
                algorithm.smooth_n
            )));
    }
    review = review
        .child(Text::section_header(stage_text.stages[3]).color(theme.header_color))
        .child(Text::body(option_label(&algorithm.algo, ALGORITHM_OPTIONS)))
        .child(Text::body(format!(
            "{}: {}",
            translations.autoeq_form.blocks.max_evaluations, algorithm.maxeval
        )))
        .child(Text::body(format!(
            "{}: {}",
            translations.autoeq_form.blocks.population, algorithm.population
        )))
        .child(Text::body(format!(
            "{}: {} · {}: {}",
            translations.autoeq_form.blocks.tolerance,
            algorithm.tolerance,
            translations.autoeq_form.blocks.absolute_tolerance,
            algorithm.atolerance
        )))
        .child(Text::body(format!(
            "{}: {} · {}",
            translations.autoeq_form.blocks.local_refinement,
            enabled_label(algorithm.refine),
            option_label(&algorithm.local_algo, LOCAL_ALGO_OPTIONS)
        )));
    if config.v2.seed_enabled {
        review = review.child(Text::body(format!(
            "{}: {}",
            translations.autoeq_form.parameters.seed, config.v2.seed
        )));
    }
    div().id(id).child(review)
}
