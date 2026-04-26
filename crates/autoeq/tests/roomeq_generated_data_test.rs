//! Integration tests using BEM-generated room measurement data
//!
//! These tests load pre-computed BEM simulation data and verify that
//! the roomeq optimizer can improve the simulated frequency responses.

use autoeq::roomeq::{FirConfig, MixedPhaseSerdeConfig, ProcessingMode, RoomConfig, optimize_room};
use std::path::{Path, PathBuf};

/// Get the autoeq crate root (CARGO_MANIFEST_DIR).
/// roomeq test data lives under `<crate>/data_tests/roomeq/`.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Get workspace root (two levels up from CARGO_MANIFEST_DIR = crates/autoeq).
/// Used for output paths under `data_generated/`.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Run roomeq optimization on a generated BEM scenario and verify improvement
fn run_roomeq_on_generated(scenario_name: &str) {
    let config_path = crate_root()
        .join("data_tests/roomeq/generated/bem")
        .join(scenario_name)
        .join("config.json");

    let config_json = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read config for {scenario_name}: {e}"));
    let mut config: RoomConfig = serde_json::from_str(&config_json)
        .unwrap_or_else(|e| panic!("Failed to parse config for {scenario_name}: {e}"));

    // Resolve CSV paths relative to the config file's directory
    if let Some(config_dir) = config_path.parent() {
        config.resolve_paths(config_dir);
    }

    // Reduce iterations for faster tests
    config.optimizer.max_iter = 2000;
    config.optimizer.refine = false;
    // Use fixed seed for reproducible results
    config.optimizer.seed = Some(42);

    let result = optimize_room(&config, 48000.0, None, None)
        .unwrap_or_else(|e| panic!("Optimization failed for {scenario_name}: {e}"));

    // Verify optimization improved the response
    assert!(
        result.combined_post_score < result.combined_pre_score,
        "{scenario_name}: optimization did not improve score: pre={:.4}, post={:.4}",
        result.combined_pre_score,
        result.combined_post_score
    );

    // Verify at least 10% improvement
    let improvement = 1.0 - result.combined_post_score / result.combined_pre_score;
    assert!(
        improvement > 0.10,
        "{scenario_name}: improvement {:.1}% is less than 10% (pre={:.4}, post={:.4})",
        improvement * 100.0,
        result.combined_pre_score,
        result.combined_post_score
    );

    // Verify all channels have EQ results.
    // Sub/LFE channels may legitimately have empty biquads when the "do no harm"
    // guard in the 2.1 workflow discards Post-EQ that would degrade the response.
    let sub_names = ["LFE", "lfe", "sub"];
    for (channel_name, channel_result) in &result.channel_results {
        let is_sub = sub_names
            .iter()
            .any(|s| channel_name.eq_ignore_ascii_case(s))
            || channel_name.to_lowercase().starts_with("sub");
        if !is_sub {
            assert!(
                !channel_result.biquads.is_empty(),
                "{scenario_name}: channel '{channel_name}' has no biquad filters"
            );
        }
        // Allow up to 10% per-channel regression — the optimizer minimizes the
        // combined score across all channels, so individual channels may trade
        // a small regression for a better overall result.
        let max_allowed = channel_result.pre_score * 1.10;
        assert!(
            channel_result.post_score < max_allowed,
            "{scenario_name}: channel '{channel_name}' regressed too much: pre={:.4}, post={:.4} (max={:.4})",
            channel_result.pre_score,
            channel_result.post_score,
            max_allowed
        );
    }

    // Verify DSP chains were generated
    for (channel_name, chain) in &result.channels {
        assert!(
            !chain.plugins.is_empty(),
            "{scenario_name}: channel '{channel_name}' has no plugins in DSP chain"
        );
    }
}

// --- Stereo 2.0 scenarios (no subs) ---

#[test]
fn test_roomeq_small_stereo_2_0() {
    run_roomeq_on_generated("small_stereo_2_0");
}

#[test]
fn test_roomeq_medium_stereo_2_0() {
    run_roomeq_on_generated("medium_stereo_2_0");
}

#[test]
fn test_roomeq_large_stereo_2_0() {
    run_roomeq_on_generated("large_stereo_2_0");
}

// --- 2.1 scenarios (single sub) ---

#[test]
fn test_roomeq_small_stereo_2_1() {
    run_roomeq_on_generated("small_stereo_2_1");
}

#[test]
fn test_roomeq_medium_stereo_2_1() {
    run_roomeq_on_generated("medium_stereo_2_1");
}

#[test]
fn test_roomeq_large_stereo_2_1() {
    run_roomeq_on_generated("large_stereo_2_1");
}

// --- Multi-seat scenarios ---

#[test]
fn test_roomeq_medium_multi_seat() {
    run_roomeq_on_generated("medium_multi_seat");
}

// --- Multi-sub scenarios ---

#[test]
fn test_roomeq_small_multi_sub_2() {
    run_roomeq_on_generated("small_multi_sub_2");
}

#[test]
fn test_roomeq_medium_multi_sub_4() {
    run_roomeq_on_generated("medium_multi_sub_4");
}

#[test]
fn test_roomeq_large_multi_sub_4() {
    run_roomeq_on_generated("large_multi_sub_4");
}

#[test]
fn test_roomeq_large_multi_seat_2_1() {
    run_roomeq_on_generated("large_multi_seat_2_1");
}

#[test]
fn test_roomeq_medium_multi_sub_multi_seat() {
    run_roomeq_on_generated("medium_multi_sub_multi_seat");
}

// ============================================================================
// Multi-mode comparison tests
// ============================================================================

/// Mode configuration for comparison tests
struct ModeConfig {
    name: &'static str,
    processing_mode: ProcessingMode,
    fir: Option<FirConfig>,
    mixed_phase: Option<MixedPhaseSerdeConfig>,
}

/// Cross-mode comparison tolerances — calibrated from empirical results.
///
/// Findings from tightening (3 room sizes, 4 modes):
///
/// **Score ratios**: Before Phase 3, `optimize_stereo_2_0` always called
///   `eq::optimize_channel_eq` regardless of `processing_mode`, so iir / fir /
///   hybrid / mixed_phase produced near-identical results. Phase 3 routes
///   each channel through `process_single_speaker`, which honours
///   `processing_mode` — so modes now genuinely diverge on bass-heavy
///   scenarios. Observed ratios: up to 1.7× on small rooms with a strong
///   sub-100 Hz mode. Limit loosened to 2.0× to reflect this.
///
/// **FR peak diff**: Large (11-20 dB) at specific room mode frequencies.
///   IIR targets narrow modes precisely; FIR spreads correction broadly.
///   Worse in large rooms (more/denser modes). This is fundamental, not a bug.
///   Peak diff is reported for diagnostics but NOT asserted — it's not meaningful.
///
/// **FR RMS diff**: The meaningful metric. Small rooms ~2.5 dB, large rooms
///   ~4.4 dB, edge-case stereo 2.0 after Phase 3 can reach ~5.3 dB because
///   IIR and FIR handle the same modal peak with different filter shapes.
///
/// **Improvement**: All modes achieve 34-58% per-channel, 45-55% combined.
const CROSS_MODE_SCORE_RATIO_LIMIT: f64 = 2.0; // post-Phase-3 calibration
const MIN_IMPROVEMENT_PCT: f64 = 0.25; // require 25% combined improvement
const MAX_CHANNEL_REGRESSION: f64 = 1.02; // max 2% regression per channel
/// RMS dB difference — the meaningful broadband agreement metric.
/// 6 dB allows for stereo 2.0 after Phase 3, where processing_mode now
/// reaches the per-channel pipeline and legitimately diverges mode by mode.
const CROSS_MODE_FR_RMS_DIFF_DB: f64 = 6.0;
/// Peak diff reported but not a hard failure — logged for diagnostics
const CROSS_MODE_FR_PEAK_WARN_DB: f64 = 10.0;

fn all_mode_configs() -> Vec<ModeConfig> {
    vec![
        ModeConfig {
            name: "iir",
            processing_mode: ProcessingMode::LowLatency,
            fir: None,
            mixed_phase: None,
        },
        ModeConfig {
            name: "fir",
            processing_mode: ProcessingMode::PhaseLinear,
            fir: Some(FirConfig {
                taps: 4096,
                phase: "kirkeby".to_string(),
                correct_excess_phase: false,
                phase_smoothing: 0.167,
                pre_ringing: None,
            }),
            mixed_phase: None,
        },
        ModeConfig {
            name: "hybrid",
            processing_mode: ProcessingMode::Hybrid,
            fir: Some(FirConfig {
                taps: 4096,
                phase: "kirkeby".to_string(),
                correct_excess_phase: false,
                phase_smoothing: 0.167,
                pre_ringing: None,
            }),
            mixed_phase: None,
        },
        ModeConfig {
            name: "mixed_phase",
            processing_mode: ProcessingMode::MixedPhase,
            fir: None,
            mixed_phase: Some(MixedPhaseSerdeConfig {
                max_fir_length_ms: 10.0,
                pre_ringing_threshold_db: -30.0,
                min_spatial_depth: 0.5,
                phase_smoothing_octaves: 0.167,
            }),
        },
    ]
}

/// Run roomeq optimization on a BEM scenario with a specific processing mode
fn run_roomeq_with_mode(
    scenario_name: &str,
    mode_config: &ModeConfig,
    output_dir: &Path,
) -> autoeq::roomeq::RoomOptimizationResult {
    let config_path = crate_root()
        .join("data_tests/roomeq/generated/bem")
        .join(scenario_name)
        .join("config.json");

    let config_json = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("Failed to read config for {scenario_name}: {e}"));
    let mut config: RoomConfig = serde_json::from_str(&config_json)
        .unwrap_or_else(|e| panic!("Failed to parse config for {scenario_name}: {e}"));

    if let Some(config_dir) = config_path.parent() {
        config.resolve_paths(config_dir);
    }

    // Override processing mode
    config.optimizer.processing_mode = mode_config.processing_mode.clone();
    config.optimizer.fir = mode_config.fir.clone();
    config.optimizer.mixed_phase = mode_config.mixed_phase.clone();

    // Enable channel matching correction
    config.optimizer.channel_matching = Some(autoeq::roomeq::ChannelMatchingConfig::default());

    // Reduced iterations for speed, fixed seed for reproducibility
    config.optimizer.max_iter = 2000;
    config.optimizer.refine = false;
    config.optimizer.seed = Some(42);

    // FIR/Hybrid need max_freq capped for reasonable tap counts
    if matches!(
        mode_config.processing_mode,
        ProcessingMode::PhaseLinear | ProcessingMode::Hybrid
    ) {
        config.optimizer.max_freq = config.optimizer.max_freq.min(1500.0);
    }

    optimize_room(&config, 48000.0, None, Some(output_dir)).unwrap_or_else(|e| {
        panic!(
            "Optimization failed for {scenario_name} mode={}: {e}",
            mode_config.name
        )
    })
}

/// Compute RMS and max dB difference between two curves in a frequency range
fn curve_diff_stats(
    curve_a: &autoeq::Curve,
    curve_b: &autoeq::Curve,
    freq_lo: f64,
    freq_hi: f64,
) -> (f64, f64, f64) {
    // (rms_diff, max_diff, freq_at_max)
    let mut sum_sq = 0.0;
    let mut count = 0usize;
    let mut max_diff: f64 = 0.0;
    let mut freq_at_max: f64 = 0.0;

    for k in 0..curve_a.freq.len() {
        let f = curve_a.freq[k];
        if f < freq_lo || f > freq_hi {
            continue;
        }
        if let Some(idx_b) = curve_b
            .freq
            .iter()
            .position(|&fb| fb >= f * 0.95 && fb <= f * 1.05)
        {
            let diff = (curve_a.spl[k] - curve_b.spl[idx_b]).abs();
            sum_sq += diff * diff;
            count += 1;
            if diff > max_diff {
                max_diff = diff;
                freq_at_max = f;
            }
        }
    }

    let rms = if count > 0 {
        (sum_sq / count as f64).sqrt()
    } else {
        0.0
    };
    (rms, max_diff, freq_at_max)
}

/// Run all 4 modes on a scenario, assert per-mode quality and cross-mode consistency
fn run_multimode_comparison(scenario_name: &str) {
    let output_base = workspace_root()
        .join("data_generated/roomeq_comparison")
        .join(scenario_name);

    let modes = all_mode_configs();
    let mut results: Vec<(&str, autoeq::roomeq::RoomOptimizationResult)> = Vec::new();
    let mut failures: Vec<String> = Vec::new();

    println!("\n=== {scenario_name}: Multi-mode comparison ===");

    for mode in &modes {
        let mode_dir = output_base.join(mode.name);
        std::fs::create_dir_all(&mode_dir).unwrap();

        let result = run_roomeq_with_mode(scenario_name, mode, &mode_dir);

        let improvement = 1.0 - result.combined_post_score / result.combined_pre_score;
        println!(
            "  {:12} pre={:.4}  post={:.4}  improv={:.1}%  filters={}",
            mode.name,
            result.combined_pre_score,
            result.combined_post_score,
            improvement * 100.0,
            result
                .channel_results
                .values()
                .map(|ch| ch.biquads.len())
                .sum::<usize>(),
        );

        // Per-channel detail
        for (ch_name, ch_result) in &result.channel_results {
            let ch_improv = 1.0 - ch_result.post_score / ch_result.pre_score;
            let fir_len = ch_result.fir_coeffs.as_ref().map_or(0, |c| c.len());
            println!(
                "    {:8} pre={:.4}  post={:.4}  improv={:.1}%  biquads={}  fir_taps={}",
                ch_name,
                ch_result.pre_score,
                ch_result.post_score,
                ch_improv * 100.0,
                ch_result.biquads.len(),
                fir_len,
            );
        }

        // Check: optimization must improve
        if result.combined_post_score >= result.combined_pre_score {
            failures.push(format!(
                "{}/{}: NO improvement (pre={:.4}, post={:.4})",
                scenario_name, mode.name, result.combined_pre_score, result.combined_post_score,
            ));
        }

        // Check: minimum improvement threshold
        if improvement < MIN_IMPROVEMENT_PCT {
            failures.push(format!(
                "{}/{}: improvement {:.1}% < {:.0}% minimum",
                scenario_name,
                mode.name,
                improvement * 100.0,
                MIN_IMPROVEMENT_PCT * 100.0,
            ));
        }

        // Check: no channel regression beyond threshold
        let sub_names = ["LFE", "lfe", "sub"];
        for (ch_name, ch_result) in &result.channel_results {
            let is_sub = sub_names.iter().any(|s| ch_name.eq_ignore_ascii_case(s))
                || ch_name.to_lowercase().starts_with("sub");
            if !is_sub {
                let ratio = ch_result.post_score / ch_result.pre_score;
                if ratio > MAX_CHANNEL_REGRESSION {
                    failures.push(format!(
                        "{}/{}/{}: channel regressed {:.1}% (pre={:.4}, post={:.4}, limit={:.0}%)",
                        scenario_name,
                        mode.name,
                        ch_name,
                        (ratio - 1.0) * 100.0,
                        ch_result.pre_score,
                        ch_result.post_score,
                        (MAX_CHANNEL_REGRESSION - 1.0) * 100.0,
                    ));
                }
            }
        }

        // Save output JSON
        let dsp_output = result.to_dsp_chain_output();
        let json_path = mode_dir.join(format!("{}.json", mode.name));
        let json = serde_json::to_string_pretty(&dsp_output).unwrap();
        std::fs::write(&json_path, json).unwrap();

        results.push((mode.name, result));
    }

    // Cross-mode consistency: score ratio between any pair
    println!("\n  Cross-mode score ratios:");
    for i in 0..results.len() {
        for j in (i + 1)..results.len() {
            let (name_a, res_a) = &results[i];
            let (name_b, res_b) = &results[j];

            let score_a = res_a.combined_post_score;
            let score_b = res_b.combined_post_score;
            let ratio = if score_a > score_b {
                score_a / score_b
            } else {
                score_b / score_a
            };

            let status = if ratio < CROSS_MODE_SCORE_RATIO_LIMIT {
                "OK"
            } else {
                "FAIL"
            };
            println!(
                "    {name_a:12} vs {name_b:12}: ratio={ratio:.3}  ({name_a}={score_a:.4}, {name_b}={score_b:.4})  [{status}]",
            );

            if ratio >= CROSS_MODE_SCORE_RATIO_LIMIT {
                failures.push(format!(
                    "{scenario_name}: score ratio {name_a}/{name_b} = {ratio:.3} >= {CROSS_MODE_SCORE_RATIO_LIMIT} \
                     ({name_a}={score_a:.4}, {name_b}={score_b:.4})",
                ));
            }
        }
    }

    // Cross-mode consistency: final curve dB difference in passband
    println!("\n  Cross-mode FR differences (in optimization range):");
    let channel_names: Vec<String> = results[0].1.channel_results.keys().cloned().collect();
    for ch_name in &channel_names {
        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                let (name_a, res_a) = &results[i];
                let (name_b, res_b) = &results[j];

                let curve_a = &res_a.channel_results[ch_name].final_curve;
                let curve_b = &res_b.channel_results[ch_name].final_curve;

                // Use the optimization frequency range from the config
                let freq_lo = 20.0_f64.max(curve_a.freq[0]).max(curve_b.freq[0]);
                let freq_hi = 500.0_f64
                    .min(curve_a.freq[curve_a.freq.len() - 1])
                    .min(curve_b.freq[curve_b.freq.len() - 1]);

                let (rms_diff, max_diff, freq_at_max) =
                    curve_diff_stats(curve_a, curve_b, freq_lo, freq_hi);

                let peak_tag = if max_diff < CROSS_MODE_FR_PEAK_WARN_DB {
                    "OK"
                } else {
                    "WARN"
                };
                let rms_status = if rms_diff < CROSS_MODE_FR_RMS_DIFF_DB {
                    "OK"
                } else {
                    "FAIL"
                };

                println!(
                    "    {ch_name:8} {name_a:12} vs {name_b:12}: peak={max_diff:.1}dB @{freq_at_max:.0}Hz [{peak_tag}]  rms={rms_diff:.2}dB [{rms_status}]",
                );

                // Peak diff is diagnostic only — room modes cause fundamental disagreement
                // RMS is the hard check for broadband agreement
                if rms_diff >= CROSS_MODE_FR_RMS_DIFF_DB {
                    failures.push(format!(
                        "{scenario_name}/{ch_name}: FR rms diff {name_a} vs {name_b} = {rms_diff:.2}dB >= {CROSS_MODE_FR_RMS_DIFF_DB}dB",
                    ));
                }
            }
        }
    }

    // Inter-channel deviation from metadata
    println!("\n  Inter-channel deviation (ICD) per mode:");
    for (mode_name, result) in &results {
        if let Some(icd) = &result.metadata.inter_channel_deviation {
            println!(
                "    {:12} midrange_rms={:.2}dB  peak={:.1}dB @{:.0}Hz  passband_rms={:.2}dB",
                mode_name,
                icd.midrange_rms_db,
                icd.midrange_peak_db,
                icd.midrange_peak_freq,
                icd.passband_rms_db,
            );
        } else {
            println!("    {:12} (no ICD data)", mode_name);
        }
    }

    // Report all failures at once for full visibility
    if !failures.is_empty() {
        println!("\n  FAILURES ({}):", failures.len());
        for f in &failures {
            println!("    - {f}");
        }
        panic!(
            "{scenario_name}: {} assertion(s) failed:\n{}",
            failures.len(),
            failures
                .iter()
                .map(|f| format!("  - {f}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    println!("  {scenario_name}: ALL CHECKS PASSED");
}

#[test]
fn test_multimode_comparison_small_stereo_2_0() {
    run_multimode_comparison("small_stereo_2_0");
}

#[test]
fn test_multimode_comparison_medium_stereo_2_0() {
    run_multimode_comparison("medium_stereo_2_0");
}

#[test]
fn test_multimode_comparison_large_stereo_2_0() {
    run_multimode_comparison("large_stereo_2_0");
}

// ============================================================================
// MixedPhase with synthetic phase data
// ============================================================================

/// Verify that MixedPhase mode exercises the full pipeline (decompose_phase + FIR)
/// when phase data is available. Uses synthetic data with known propagation delay.
#[test]
fn test_mixedphase_with_phase_data() {
    use autoeq::roomeq::synthetic::{generate_channel_curve, generate_flat_curve};
    use autoeq::roomeq::{
        MeasurementSource, MixedPhaseSerdeConfig, OptimizerConfig, SpeakerConfig, SystemConfig,
        SystemModel,
    };
    use math_audio_iir_fir::{Biquad, BiquadFilterType};
    use std::collections::HashMap;

    let sample_rate = 48000.0;

    // Generate a flat base curve
    let base = generate_flat_curve(20.0, 20000.0, 400);

    // Simulate room modes (resonances a real room would have)
    let room_modes = vec![
        Biquad::new(BiquadFilterType::Peak, 63.0, sample_rate, 8.0, 10.0),
        Biquad::new(BiquadFilterType::Peak, 125.0, sample_rate, 5.0, -8.0),
        Biquad::new(BiquadFilterType::Peak, 200.0, sample_rate, 6.0, 6.0),
        Biquad::new(BiquadFilterType::Peak, 80.0, sample_rate, 4.0, -5.0),
    ];

    // Generate channel curve WITH phase data (2ms propagation delay)
    let left = generate_channel_curve(&base, &room_modes, 2.0, 0.3, 42, sample_rate);
    let right = generate_channel_curve(&base, &room_modes, 2.5, 0.3, 43, sample_rate);

    // Verify phase data is present
    assert!(left.phase.is_some(), "Left channel must have phase data");
    assert!(right.phase.is_some(), "Right channel must have phase data");

    // Build RoomConfig with InMemory sources
    let mut speakers = HashMap::new();
    speakers.insert(
        "left".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(left)),
    );
    speakers.insert(
        "right".to_string(),
        SpeakerConfig::Single(MeasurementSource::InMemory(right)),
    );

    let mut system_speakers = HashMap::new();
    system_speakers.insert("L".to_string(), "left".to_string());
    system_speakers.insert("R".to_string(), "right".to_string());

    // Use Custom model (not Stereo) to avoid the Stereo 2.0 workflow
    // which bypasses process_single_speaker and ignores processing_mode.
    let config = RoomConfig {
        version: "1.3.0".to_string(),
        system: Some(SystemConfig {
            model: SystemModel::Custom,
            speakers: system_speakers,
            subwoofers: None,
            bass_management: None,
        }),
        speakers,
        crossovers: None,
        target_curve: None,
        optimizer: OptimizerConfig {
            processing_mode: ProcessingMode::MixedPhase,
            loss_type: "flat".to_string(),
            algorithm: "autoeq:de".to_string(),
            num_filters: 5,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 20.0,
            max_freq: 500.0,
            max_iter: 2000,
            seed: Some(42),
            refine: false,
            psychoacoustic: true,
            asymmetric_loss: true,
            mixed_phase: Some(MixedPhaseSerdeConfig {
                max_fir_length_ms: 10.0,
                pre_ringing_threshold_db: -30.0,
                min_spatial_depth: 0.5,
                phase_smoothing_octaves: 0.167,
            }),
            ..OptimizerConfig::default()
        },
        recording_config: None,
        cea2034_cache: None,
    };

    let output_dir = workspace_root().join("data_generated/roomeq_comparison/mixedphase_synthetic");
    std::fs::create_dir_all(&output_dir).unwrap();

    let result = optimize_room(&config, sample_rate, None, Some(&output_dir))
        .unwrap_or_else(|e| panic!("MixedPhase optimization failed: {e}"));

    // Verify optimization improved
    assert!(
        result.combined_post_score < result.combined_pre_score,
        "MixedPhase did not improve: pre={:.4}, post={:.4}",
        result.combined_pre_score,
        result.combined_post_score,
    );

    // Verify FIR was actually generated (not just IIR fallback)
    let has_fir = result
        .channel_results
        .values()
        .any(|ch| ch.fir_coeffs.is_some());
    assert!(
        has_fir,
        "MixedPhase with phase data should have generated FIR coefficients"
    );

    // Verify convolution plugin in DSP chain
    let has_convolution = result
        .channels
        .values()
        .any(|ch| ch.plugins.iter().any(|p| p.plugin_type == "convolution"));
    assert!(
        has_convolution,
        "MixedPhase DSP chain should include a convolution plugin for excess phase FIR"
    );

    // Save output for inspection
    let dsp_output = result.to_dsp_chain_output();
    let json = serde_json::to_string_pretty(&dsp_output).unwrap();
    std::fs::write(output_dir.join("mixed_phase.json"), json).unwrap();

    let improvement = (1.0 - result.combined_post_score / result.combined_pre_score) * 100.0;
    println!(
        "  MixedPhase with phase data: pre={:.4}, post={:.4}, improvement={:.1}%",
        result.combined_pre_score, result.combined_post_score, improvement
    );
}
