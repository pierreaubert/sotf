use super::*;

/// Resolve the `MeasurementSource` for a logical role via the SystemConfig →
/// RoomConfig.speakers indirection. Workflows only accept `SpeakerConfig::Single`
/// roles; any other variant is rejected up front so the generic path doesn't
/// see half-processed data.
pub(super) fn resolve_single_source<'a>(
    role: &str,
    config: &'a RoomConfig,
    sys: &SystemConfig,
) -> Result<&'a crate::MeasurementSource> {
    let meas_key = sys
        .speakers
        .get(role)
        .ok_or_else(|| AutoeqError::InvalidConfiguration {
            message: format!("Missing speaker mapping for '{}'", role),
        })?;
    let cfg = config
        .speakers
        .get(meas_key)
        .ok_or_else(|| AutoeqError::InvalidConfiguration {
            message: format!("Missing speaker config for key '{}'", meas_key),
        })?;
    match cfg {
        SpeakerConfig::Single(s) => Ok(s),
        _ => Err(AutoeqError::InvalidConfiguration {
            message: format!("Workflow requires Single speaker config for '{}'", role),
        }),
    }
}

/// Helper to load curves for all logical channels
pub(super) fn load_logical_channels(
    config: &RoomConfig,
    sys: &SystemConfig,
) -> Result<HashMap<String, Curve>> {
    let mut curves = HashMap::new();
    for (role, meas_key) in &sys.speakers {
        if let Some(cfg) = config.speakers.get(meas_key) {
            let source = match cfg {
                SpeakerConfig::Single(s) => s,
                _ => {
                    return Err(AutoeqError::InvalidConfiguration {
                        message: format!("Workflow requires Single speaker config for '{}'", role),
                    });
                }
            };
            let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
                message: e.to_string(),
            })?;
            curves.insert(role.clone(), curve);
        }
    }
    Ok(curves)
}

// ============================================================================
// Sub Preprocessing for Stereo Workflows
// ============================================================================

/// Information about an individual subwoofer driver from multi-sub preprocessing
#[derive(Clone)]
pub(super) struct SubDriverInfo {
    /// Driver name (e.g., "subs_1", "Front Sub")
    pub(super) name: String,
    /// Gain in dB from MSO/DBA optimization
    pub(super) gain: f64,
    /// Delay in ms from MSO/DBA optimization
    pub(super) delay: f64,
    /// Whether this driver is polarity-inverted
    pub(super) inverted: bool,
    /// Initial measurement curve for this driver
    pub(super) initial_curve: Option<Curve>,
}

/// Result of subwoofer preprocessing
pub(super) struct SubPreprocessResult {
    /// Combined curve (for crossover optimization and shared post-EQ)
    pub(super) combined_curve: Curve,
    /// Per-driver info (None for single sub)
    pub(super) drivers: Option<Vec<SubDriverInfo>>,
}

/// Preprocess the LFE channel's SpeakerConfig into a combined curve and per-driver info.
///
/// Dispatches by SpeakerConfig variant:
/// - Single: load curve, no drivers
/// - MultiSub + Mso: run MSO optimization, return combined + per-sub gains/delays
/// - MultiSub + Single: average all subs, return combined + per-sub info (zero gains/delays)
/// - MultiSub + Dba: error (should use SpeakerConfig::Dba)
/// - Cardioid: simulate combined response from front + delayed/inverted rear
/// - Dba: run DBA optimization, return combined + front/rear info
/// - Group: error (handled by generic path)
pub(super) fn preprocess_sub(
    lfe_config: &SpeakerConfig,
    strategy: &SubwooferStrategy,
    optimizer: &crate::roomeq::types::OptimizerConfig,
    sample_rate: f64,
) -> Result<SubPreprocessResult> {
    match lfe_config {
        SpeakerConfig::Single(source) => {
            let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
                message: e.to_string(),
            })?;
            Ok(SubPreprocessResult {
                combined_curve: curve,
                drivers: None,
            })
        }
        SpeakerConfig::MultiSub(ms) => match strategy {
            SubwooferStrategy::Mso => preprocess_multisub_mso(ms, optimizer, sample_rate),
            SubwooferStrategy::Single => preprocess_multisub_independent(ms),
            SubwooferStrategy::Dba => Err(AutoeqError::InvalidConfiguration {
                message: "SubwooferStrategy::Dba requires SpeakerConfig::Dba, not MultiSub"
                    .to_string(),
            }),
        },
        SpeakerConfig::Cardioid(c) => preprocess_cardioid(c),
        SpeakerConfig::Dba(d) => preprocess_dba(d, optimizer, sample_rate),
        SpeakerConfig::Group(_) => Err(AutoeqError::InvalidConfiguration {
            message: "Group speaker config should not reach stereo sub workflow; use generic path"
                .to_string(),
        }),
    }
}

/// MSO: optimize inter-sub gains/delays, return combined curve + per-sub info
pub(super) fn preprocess_multisub_mso(
    ms: &MultiSubGroup,
    optimizer: &crate::roomeq::types::OptimizerConfig,
    sample_rate: f64,
) -> Result<SubPreprocessResult> {
    info!("  MSO optimization for {} subwoofers", ms.subwoofers.len());

    let (result, combined) = multisub::optimize_multisub(&ms.subwoofers, optimizer, sample_rate)
        .map_err(|e| AutoeqError::OptimizationFailed {
            message: format!("MSO optimization failed: {}", e),
        })?;

    info!(
        "  MSO result: gains={:?}, delays={:?}",
        result.gains, result.delays
    );

    // Load individual curves for driver info
    let mut drivers = Vec::new();
    for (i, source) in ms.subwoofers.iter().enumerate() {
        let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: e.to_string(),
        })?;
        drivers.push(SubDriverInfo {
            name: format!("{}_{}", ms.name, i + 1),
            gain: result.gains.get(i).copied().unwrap_or(0.0),
            delay: result.delays.get(i).copied().unwrap_or(0.0),
            inverted: false,
            initial_curve: Some(curve),
        });
    }

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

/// Independent subs: average all sub curves, return combined + per-sub info (zero gains/delays)
pub(super) fn preprocess_multisub_independent(ms: &MultiSubGroup) -> Result<SubPreprocessResult> {
    info!(
        "  Independent sub averaging for {} subwoofers",
        ms.subwoofers.len()
    );

    let mut curves = Vec::new();
    for source in &ms.subwoofers {
        let curve = load_source(source).map_err(|e| AutoeqError::InvalidMeasurement {
            message: e.to_string(),
        })?;
        curves.push(curve);
    }

    // Power summation on the first sub's frequency grid:
    // Convert dB to linear power, sum, convert back to dB.
    // This correctly represents incoherent summation of multiple subs.
    let ref_freq = curves[0].freq.clone();
    let mut sum_power = ndarray::Array1::<f64>::zeros(ref_freq.len());
    for curve in &curves {
        let interp = crate::read::interpolate_log_space(&ref_freq, curve);
        sum_power += &interp.spl.mapv(|db| 10.0_f64.powf(db / 10.0));
    }
    let avg_spl = sum_power.mapv(|p| 10.0 * p.log10());

    let combined = Curve {
        freq: ref_freq,
        spl: avg_spl,
        phase: None,
        ..Default::default()
    };

    let drivers: Vec<SubDriverInfo> = curves
        .into_iter()
        .enumerate()
        .map(|(i, curve)| SubDriverInfo {
            name: format!("{}_{}", ms.name, i + 1),
            gain: 0.0,
            delay: 0.0,
            inverted: false,
            initial_curve: Some(curve),
        })
        .collect();

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

/// Cardioid: simulate combined response from front + delayed/inverted rear sub
pub(super) fn preprocess_cardioid(c: &CardioidConfig) -> Result<SubPreprocessResult> {
    let front_curve = load_source(&c.front).map_err(|e| AutoeqError::InvalidMeasurement {
        message: format!("Cardioid front: {}", e),
    })?;
    let rear_curve = load_source(&c.rear).map_err(|e| AutoeqError::InvalidMeasurement {
        message: format!("Cardioid rear: {}", e),
    })?;

    if !crate::roomeq::frequency_grid::is_valid_frequency_grid(&front_curve.freq)
        || !crate::roomeq::frequency_grid::is_valid_frequency_grid(&rear_curve.freq)
    {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Cardioid preprocessing requires valid frequency grids".to_string(),
        });
    }
    if front_curve.spl.len() != front_curve.freq.len()
        || rear_curve.spl.len() != rear_curve.freq.len()
        || front_curve
            .phase
            .as_ref()
            .is_some_and(|phase| phase.len() != front_curve.freq.len())
        || rear_curve
            .phase
            .as_ref()
            .is_some_and(|phase| phase.len() != rear_curve.freq.len())
    {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Cardioid preprocessing curve arrays must match frequency-grid length"
                .to_string(),
        });
    }
    if !crate::roomeq::frequency_grid::same_frequency_grid(&front_curve.freq, &rear_curve.freq) {
        return Err(AutoeqError::InvalidMeasurement {
            message: "Cardioid preprocessing requires front and rear curves to share the same frequency grid".to_string(),
        });
    }

    let delay_ms = c.separation_meters / 343.0 * 1000.0;
    info!(
        "  Cardioid: separation={:.2}m, delay={:.2}ms",
        c.separation_meters, delay_ms
    );

    // Simulate combined response (complex sum of front + delayed/inverted rear)
    use num_complex::Complex;
    let n_points = front_curve.freq.len();
    let mut combined_spl = ndarray::Array1::zeros(n_points);

    let front_phase_zeros = ndarray::Array1::zeros(n_points);
    let rear_phase_zeros = ndarray::Array1::zeros(n_points);
    let front_phase = front_curve.phase.as_ref().unwrap_or(&front_phase_zeros);
    let rear_phase = rear_curve.phase.as_ref().unwrap_or(&rear_phase_zeros);

    for i in 0..n_points {
        let f = front_curve.freq[i];
        let omega = 2.0 * std::f64::consts::PI * f;

        // Front
        let f_mag = 10.0_f64.powf(front_curve.spl[i] / 20.0);
        let f_phi = front_phase[i].to_radians();
        let f_c = Complex::from_polar(f_mag, f_phi);

        // Rear (Inverted + Delayed)
        let r_mag = 10.0_f64.powf(rear_curve.spl[i] / 20.0);
        let r_phi_meas = rear_phase[i].to_radians();
        let delay_s = delay_ms / 1000.0;
        let delay_phi = -omega * delay_s;
        let invert_phi = std::f64::consts::PI;
        let r_phi_total = r_phi_meas + delay_phi + invert_phi;
        let r_c = Complex::from_polar(r_mag, r_phi_total);

        let sum = f_c + r_c;
        combined_spl[i] = 20.0 * sum.norm().log10();
    }

    let combined = Curve {
        freq: front_curve.freq.clone(),
        spl: combined_spl,
        phase: None,
        ..Default::default()
    };

    let drivers = vec![
        SubDriverInfo {
            name: "Front Sub".to_string(),
            gain: 0.0,
            delay: 0.0,
            inverted: false,
            initial_curve: Some(front_curve),
        },
        SubDriverInfo {
            name: "Rear Sub".to_string(),
            gain: 0.0,
            delay: delay_ms,
            inverted: true,
            initial_curve: Some(rear_curve),
        },
    ];

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

/// DBA: run DBA optimization, return combined curve + front/rear driver info
pub(super) fn preprocess_dba(
    d: &DBAConfig,
    optimizer: &crate::roomeq::types::OptimizerConfig,
    sample_rate: f64,
) -> Result<SubPreprocessResult> {
    info!("  DBA optimization");

    let (result, combined) = dba::optimize_dba(d, optimizer, sample_rate).map_err(|e| {
        AutoeqError::OptimizationFailed {
            message: format!("DBA optimization failed: {}", e),
        }
    })?;

    info!(
        "  DBA result: gains={:?}, delays={:?}",
        result.gains, result.delays
    );

    // Load front and rear array responses for display
    let front_curve =
        dba::sum_array_response(&d.front).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("DBA front array: {}", e),
        })?;
    let rear_curve =
        dba::sum_array_response(&d.rear).map_err(|e| AutoeqError::InvalidMeasurement {
            message: format!("DBA rear array: {}", e),
        })?;

    let drivers = vec![
        SubDriverInfo {
            name: "Front Array".to_string(),
            gain: result.gains.first().copied().unwrap_or(0.0),
            delay: result.delays.first().copied().unwrap_or(0.0),
            inverted: false,
            initial_curve: Some(front_curve),
        },
        SubDriverInfo {
            name: "Rear Array".to_string(),
            gain: result.gains.get(1).copied().unwrap_or(0.0),
            delay: result.delays.get(1).copied().unwrap_or(0.0),
            inverted: true,
            initial_curve: Some(rear_curve),
        },
    ];

    Ok(SubPreprocessResult {
        combined_curve: combined,
        drivers: Some(drivers),
    })
}

#[derive(Debug, Clone)]
struct GroupCrossoverPlan {
    crossover_type: String,
    frequency_hz: f64,
    configured_hz: f64,
    frequency_range: Option<(f64, f64)>,
}

#[derive(Debug, Clone)]
struct BassManagementJointGroupInput {
    group_id: String,
    roles: Vec<String>,
    plan: GroupCrossoverPlan,
    virtual_main: Curve,
    phase_available: bool,
    advisories: Vec<String>,
}

pub(super) fn grouped_home_cinema_roles(main_roles: &[String]) -> BTreeMap<String, Vec<String>> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for role in main_roles {
        let role_id = crate::roomeq::home_cinema::group_id_for_role(
            crate::roomeq::home_cinema::role_for_channel(role),
        );
        groups
            .entry(role_id.to_string())
            .or_default()
            .push(role.clone());
    }
    groups
}

fn group_crossover_plan(
    config: &RoomConfig,
    fallback: &CrossoverConfig,
    group_id: &str,
) -> Result<GroupCrossoverPlan> {
    let selected = config
        .system
        .as_ref()
        .and_then(|system| system.bass_management.as_ref())
        .and_then(|bm| bm.group_crossovers.get(group_id))
        .and_then(|key| {
            config
                .crossovers
                .as_ref()
                .and_then(|crossovers| crossovers.get(key))
        })
        .unwrap_or(fallback);

    let (min_hz, max_hz, configured_hz) = if let Some(freq) = selected.frequency {
        (freq, freq, freq)
    } else if let Some((min, max)) = selected.frequency_range {
        (min, max, (min.max(1.0) * max.max(1.0)).sqrt())
    } else {
        return Err(AutoeqError::InvalidConfiguration {
            message: format!(
                "Bass-management crossover for group '{group_id}' requires 'frequency' or 'frequency_range'"
            ),
        });
    };

    Ok(GroupCrossoverPlan {
        crossover_type: selected.crossover_type.clone(),
        frequency_hz: configured_hz,
        configured_hz,
        frequency_range: (min_hz != max_hz).then_some((min_hz, max_hz)),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn optimize_home_cinema_group_crossovers(
    config: &RoomConfig,
    main_roles: &[String],
    aligned_curves: &HashMap<String, Curve>,
    aligned_pre_eq_curves: &HashMap<String, Curve>,
    sub_role: &str,
    fallback_crossover: &CrossoverConfig,
    sample_rate: f64,
    bass_management: Option<&crate::roomeq::home_cinema::EffectiveBassManagement>,
) -> Result<BTreeMap<String, crate::roomeq::home_cinema::BassManagementGroupReport>> {
    let mut reports = BTreeMap::new();
    let mut joint_inputs = Vec::new();
    let sub_curve = &aligned_pre_eq_curves[sub_role];

    for (group_id, roles) in grouped_home_cinema_roles(main_roles) {
        let mut advisories = Vec::new();
        let plan = group_crossover_plan(config, fallback_crossover, &group_id)?;
        let main_refs: Vec<&Curve> = roles
            .iter()
            .map(|role| &aligned_pre_eq_curves[role])
            .collect();
        let mut measured_refs: Vec<&Curve> =
            roles.iter().map(|role| &aligned_curves[role]).collect();
        measured_refs.push(&aligned_curves[sub_role]);
        let mut phase_refs = main_refs.clone();
        phase_refs.push(sub_curve);
        let measured_phase_available = all_curves_have_usable_phase(&measured_refs);
        let shared_grid_available = all_curves_share_frequency_grid(&measured_refs)
            && all_curves_share_frequency_grid(&phase_refs);
        let phase_available = measured_phase_available && shared_grid_available;

        if !measured_phase_available {
            advisories.push("missing_phase_group_crossover_alignment_skipped".to_string());
        } else if !shared_grid_available {
            advisories
                .push("frequency_grid_mismatch_group_crossover_alignment_skipped".to_string());
        }

        let virtual_main = if phase_available {
            complex_sum_mains(&main_refs)
        } else {
            average_mains_magnitude(&main_refs)
        };
        joint_inputs.push(BassManagementJointGroupInput {
            group_id: group_id.clone(),
            roles: roles.clone(),
            plan: plan.clone(),
            virtual_main: virtual_main.clone(),
            phase_available,
            advisories: advisories.clone(),
        });
        let selected_type = select_bass_management_crossover_type(
            &plan.crossover_type,
            &virtual_main,
            sub_curve,
            plan.frequency_hz,
            sample_rate,
        );
        let selected_type_str = selected_type.as_str();

        let objective_before_curve = predict_bass_management_sum(
            &virtual_main,
            sub_curve,
            selected_type_str,
            plan.frequency_hz,
            sample_rate,
            0.0,
            0.0,
            0.0,
            0.0,
            false,
        );
        let objective_before =
            bass_management_objective(objective_before_curve.as_ref(), plan.frequency_hz);

        let mut final_freq = plan.frequency_hz;
        let mut main_delay_ms = 0.0;
        let mut bass_delay_ms = 0.0;
        let mut polarity_inverted = false;
        let mut trim_db = 0.0;
        let mut objective_after = objective_before;

        if phase_available {
            let crossover_type_enum: crate::loss::CrossoverType = selected_type_str
                .parse()
                .map_err(|e: String| AutoeqError::InvalidConfiguration { message: e })?;
            let fixed_freqs = plan
                .frequency_range
                .is_none()
                .then_some(vec![plan.frequency_hz]);
            let mut xo_optimizer_config = config.optimizer.clone();
            xo_optimizer_config.min_db = 0.0;
            xo_optimizer_config.max_db = 0.0;
            let (xo_gains, xo_delays, xo_freqs, _, inversions) = crossover::optimize_crossover(
                vec![virtual_main.clone(), sub_curve.clone()],
                crossover_type_enum,
                sample_rate,
                &xo_optimizer_config,
                fixed_freqs,
                plan.frequency_range,
            )
            .map_err(|e| AutoeqError::OptimizationFailed {
                message: e.to_string(),
            })?;

            final_freq = xo_freqs.first().copied().unwrap_or(plan.frequency_hz);
            let (main_delay, bass_delay) = normalize_crossover_delays(
                xo_delays.first().copied().unwrap_or(0.0),
                xo_delays.get(1).copied().unwrap_or(0.0),
            );
            main_delay_ms = main_delay;
            bass_delay_ms = bass_delay;
            polarity_inverted = inversions.get(1).copied().unwrap_or(false);

            let hp = create_crossover_filters(selected_type_str, final_freq, sample_rate, false);
            let lp = create_crossover_filters(selected_type_str, final_freq, sample_rate, true);
            let apply = |curve: &Curve, filters: &[Biquad], gain: f64, delay: f64, invert: bool| {
                let resp =
                    response::compute_peq_complex_response(filters, &curve.freq, sample_rate);
                let mut c = response::apply_complex_response(curve, &resp);
                for spl in c.spl.iter_mut() {
                    *spl += gain;
                }
                apply_delay_and_polarity_to_curve(&c, delay, invert)
            };
            let main_post = apply(
                &virtual_main,
                &hp,
                xo_gains.first().copied().unwrap_or(0.0),
                main_delay_ms,
                false,
            );
            let sub_post = apply(
                sub_curve,
                &lp,
                xo_gains.get(1).copied().unwrap_or(0.0),
                bass_delay_ms,
                polarity_inverted,
            );
            let main_freqs: Vec<f32> = main_post.freq.iter().map(|&f| f as f32).collect();
            let main_spl: Vec<f32> = main_post.spl.iter().map(|&s| s as f32).collect();
            let sub_freqs: Vec<f32> = sub_post.freq.iter().map(|&f| f as f32).collect();
            let sub_spl: Vec<f32> = sub_post.spl.iter().map(|&s| s as f32).collect();
            let main_mean =
                compute_average_response(&main_freqs, &main_spl, Some((final_freq as f32, 2000.0)))
                    as f64;
            let sub_mean =
                compute_average_response(&sub_freqs, &sub_spl, Some((20.0, final_freq as f32)))
                    as f64;
            let requested_trim = xo_gains.get(1).copied().unwrap_or(0.0) + main_mean - sub_mean;
            let (limited_trim, gain_limited) =
                crate::roomeq::home_cinema::limited_sub_gain(requested_trim, bass_management);
            trim_db = limited_trim;
            if gain_limited {
                advisories.push("group_sub_trim_limited_for_headroom".to_string());
            }

            let objective_after_curve = predict_bass_management_sum(
                &virtual_main,
                sub_curve,
                selected_type_str,
                final_freq,
                sample_rate,
                xo_gains.first().copied().unwrap_or(0.0),
                trim_db,
                main_delay_ms,
                bass_delay_ms,
                polarity_inverted,
            );
            objective_after = bass_management_objective(objective_after_curve.as_ref(), final_freq);
            if objective_after >= objective_before {
                advisories.push("group_optimizer_no_improvement".to_string());
                final_freq = plan.frequency_hz;
                main_delay_ms = 0.0;
                bass_delay_ms = 0.0;
                polarity_inverted = false;
                trim_db = 0.0;
                objective_after = objective_before;
            }
        }

        if advisories.is_empty() {
            advisories.push("ok".to_string());
        }
        reports.insert(
            group_id.clone(),
            crate::roomeq::home_cinema::BassManagementGroupReport {
                group_id,
                roles,
                crossover_type: selected_type_str.to_string(),
                selected_crossover_hz: Some(final_freq),
                configured_crossover_hz: Some(plan.configured_hz),
                main_delay_ms,
                bass_route_delay_ms: bass_delay_ms,
                polarity_inverted,
                trim_db,
                objective_before,
                objective_after,
                advisories,
            },
        );
    }

    if let Some(joint_reports) = optimize_home_cinema_joint_group_crossovers(
        config,
        &joint_inputs,
        &reports,
        sub_curve,
        sample_rate,
    ) {
        reports = joint_reports;
    }

    Ok(reports)
}

fn optimize_home_cinema_joint_group_crossovers(
    config: &RoomConfig,
    inputs: &[BassManagementJointGroupInput],
    current_reports: &BTreeMap<String, crate::roomeq::home_cinema::BassManagementGroupReport>,
    sub_curve: &Curve,
    sample_rate: f64,
) -> Option<BTreeMap<String, crate::roomeq::home_cinema::BassManagementGroupReport>> {
    let optimizable: Vec<&BassManagementJointGroupInput> = inputs
        .iter()
        .filter(|input| input.phase_available)
        .collect();
    if optimizable.is_empty() {
        return None;
    }

    let mut lower_bounds = Vec::new();
    let mut upper_bounds = Vec::new();
    let mut initial = Vec::new();
    let mut type_candidates = Vec::new();

    for input in &optimizable {
        let candidates: Vec<String> =
            bass_management_crossover_type_candidates(&input.plan.crossover_type)
                .into_iter()
                .filter(|candidate| candidate.parse::<crate::loss::CrossoverType>().is_ok())
                .collect();
        let candidates = if candidates.is_empty() {
            vec!["LR24".to_string()]
        } else {
            candidates
        };
        let current_report = current_reports.get(&input.group_id);
        let selected_type = current_report
            .map(|report| report.crossover_type.clone())
            .unwrap_or_else(|| {
                select_bass_management_crossover_type(
                    &input.plan.crossover_type,
                    &input.virtual_main,
                    sub_curve,
                    input.plan.frequency_hz,
                    sample_rate,
                )
            });
        let initial_type_idx = candidates
            .iter()
            .position(|candidate| candidate == &selected_type)
            .unwrap_or(0) as f64;
        type_candidates.push(candidates);

        let (min_freq, max_freq) = input
            .plan
            .frequency_range
            .unwrap_or((input.plan.frequency_hz, input.plan.frequency_hz));
        lower_bounds.extend_from_slice(&[min_freq, 0.0, 0.0, 0.0, 0.0, config.optimizer.min_db]);
        upper_bounds.extend_from_slice(&[
            max_freq,
            (type_candidates.last().unwrap().len().saturating_sub(1)) as f64,
            20.0,
            20.0,
            1.0,
            config
                .system
                .as_ref()
                .and_then(|system| system.bass_management.as_ref())
                .map(|bm| bm.max_sub_boost_db.max(0.0))
                .unwrap_or(config.optimizer.max_db.max(0.0)),
        ]);
        initial.extend_from_slice(&[
            current_report
                .and_then(|report| report.selected_crossover_hz)
                .unwrap_or(input.plan.frequency_hz),
            initial_type_idx,
            current_report
                .map(|report| report.main_delay_ms)
                .unwrap_or(0.0),
            current_report
                .map(|report| report.bass_route_delay_ms)
                .unwrap_or(0.0),
            current_report
                .map(|report| f64::from(report.polarity_inverted))
                .unwrap_or(0.0),
            current_report.map(|report| report.trim_db).unwrap_or(0.0),
        ]);
    }

    let objective = |params: &[f64]| -> f64 {
        let mut total = 0.0;
        let mut trim_power = 0.0;
        for (idx, input) in optimizable.iter().enumerate() {
            let base = idx * 6;
            let freq = params[base].clamp(lower_bounds[base], upper_bounds[base]);
            let candidates = &type_candidates[idx];
            let type_idx = params[base + 1]
                .round()
                .clamp(0.0, (candidates.len().saturating_sub(1)) as f64)
                as usize;
            let xover_type = &candidates[type_idx];
            let main_delay = params[base + 2].clamp(0.0, 20.0);
            let bass_delay = params[base + 3].clamp(0.0, 20.0);
            let inverted = params[base + 4] >= 0.5;
            let trim = params[base + 5].clamp(lower_bounds[base + 5], upper_bounds[base + 5]);
            let predicted = predict_bass_management_sum(
                &input.virtual_main,
                sub_curve,
                xover_type,
                freq,
                sample_rate,
                0.0,
                trim,
                main_delay,
                bass_delay,
                inverted,
            );
            let Some(group_loss) = bass_management_objective(predicted.as_ref(), freq) else {
                return 1.0e12;
            };
            total += group_loss;
            trim_power += 10.0_f64.powf(trim / 10.0);
        }

        // Soft shared-bus headroom pressure: keep the DE from winning a small
        // crossover smoothness gain by asking all groups to boost the sub bus.
        let trim_power_db = 10.0 * trim_power.max(1e-12).log10();
        let allowed = config
            .system
            .as_ref()
            .and_then(|system| system.bass_management.as_ref())
            .map(|bm| bm.headroom_margin_db)
            .unwrap_or(6.0);
        let headroom_excess = (trim_power_db - allowed).max(0.0);
        total + headroom_excess * headroom_excess * 2.0
    };

    let baseline = optimizable
        .iter()
        .filter_map(|input| {
            current_reports
                .get(&input.group_id)
                .and_then(|report| report.objective_after.or(report.objective_before))
        })
        .sum::<f64>();
    let baseline = if baseline.is_finite() && baseline > 0.0 {
        baseline
    } else {
        objective(&initial)
    };
    let (best, best_score) = differential_evolution_minimize(
        &lower_bounds,
        &upper_bounds,
        &initial,
        &objective,
        config.optimizer.population,
        config.optimizer.max_iter,
        config.optimizer.seed.unwrap_or(0x514_ba55),
    );
    if best_score >= baseline - 1.0e-6 {
        return None;
    }

    let mut reports = BTreeMap::new();
    for input in inputs {
        if !input.phase_available {
            let mut advisories = input.advisories.clone();
            if advisories.is_empty() {
                advisories.push("phase_unavailable_joint_group_skipped".to_string());
            }
            reports.insert(
                input.group_id.clone(),
                crate::roomeq::home_cinema::BassManagementGroupReport {
                    group_id: input.group_id.clone(),
                    roles: input.roles.clone(),
                    crossover_type: input.plan.crossover_type.clone(),
                    selected_crossover_hz: Some(input.plan.frequency_hz),
                    configured_crossover_hz: Some(input.plan.configured_hz),
                    main_delay_ms: 0.0,
                    bass_route_delay_ms: 0.0,
                    polarity_inverted: false,
                    trim_db: 0.0,
                    objective_before: None,
                    objective_after: None,
                    advisories,
                },
            );
        }
    }

    let mut decoded = Vec::new();
    for (idx, input) in optimizable.iter().enumerate() {
        let base = idx * 6;
        let freq = best[base].clamp(lower_bounds[base], upper_bounds[base]);
        let candidates = &type_candidates[idx];
        let type_idx = best[base + 1]
            .round()
            .clamp(0.0, (candidates.len().saturating_sub(1)) as f64)
            as usize;
        let main_delay = best[base + 2].clamp(0.0, 20.0);
        let bass_delay = best[base + 3].clamp(0.0, 20.0);
        let inverted = best[base + 4] >= 0.5;
        let trim = best[base + 5].clamp(lower_bounds[base + 5], upper_bounds[base + 5]);
        decoded.push((
            idx,
            input,
            freq,
            candidates[type_idx].clone(),
            main_delay,
            bass_delay,
            inverted,
            trim,
        ));
    }

    let min_delay = if decoded
        .iter()
        .flat_map(|(_, _, _, _, main_delay, bass_delay, _, _)| [*main_delay, *bass_delay])
        .fold(f64::INFINITY, f64::min)
        .is_finite()
    {
        {
            decoded
                .iter()
                .flat_map(|(_, _, _, _, main_delay, bass_delay, _, _)| [*main_delay, *bass_delay])
                .fold(f64::INFINITY, f64::min)
        }
    } else {
        0.0
    };

    for (_, input, freq, xover_type, main_delay, bass_delay, inverted, trim) in decoded {
        let objective_before_curve = predict_bass_management_sum(
            &input.virtual_main,
            sub_curve,
            &xover_type,
            input.plan.frequency_hz,
            sample_rate,
            0.0,
            0.0,
            0.0,
            0.0,
            false,
        );
        let objective_after_curve = predict_bass_management_sum(
            &input.virtual_main,
            sub_curve,
            &xover_type,
            freq,
            sample_rate,
            0.0,
            trim,
            main_delay - min_delay,
            bass_delay - min_delay,
            inverted,
        );
        let mut advisories = input.advisories.clone();
        advisories.push("joint_de_optimized".to_string());
        reports.insert(
            input.group_id.clone(),
            crate::roomeq::home_cinema::BassManagementGroupReport {
                group_id: input.group_id.clone(),
                roles: input.roles.clone(),
                crossover_type: xover_type,
                selected_crossover_hz: Some(freq),
                configured_crossover_hz: Some(input.plan.configured_hz),
                main_delay_ms: main_delay - min_delay,
                bass_route_delay_ms: bass_delay - min_delay,
                polarity_inverted: inverted,
                trim_db: trim,
                objective_before: bass_management_objective(
                    objective_before_curve.as_ref(),
                    input.plan.frequency_hz,
                ),
                objective_after: bass_management_objective(objective_after_curve.as_ref(), freq),
                advisories,
            },
        );
    }

    Some(reports)
}

pub(super) fn differential_evolution_minimize<F>(
    lower_bounds: &[f64],
    upper_bounds: &[f64],
    initial: &[f64],
    objective: &F,
    requested_population: usize,
    requested_evals: usize,
    seed: u64,
) -> (Vec<f64>, f64)
where
    F: Fn(&[f64]) -> f64,
{
    let dims = initial.len();
    let population_size = requested_population.max(dims * 4).clamp(12, 96);
    let max_evals = requested_evals
        .max(population_size * 4)
        .clamp(population_size, 2_000);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut population = Vec::with_capacity(population_size);
    population.push(initial.to_vec());
    for _ in 1..population_size {
        population.push(
            lower_bounds
                .iter()
                .zip(upper_bounds.iter())
                .map(|(lo, hi)| {
                    if (*hi - *lo).abs() <= f64::EPSILON {
                        *lo
                    } else {
                        rng.random_range(*lo..=*hi)
                    }
                })
                .collect::<Vec<_>>(),
        );
    }
    let mut scores: Vec<f64> = population
        .iter()
        .map(|candidate| objective(candidate))
        .collect();
    let mut evals = population_size;
    let mut best_idx = scores
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    while evals < max_evals {
        for target_idx in 0..population_size {
            if evals >= max_evals {
                break;
            }
            let mut a;
            let mut b;
            let mut c;
            loop {
                a = rng.random_range(0..population_size);
                if a != target_idx {
                    break;
                }
            }
            loop {
                b = rng.random_range(0..population_size);
                if b != target_idx && b != a {
                    break;
                }
            }
            loop {
                c = rng.random_range(0..population_size);
                if c != target_idx && c != a && c != b {
                    break;
                }
            }
            let forced_dim = rng.random_range(0..dims);
            let mut trial = population[target_idx].clone();
            for dim in 0..dims {
                if dim == forced_dim || rng.random::<f64>() < 0.9 {
                    let value =
                        population[a][dim] + 0.7 * (population[b][dim] - population[c][dim]);
                    trial[dim] = value.clamp(lower_bounds[dim], upper_bounds[dim]);
                }
            }
            let trial_score = objective(&trial);
            evals += 1;
            if trial_score < scores[target_idx] {
                population[target_idx] = trial;
                scores[target_idx] = trial_score;
                if trial_score < scores[best_idx] {
                    best_idx = target_idx;
                }
            }
        }
    }

    (population[best_idx].clone(), scores[best_idx])
}

pub(super) fn bass_management_sub_output_results(
    fallback_role: &str,
    drivers: Option<&[SubDriverInfo]>,
    shared_gain_db: f64,
    strategy: &SubwooferStrategy,
) -> Vec<crate::roomeq::home_cinema::BassManagementSubOutputReport> {
    let strategy_source = match strategy {
        SubwooferStrategy::Single => "single",
        SubwooferStrategy::Mso => "mso",
        SubwooferStrategy::Dba => "dba",
    };
    let Some(drivers) = drivers else {
        return vec![crate::roomeq::home_cinema::BassManagementSubOutputReport {
            output_role: fallback_role.to_string(),
            gain_db: shared_gain_db,
            delay_ms: 0.0,
            polarity_inverted: false,
            strategy_source: strategy_source.to_string(),
            headroom_contribution_db: shared_gain_db,
        }];
    };

    drivers
        .iter()
        .map(
            |driver| crate::roomeq::home_cinema::BassManagementSubOutputReport {
                output_role: driver.name.clone(),
                gain_db: driver.gain + shared_gain_db,
                delay_ms: driver.delay,
                polarity_inverted: driver.inverted,
                strategy_source: if matches!(strategy, SubwooferStrategy::Dba) {
                    if driver.inverted {
                        "dba_rear".to_string()
                    } else {
                        "dba_front".to_string()
                    }
                } else {
                    strategy_source.to_string()
                },
                headroom_contribution_db: driver.gain + shared_gain_db,
            },
        )
        .collect()
}

pub(super) fn limit_bass_management_sub_output_gains(
    sub_outputs: &mut [crate::roomeq::home_cinema::BassManagementSubOutputReport],
    bass_management: Option<&crate::roomeq::home_cinema::EffectiveBassManagement>,
) -> bool {
    let Some(bm) = bass_management else {
        return false;
    };
    let max_boost = bm.config.max_sub_boost_db.max(0.0);
    let mut limited = false;
    for output in sub_outputs {
        if output.gain_db > max_boost {
            output.gain_db = max_boost;
            output.headroom_contribution_db = output.gain_db;
            limited = true;
        }
    }
    limited
}

#[allow(dead_code)]
pub(super) fn refine_bass_management_sub_outputs(
    config: &RoomConfig,
    main_roles: &[String],
    aligned_pre_eq_curves: &HashMap<String, Curve>,
    group_results: &mut BTreeMap<String, crate::roomeq::home_cinema::BassManagementGroupReport>,
    sub_outputs: &mut [crate::roomeq::home_cinema::BassManagementSubOutputReport],
    drivers: Option<&[SubDriverInfo]>,
    sample_rate: f64,
) -> Vec<String> {
    let Some(drivers) = drivers else {
        return Vec::new();
    };
    if drivers.len() != sub_outputs.len() || drivers.is_empty() {
        return vec!["joint_sub_output_skipped_driver_metadata_mismatch".to_string()];
    }
    if drivers.iter().any(|driver| {
        driver
            .initial_curve
            .as_ref()
            .map(|curve| !curve_has_usable_phase(curve))
            .unwrap_or(true)
    }) {
        return vec!["joint_sub_output_skipped_missing_phase".to_string()];
    }

    let mut group_inputs = Vec::new();
    for (group_id, roles) in grouped_home_cinema_roles(main_roles) {
        let Some(group) = group_results.get(&group_id) else {
            continue;
        };
        if group.selected_crossover_hz.is_none() {
            continue;
        }
        let main_refs: Vec<&Curve> = roles
            .iter()
            .filter_map(|role| aligned_pre_eq_curves.get(role))
            .collect();
        if main_refs.len() != roles.len()
            || !all_curves_have_usable_phase(&main_refs)
            || !all_curves_share_frequency_grid(&main_refs)
        {
            continue;
        }
        group_inputs.push((group_id, complex_sum_mains(&main_refs)));
    }
    if group_inputs.is_empty() {
        return vec!["joint_sub_output_skipped_no_phase_valid_groups".to_string()];
    }

    let mut lower_bounds = Vec::new();
    let mut upper_bounds = Vec::new();
    let mut initial = Vec::new();
    for output in sub_outputs.iter() {
        let is_dba_front = output.strategy_source == "dba_front";
        let is_dba_rear = output.strategy_source == "dba_rear";
        if is_dba_front {
            lower_bounds.extend_from_slice(&[output.gain_db, 0.0, 0.0]);
            upper_bounds.extend_from_slice(&[output.gain_db, 0.001, 0.0]);
        } else if is_dba_rear {
            lower_bounds.extend_from_slice(&[config.optimizer.min_db.min(-30.0), 0.0, 1.0]);
            upper_bounds.extend_from_slice(&[0.0, 100.0, 1.0]);
        } else {
            let gain_span = config.optimizer.max_db.max(6.0);
            lower_bounds.push(output.gain_db - gain_span);
            upper_bounds.push(output.gain_db + gain_span);
            lower_bounds.push(0.0);
            upper_bounds.push(20.0);
            lower_bounds.push(0.0);
            upper_bounds.push(1.0);
        }
        initial.extend_from_slice(&[
            output.gain_db,
            output.delay_ms.max(0.0),
            f64::from(output.polarity_inverted),
        ]);
    }

    let decode =
        |params: &[f64]| -> Vec<crate::roomeq::home_cinema::BassManagementSubOutputReport> {
            let min_delay = params
                .chunks_exact(3)
                .map(|chunk| chunk[1].max(0.0))
                .fold(f64::INFINITY, f64::min);
            let min_delay = if min_delay.is_finite() {
                min_delay
            } else {
                0.0
            };
            sub_outputs
                .iter()
                .enumerate()
                .map(|(idx, output)| {
                    let base = idx * 3;
                    let gain = params[base].clamp(lower_bounds[base], upper_bounds[base]);
                    let delay = params[base + 1]
                        .clamp(lower_bounds[base + 1], upper_bounds[base + 1])
                        - min_delay;
                    let polarity = params[base + 2]
                        .round()
                        .clamp(lower_bounds[base + 2], upper_bounds[base + 2])
                        >= 0.5;
                    crate::roomeq::home_cinema::BassManagementSubOutputReport {
                        output_role: output.output_role.clone(),
                        gain_db: gain,
                        delay_ms: delay.max(0.0),
                        polarity_inverted: polarity,
                        strategy_source: output.strategy_source.clone(),
                        headroom_contribution_db: gain,
                    }
                })
                .collect()
        };

    let objective = |params: &[f64]| -> f64 {
        let decoded = decode(params);
        let mut total = 0.0;
        for (group_id, virtual_main) in &group_inputs {
            let Some(group) = group_results.get(group_id) else {
                continue;
            };
            let Some(freq) = group.selected_crossover_hz else {
                continue;
            };
            let Some(virtual_sub) =
                sum_sub_output_responses_on_grid(&virtual_main.freq, drivers, &decoded)
            else {
                return 1.0e12;
            };
            let predicted = predict_bass_management_sum(
                virtual_main,
                &virtual_sub,
                &group.crossover_type,
                freq,
                sample_rate,
                0.0,
                group.trim_db,
                group.main_delay_ms,
                group.bass_route_delay_ms,
                group.polarity_inverted,
            );
            let Some(loss) = bass_management_objective(predicted.as_ref(), freq) else {
                return 1.0e12;
            };
            total += loss;
        }

        let gain_power = decoded
            .iter()
            .map(|output| 10.0_f64.powf(output.gain_db / 10.0))
            .sum::<f64>();
        let gain_power_db = 10.0 * gain_power.max(1e-12).log10();
        let allowed = config
            .system
            .as_ref()
            .and_then(|system| system.bass_management.as_ref())
            .map(|bm| bm.headroom_margin_db)
            .unwrap_or(6.0);
        let headroom_excess = (gain_power_db - allowed).max(0.0);
        total + headroom_excess * headroom_excess * 2.0
    };

    let baseline = objective(&initial);
    let (best, best_score) = differential_evolution_minimize(
        &lower_bounds,
        &upper_bounds,
        &initial,
        &objective,
        config.optimizer.population,
        config.optimizer.max_iter,
        config.optimizer.seed.unwrap_or(0x5ab_014),
    );
    if best_score >= baseline - 1.0e-6 {
        return vec!["joint_sub_output_no_improvement".to_string()];
    }

    let decoded = decode(&best);
    for (target, optimized) in sub_outputs.iter_mut().zip(decoded) {
        *target = optimized;
    }

    for (group_id, virtual_main) in group_inputs {
        if let Some(group) = group_results.get_mut(&group_id)
            && let Some(freq) = group.selected_crossover_hz
            && let Some(virtual_sub) =
                sum_sub_output_responses_on_grid(&virtual_main.freq, drivers, sub_outputs)
        {
            let predicted = predict_bass_management_sum(
                &virtual_main,
                &virtual_sub,
                &group.crossover_type,
                freq,
                sample_rate,
                0.0,
                group.trim_db,
                group.main_delay_ms,
                group.bass_route_delay_ms,
                group.polarity_inverted,
            );
            group.objective_after = bass_management_objective(predicted.as_ref(), freq);
            group.advisories.retain(|advisory| {
                advisory != "ok" && advisory != "joint_sub_output_no_improvement"
            });
            group
                .advisories
                .push("joint_sub_output_de_optimized".to_string());
        }
    }

    vec!["joint_sub_output_de_optimized".to_string()]
}

#[allow(clippy::too_many_arguments)]
pub(super) fn optimize_bass_management_joint_solution(
    config: &RoomConfig,
    main_roles: &[String],
    aligned_pre_eq_curves: &HashMap<String, Curve>,
    group_results: &mut BTreeMap<String, crate::roomeq::home_cinema::BassManagementGroupReport>,
    sub_outputs: &mut [crate::roomeq::home_cinema::BassManagementSubOutputReport],
    drivers: Option<&[SubDriverInfo]>,
    sub_role: &str,
    sample_rate: f64,
) -> Vec<String> {
    let driver_inputs = if let Some(drivers) = drivers {
        drivers.to_vec()
    } else {
        vec![SubDriverInfo {
            name: sub_role.to_string(),
            gain: 0.0,
            delay: 0.0,
            inverted: false,
            initial_curve: aligned_pre_eq_curves.get(sub_role).cloned(),
        }]
    };
    if driver_inputs.len() != sub_outputs.len() || driver_inputs.is_empty() {
        return vec!["joint_route_optimizer_skipped_driver_metadata_mismatch".to_string()];
    }
    if driver_inputs.iter().any(|driver| {
        driver
            .initial_curve
            .as_ref()
            .map(|curve| !curve_has_usable_phase(curve))
            .unwrap_or(true)
    }) {
        return vec!["joint_route_optimizer_skipped_missing_sub_phase".to_string()];
    }

    let mut group_inputs = Vec::new();
    for (group_id, roles) in grouped_home_cinema_roles(main_roles) {
        let Some(group) = group_results.get(&group_id).cloned() else {
            continue;
        };
        if group.selected_crossover_hz.is_none() {
            continue;
        }
        let main_refs: Vec<&Curve> = roles
            .iter()
            .filter_map(|role| aligned_pre_eq_curves.get(role))
            .collect();
        if main_refs.len() != roles.len()
            || !all_curves_have_usable_phase(&main_refs)
            || !all_curves_share_frequency_grid(&main_refs)
        {
            continue;
        }
        group_inputs.push((group_id, roles, group, complex_sum_mains(&main_refs)));
    }
    if group_inputs.is_empty() {
        return vec!["joint_route_optimizer_skipped_no_phase_valid_groups".to_string()];
    }

    let mut lower_bounds = Vec::new();
    let mut upper_bounds = Vec::new();
    let mut initial = Vec::new();
    let mut type_candidates = Vec::new();

    for (group_id, _, group, _) in &group_inputs {
        let candidates = bass_management_crossover_type_candidates(&group.crossover_type)
            .into_iter()
            .filter(|candidate| candidate.parse::<crate::loss::CrossoverType>().is_ok())
            .collect::<Vec<_>>();
        let candidates = if candidates.is_empty() {
            vec![group.crossover_type.clone()]
        } else {
            candidates
        };
        let type_idx = candidates
            .iter()
            .position(|candidate| candidate == &group.crossover_type)
            .unwrap_or(0) as f64;
        let current_freq = group
            .selected_crossover_hz
            .or(group.configured_crossover_hz)
            .unwrap_or(80.0);
        let octave = 2.0_f64.sqrt();
        let role_bounds = if group_id == "height" {
            (60.0, 200.0)
        } else {
            (40.0, 160.0)
        };
        let min_freq = (current_freq / octave).clamp(role_bounds.0, role_bounds.1);
        let max_freq = (current_freq * octave).clamp(min_freq, role_bounds.1);
        type_candidates.push(candidates);
        lower_bounds.extend_from_slice(&[min_freq, 0.0, 0.0, 0.0, 0.0, config.optimizer.min_db]);
        upper_bounds.extend_from_slice(&[
            max_freq,
            (type_candidates.last().unwrap().len().saturating_sub(1)) as f64,
            20.0,
            20.0,
            1.0,
            config
                .system
                .as_ref()
                .and_then(|system| system.bass_management.as_ref())
                .map(|bm| bm.max_sub_boost_db.max(0.0))
                .unwrap_or(config.optimizer.max_db.max(0.0)),
        ]);
        initial.extend_from_slice(&[
            current_freq,
            type_idx,
            group.main_delay_ms.max(0.0),
            group.bass_route_delay_ms.max(0.0),
            f64::from(group.polarity_inverted),
            group.trim_db,
        ]);
    }

    let output_offset = initial.len();
    let max_output_boost = config
        .system
        .as_ref()
        .and_then(|system| system.bass_management.as_ref())
        .map(|bm| bm.max_sub_boost_db.max(0.0))
        .unwrap_or(config.optimizer.max_db.max(0.0));
    for output in sub_outputs.iter() {
        let is_dba_front = output.strategy_source == "dba_front";
        let is_dba_rear = output.strategy_source == "dba_rear";
        if is_dba_front {
            lower_bounds.extend_from_slice(&[output.gain_db, 0.0, 0.0]);
            upper_bounds.extend_from_slice(&[output.gain_db, 0.001, 0.0]);
        } else if is_dba_rear {
            lower_bounds.extend_from_slice(&[config.optimizer.min_db.min(-30.0), 0.0, 1.0]);
            upper_bounds.extend_from_slice(&[0.0, 100.0, 1.0]);
        } else {
            let gain_span = config.optimizer.max_db.max(6.0);
            lower_bounds.extend_from_slice(&[output.gain_db - gain_span, 0.0, 0.0]);
            upper_bounds.extend_from_slice(&[max_output_boost, 20.0, 1.0]);
        }
        initial.extend_from_slice(&[
            output.gain_db,
            output.delay_ms.max(0.0),
            f64::from(output.polarity_inverted),
        ]);
    }

    let decode = |params: &[f64]| {
        let mut groups = Vec::new();
        let mut delays = Vec::new();
        for (idx, (group_id, roles, group, _)) in group_inputs.iter().enumerate() {
            let base = idx * 6;
            let candidates = &type_candidates[idx];
            let type_idx = params[base + 1]
                .round()
                .clamp(0.0, (candidates.len().saturating_sub(1)) as f64)
                as usize;
            let main_delay = params[base + 2].clamp(lower_bounds[base + 2], upper_bounds[base + 2]);
            let bass_delay = params[base + 3].clamp(lower_bounds[base + 3], upper_bounds[base + 3]);
            delays.push(main_delay);
            delays.push(bass_delay);
            groups.push(crate::roomeq::home_cinema::BassManagementGroupReport {
                group_id: group_id.clone(),
                roles: roles.clone(),
                crossover_type: candidates[type_idx].clone(),
                selected_crossover_hz: Some(
                    params[base].clamp(lower_bounds[base], upper_bounds[base]),
                ),
                configured_crossover_hz: group.configured_crossover_hz,
                main_delay_ms: main_delay,
                bass_route_delay_ms: bass_delay,
                polarity_inverted: params[base + 4].round().clamp(0.0, 1.0) >= 0.5,
                trim_db: params[base + 5].clamp(lower_bounds[base + 5], upper_bounds[base + 5]),
                objective_before: group.objective_before,
                objective_after: group.objective_after,
                advisories: group.advisories.clone(),
            });
        }
        let mut outputs = Vec::new();
        for (idx, output) in sub_outputs.iter().enumerate() {
            let base = output_offset + idx * 3;
            let delay = params[base + 1].clamp(lower_bounds[base + 1], upper_bounds[base + 1]);
            delays.push(delay);
            outputs.push(crate::roomeq::home_cinema::BassManagementSubOutputReport {
                output_role: output.output_role.clone(),
                gain_db: params[base].clamp(lower_bounds[base], upper_bounds[base]),
                delay_ms: delay,
                polarity_inverted: params[base + 2]
                    .round()
                    .clamp(lower_bounds[base + 2], upper_bounds[base + 2])
                    >= 0.5,
                strategy_source: output.strategy_source.clone(),
                headroom_contribution_db: params[base]
                    .clamp(lower_bounds[base], upper_bounds[base]),
            });
        }
        let common_delay = delays.into_iter().fold(f64::INFINITY, f64::min);
        let common_delay = if common_delay.is_finite() {
            common_delay
        } else {
            0.0
        };
        for group in &mut groups {
            group.main_delay_ms = (group.main_delay_ms - common_delay).max(0.0);
            group.bass_route_delay_ms = (group.bass_route_delay_ms - common_delay).max(0.0);
        }
        for output in &mut outputs {
            output.delay_ms = (output.delay_ms - common_delay).max(0.0);
        }
        (groups, outputs)
    };

    let objective = |params: &[f64]| -> f64 {
        let (groups, outputs) = decode(params);
        let mut total = 0.0;
        for ((_, _, _, virtual_main), group) in group_inputs.iter().zip(groups.iter()) {
            let Some(freq) = group.selected_crossover_hz else {
                return 1.0e12;
            };
            let Some(virtual_sub) =
                sum_sub_output_responses_on_grid(&virtual_main.freq, &driver_inputs, &outputs)
            else {
                return 1.0e12;
            };
            let predicted = predict_bass_management_sum(
                virtual_main,
                &virtual_sub,
                &group.crossover_type,
                freq,
                sample_rate,
                0.0,
                group.trim_db,
                group.main_delay_ms,
                group.bass_route_delay_ms,
                group.polarity_inverted,
            );
            let Some(loss) = bass_management_objective(predicted.as_ref(), freq) else {
                return 1.0e12;
            };
            total += loss;
        }

        let optimization = joint_bass_management_report_from_parts(&groups, &outputs);
        let graph =
            crate::roomeq::home_cinema::bass_management_routing_graph(config, Some(&optimization));
        if let Some(effective) = crate::roomeq::home_cinema::effective_bass_management(config)
            && let Some(headroom) = crate::roomeq::home_cinema::simulate_bass_bus_headroom(
                graph.as_ref(),
                &effective.config.headroom_model,
                effective.config.headroom_margin_db,
                sample_rate,
            )
        {
            let headroom_excess = (-headroom.margin_db).max(0.0);
            total += headroom_excess * headroom_excess * 2.0;
        }
        total
    };

    let baseline = objective(&initial);
    let (best, best_score) = differential_evolution_minimize(
        &lower_bounds,
        &upper_bounds,
        &initial,
        &objective,
        config.optimizer.population,
        config.optimizer.max_iter,
        config.optimizer.seed.unwrap_or(0x14_ba55),
    );
    if best_score >= baseline - 1.0e-6 {
        return vec!["joint_optimizer_no_improvement".to_string()];
    }

    let (mut decoded_groups, decoded_outputs) = decode(&best);
    for ((_, _, _, virtual_main), group) in group_inputs.iter().zip(decoded_groups.iter_mut()) {
        if let Some(freq) = group.selected_crossover_hz
            && let Some(virtual_sub) = sum_sub_output_responses_on_grid(
                &virtual_main.freq,
                &driver_inputs,
                &decoded_outputs,
            )
        {
            let before = predict_bass_management_sum(
                virtual_main,
                &virtual_sub,
                &group.crossover_type,
                group.configured_crossover_hz.unwrap_or(freq),
                sample_rate,
                0.0,
                0.0,
                0.0,
                0.0,
                false,
            );
            let after = predict_bass_management_sum(
                virtual_main,
                &virtual_sub,
                &group.crossover_type,
                freq,
                sample_rate,
                0.0,
                group.trim_db,
                group.main_delay_ms,
                group.bass_route_delay_ms,
                group.polarity_inverted,
            );
            group.objective_before = bass_management_objective(before.as_ref(), freq);
            group.objective_after = bass_management_objective(after.as_ref(), freq);
        }
        group
            .advisories
            .retain(|advisory| advisory != "ok" && advisory != "joint_optimizer_no_improvement");
        group
            .advisories
            .push("joint_route_de_optimized".to_string());
    }
    for group in decoded_groups {
        group_results.insert(group.group_id.clone(), group);
    }
    for (target, optimized) in sub_outputs.iter_mut().zip(decoded_outputs) {
        *target = optimized;
    }
    vec!["joint_route_de_optimized".to_string()]
}

pub(super) fn joint_bass_management_report_from_parts(
    groups: &[crate::roomeq::home_cinema::BassManagementGroupReport],
    outputs: &[crate::roomeq::home_cinema::BassManagementSubOutputReport],
) -> crate::roomeq::home_cinema::BassManagementOptimizationReport {
    let first_group = groups.first();
    let applied_gain = outputs
        .iter()
        .map(|output| output.gain_db)
        .fold(f64::NEG_INFINITY, f64::max);
    let applied_gain = if applied_gain.is_finite() {
        applied_gain
    } else {
        0.0
    };
    crate::roomeq::home_cinema::BassManagementOptimizationReport {
        applied: true,
        phase_required: true,
        phase_available: true,
        configured_crossover_hz: first_group.and_then(|group| group.configured_crossover_hz),
        optimized_crossover_hz: first_group.and_then(|group| group.selected_crossover_hz),
        crossover_range_hz: None,
        crossover_type: first_group
            .map(|group| group.crossover_type.clone())
            .unwrap_or_else(|| "LR24".to_string()),
        main_delay_ms: first_group.map(|group| group.main_delay_ms).unwrap_or(0.0),
        sub_delay_ms: first_group
            .map(|group| group.bass_route_delay_ms)
            .unwrap_or(0.0),
        relative_sub_delay_ms: first_group
            .map(|group| group.bass_route_delay_ms - group.main_delay_ms)
            .unwrap_or(0.0),
        sub_polarity_inverted: first_group
            .map(|group| group.polarity_inverted)
            .unwrap_or(false),
        requested_sub_gain_db: applied_gain,
        applied_sub_gain_db: applied_gain,
        gain_limited: false,
        estimated_bass_bus_peak_gain_db: None,
        objective_before: groups
            .iter()
            .filter_map(|group| group.objective_before)
            .reduce(|a, b| a + b),
        objective_after: groups
            .iter()
            .filter_map(|group| group.objective_after)
            .reduce(|a, b| a + b),
        group_results: groups.to_vec(),
        sub_output_results: outputs.to_vec(),
        advisories: vec!["joint_route_solution".to_string()],
    }
}

pub(super) fn predict_bass_output_curve_from_routes(
    sub_curve: &Curve,
    graph: &crate::roomeq::home_cinema::BassManagementRoutingGraph,
    output_role: &str,
    sample_rate: f64,
) -> Option<Curve> {
    use num_complex::Complex;

    if !curve_has_usable_phase(sub_curve) {
        return None;
    }
    let phase = sub_curve.phase.as_ref()?;
    let mut complex_sum = vec![Complex::new(0.0, 0.0); sub_curve.freq.len()];
    let mut any_route = false;
    for route in graph.routes.iter().filter(|route| {
        route.destination == output_role
            && (route.route_kind == "redirected_bass_lowpass_to_sub"
                || route.route_kind == "lfe_lowpass_to_sub")
    }) {
        any_route = true;
        let filters = if let Some(freq) = route.low_pass_hz {
            create_crossover_filters(&route.crossover_type, freq, sample_rate, true)
        } else {
            Vec::new()
        };
        let response =
            response::compute_peq_complex_response(&filters, &sub_curve.freq, sample_rate);
        let polarity_phase = if route.polarity_inverted { 180.0 } else { 0.0 };
        for idx in 0..sub_curve.freq.len() {
            let freq_hz = sub_curve.freq[idx];
            let delay_phase = -360.0 * freq_hz * route.delay_ms / 1000.0;
            let mag = 10.0_f64.powf((sub_curve.spl[idx] + route.gain_db) / 20.0);
            let phase_rad = (phase[idx] + delay_phase + polarity_phase).to_radians();
            complex_sum[idx] += Complex::from_polar(mag, phase_rad) * response[idx];
        }
    }
    if !any_route {
        return None;
    }

    let mut spl = ndarray::Array1::<f64>::zeros(sub_curve.freq.len());
    let mut output_phase = ndarray::Array1::<f64>::zeros(sub_curve.freq.len());
    for (idx, value) in complex_sum.iter().enumerate() {
        spl[idx] = 20.0 * value.norm().max(1e-12).log10();
        output_phase[idx] = value.arg().to_degrees();
    }
    Some(Curve {
        freq: sub_curve.freq.clone(),
        spl,
        phase: Some(output_phase),
        ..Default::default()
    })
}

pub(super) fn predict_bass_bus_curve_from_routes(
    reference_curve: &Curve,
    graph: &crate::roomeq::home_cinema::BassManagementRoutingGraph,
    output_base_curves: &HashMap<String, Curve>,
    fallback_curve: &Curve,
    sample_rate: f64,
) -> Option<Curve> {
    use num_complex::Complex;

    if !curve_has_usable_phase(reference_curve) {
        return None;
    }

    let mut complex_sum = vec![Complex::new(0.0, 0.0); reference_curve.freq.len()];
    let mut any_route = false;
    for route in graph.routes.iter().filter(|route| {
        route.route_kind == "redirected_bass_lowpass_to_sub"
            || route.route_kind == "lfe_lowpass_to_sub"
    }) {
        let base_curve = output_base_curves
            .get(&route.destination)
            .unwrap_or(fallback_curve);
        if !curve_has_usable_phase(base_curve) {
            continue;
        }
        let curve = if crate::roomeq::frequency_grid::same_frequency_grid(
            &reference_curve.freq,
            &base_curve.freq,
        ) {
            base_curve.clone()
        } else {
            crate::read::interpolate_log_space(&reference_curve.freq, base_curve)
        };
        let Some(phase) = curve.phase.as_ref() else {
            continue;
        };
        any_route = true;
        let filters = if let Some(freq) = route.low_pass_hz {
            create_crossover_filters(&route.crossover_type, freq, sample_rate, true)
        } else {
            Vec::new()
        };
        let response =
            response::compute_peq_complex_response(&filters, &reference_curve.freq, sample_rate);
        let polarity_phase = if route.polarity_inverted { 180.0 } else { 0.0 };
        for idx in 0..reference_curve.freq.len() {
            let freq_hz = reference_curve.freq[idx];
            let delay_phase = -360.0 * freq_hz * route.delay_ms / 1000.0;
            let mag = 10.0_f64.powf((curve.spl[idx] + route.gain_db) / 20.0);
            let phase_rad = (phase[idx] + delay_phase + polarity_phase).to_radians();
            complex_sum[idx] += Complex::from_polar(mag, phase_rad) * response[idx];
        }
    }
    if !any_route {
        return None;
    }

    let mut spl = ndarray::Array1::<f64>::zeros(reference_curve.freq.len());
    let mut output_phase = ndarray::Array1::<f64>::zeros(reference_curve.freq.len());
    for (idx, value) in complex_sum.iter().enumerate() {
        spl[idx] = 20.0 * value.norm().max(1e-12).log10();
        output_phase[idx] = value.arg().to_degrees();
    }
    Some(Curve {
        freq: reference_curve.freq.clone(),
        spl,
        phase: Some(output_phase),
        ..Default::default()
    })
}

pub(super) fn bass_route_upper_frequency_hz(
    graph: Option<&crate::roomeq::home_cinema::BassManagementRoutingGraph>,
    fallback_hz: f64,
) -> f64 {
    graph
        .and_then(|graph| {
            graph
                .routes
                .iter()
                .filter_map(|route| route.low_pass_hz)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        })
        .unwrap_or(fallback_hz)
}

pub(super) fn representative_bass_route_signature(
    graph: Option<&crate::roomeq::home_cinema::BassManagementRoutingGraph>,
    fallback_type: &str,
    fallback_hz: f64,
) -> (String, f64) {
    graph
        .and_then(|graph| {
            graph
                .routes
                .iter()
                .filter(|route| {
                    route.route_kind == "redirected_bass_lowpass_to_sub"
                        || route.route_kind == "lfe_lowpass_to_sub"
                })
                .filter_map(|route| {
                    route
                        .low_pass_hz
                        .map(|freq| (route.crossover_type.clone(), freq))
                })
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        })
        .unwrap_or_else(|| (fallback_type.to_string(), fallback_hz))
}

pub(super) fn sum_sub_output_responses_on_grid(
    target_freq: &ndarray::Array1<f64>,
    drivers: &[SubDriverInfo],
    outputs: &[crate::roomeq::home_cinema::BassManagementSubOutputReport],
) -> Option<Curve> {
    use num_complex::Complex;
    if drivers.len() != outputs.len() || drivers.is_empty() {
        return None;
    }

    let mut complex_sum = vec![Complex::new(0.0, 0.0); target_freq.len()];
    for (driver, output) in drivers.iter().zip(outputs.iter()) {
        let curve = driver.initial_curve.as_ref()?;
        if !curve_has_usable_phase(curve) {
            return None;
        }
        let interpolated = crate::read::interpolate_log_space(target_freq, curve);
        let phase = interpolated.phase.as_ref()?;
        for idx in 0..target_freq.len() {
            let freq_hz = target_freq[idx];
            let gain_db = output.gain_db;
            let delay_phase = -360.0 * freq_hz * output.delay_ms / 1000.0;
            let polarity_phase = if output.polarity_inverted { 180.0 } else { 0.0 };
            let mag = 10.0_f64.powf((interpolated.spl[idx] + gain_db) / 20.0);
            let phase_rad = (phase[idx] + delay_phase + polarity_phase).to_radians();
            complex_sum[idx] += Complex::from_polar(mag, phase_rad);
        }
    }

    let mut spl = ndarray::Array1::<f64>::zeros(target_freq.len());
    let mut phase = ndarray::Array1::<f64>::zeros(target_freq.len());
    for (idx, value) in complex_sum.iter().enumerate() {
        spl[idx] = 20.0 * value.norm().max(1e-12).log10();
        phase[idx] = value.arg().to_degrees();
    }

    Some(Curve {
        freq: target_freq.clone(),
        spl,
        phase: Some(phase),
        ..Default::default()
    })
}


