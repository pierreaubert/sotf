// Expert mode render body — thin orchestrator that assembles section cards.
// This file is include!()'d from render.rs, sharing its scope.
{
    let mut form = VStack::new().spacing(StackSpacing::Lg);
    let base_id = id.clone();
    let is_narrow_layout = is_narrow_default_layout(available_width);

    // Compute FIR latency for capability and filter design sections
    let fir_latency_ms = config.eq_design.fir_taps as f64 / config.eq_design.sample_rate as f64 * 1000.0;

    let toggle_theme = theme.toggle_theme();

    // Section 0: Goals (loss type + target curve) — shown for spinorama/headphone
    if show_goals {
        form = form.child(include!("render_section_goals.rs"));
    }

    // Section 1: Capability (IIR / FIR / Mixed / Mixed Phase)
    if !hide_capability_section {
        form = form.child(include!("render_section_capability.rs"));
    }

    // Section 2: Target (listening distance + slope)
    if !hide_target_distance_section {
        form = form.child(include!("render_section_target.rs"));
    }

    // Section 3: Optimisation Goal (match target / natural / psychoacoustic)
    if !hide_optimization_goal_section {
        form = form.child(include!("render_section_optimization_goal.rs"));
    }

    // Section 4: Filter Design (IIR/FIR params, crossover, bass management)
    form = form.child(include!("render_section_filter_design.rs"));

    // Section 5: Delay Correction (conditional)
    if !hide_room_sections {
        form = form.child(include!("render_section_delay.rs"));
    }

    // Section 6: Multiple Measurements Per Speaker (conditional)
    if !hide_multi_measurement {
        form = form.child(include!("render_section_multi_measurement.rs"));
    }

    // Section 7: Home Cinema Specific (conditional)
    if optimization_type == OptimizationType::Speaker && !hide_room_sections {
        form = form.child(include!("render_section_home_cinema.rs"));
    }

    // Section 8: Optimisation Algorithm Configuration (conditional)
    if show_optimization_tuning {
        form = form.child(include!("render_section_algorithm.rs"));
    }

    div().id(id).child(form)
}
