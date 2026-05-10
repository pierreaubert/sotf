#[cfg(test)]
mod tests {
    use autoeq::MeasurementSource;
    use autoeq::roomeq::{
        CrossoverConfig, OptimizerConfig, RoomConfig, SpeakerConfig, SubwooferStrategy,
        SubwooferSystemConfig, SystemConfig, SystemModel,
    };
    use std::collections::HashMap;

    fn make_test_curve(base_level: f64) -> autoeq::Curve {
        let n = 100;
        let freq: Vec<f64> = (0..n)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
            .collect();
        // Flat curve
        let spl: Vec<f64> = vec![base_level; n];
        autoeq::Curve {
            freq: ndarray::Array1::from_vec(freq),
            spl: ndarray::Array1::from_vec(spl),
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_stereo_2_0_level_alignment() {
        // L is 80dB, R is 85dB.
        // Lowest is 80dB.
        // R should be attenuated by -5dB. L should be 0dB.

        let mut speakers = HashMap::new();
        speakers.insert(
            "left_meas".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "right_meas".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(85.0))),
        );

        let mut system_speakers = HashMap::new();
        system_speakers.insert("L".to_string(), "left_meas".to_string());
        system_speakers.insert("R".to_string(), "right_meas".to_string());

        let config = RoomConfig {
            version: "1.2.0".to_string(),
            system: Some(SystemConfig {
                model: SystemModel::Stereo,
                speakers: system_speakers,
                subwoofers: None,
                bass_management: None,
            }),
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                max_iter: 100, // Fast
                ..OptimizerConfig::default()
            },
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = autoeq::roomeq::optimize_room(&config, 48000.0, None, None)
            .expect("Optimization failed");

        // Verify alignment gains in DSP chain
        // L should have minimal gain change
        // R should have ~-5dB gain

        let l_chain = &result.channels["L"];
        let r_chain = &result.channels["R"];

        // Find gain plugins
        let get_gain = |plugins: &[autoeq::roomeq::PluginConfigWrapper]| -> f64 {
            for p in plugins {
                if p.plugin_type == "gain" {
                    return p.parameters["gain_db"].as_f64().unwrap_or(0.0);
                }
            }
            0.0
        };

        let l_gain = get_gain(&l_chain.plugins);
        let r_gain = get_gain(&r_chain.plugins);

        println!("L Gain: {}, R Gain: {}", l_gain, r_gain);

        assert!(
            l_gain.abs() < 0.1,
            "Left should not be gained (it is lowest)"
        );
        assert!(
            (r_gain - -5.0).abs() < 0.1,
            "Right should be attenuated by -5dB"
        );
    }

    #[test]
    fn test_stereo_2_1_workflow() {
        // L=80, R=80, Sub=90
        // Sub should be attenuated -10dB.

        let mut speakers = HashMap::new();
        speakers.insert(
            "l".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "r".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "sub".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(90.0))),
        );

        let mut sys_spk = HashMap::new();
        sys_spk.insert("L".to_string(), "l".to_string());
        sys_spk.insert("R".to_string(), "r".to_string());
        sys_spk.insert("LFE".to_string(), "sub".to_string());

        let mut sub_map = HashMap::new();
        sub_map.insert("sub".to_string(), "L".to_string());

        let mut crossovers = HashMap::new();
        crossovers.insert(
            "sub_xover".to_string(),
            CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        );

        let config = RoomConfig {
            version: "1.2.0".to_string(),
            system: Some(SystemConfig {
                model: SystemModel::Stereo,
                speakers: sys_spk,
                subwoofers: Some(SubwooferSystemConfig {
                    config: SubwooferStrategy::Single,
                    crossover: Some("sub_xover".to_string()),
                    mapping: sub_map,
                }),
                bass_management: None,
            }),
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            optimizer: OptimizerConfig {
                max_iter: 100,
                ..OptimizerConfig::default()
            },
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = autoeq::roomeq::optimize_room(&config, 48000.0, None, None)
            .expect("Optimization failed");

        // Verify channels exist
        assert!(result.channels.contains_key("L"));
        assert!(result.channels.contains_key("R"));
        assert!(result.channels.contains_key("LFE"));

        // Check LFE gain (should be around -10dB)
        let lfe_chain = &result.channels["LFE"];
        // Gain might be split between alignment gain and crossover gain.
        // We iterate plugins to find gains.
        let mut total_gain = 0.0;
        for p in &lfe_chain.plugins {
            if p.plugin_type == "gain" {
                total_gain += p.parameters["gain_db"].as_f64().unwrap_or(0.0);
            }
        }

        println!("LFE Total Gain: {}", total_gain);
        // It might not be exactly -10 because crossover optimization might adjust it further.
        // But alignment step should have put it near -10.
        assert!(total_gain < -8.0, "LFE should be significantly attenuated");
    }

    /// Regression for app-gpui roomeq: `to_room_config()` always sets
    /// `system: None`, so the optimizer goes through the *generic*
    /// per-speaker loop in `optimize_room_impl` rather than any workflow
    /// branch. Verify every speaker in `config.speakers` appears in
    /// `result.channel_results` — a regression where only the first
    /// speaker completed would show up here as `channel_results.len() < 2`.
    #[test]
    fn test_generic_loop_processes_all_speakers_when_system_is_none() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "right".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(82.0))),
        );

        let config = RoomConfig {
            version: "1.2.0".to_string(),
            system: None, // mirrors app-gpui's to_room_config()
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                max_iter: 100,
                num_filters: 3,
                ..OptimizerConfig::default()
            },
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = autoeq::roomeq::optimize_room(&config, 48000.0, None, None)
            .expect("Optimization of 2-speaker system:None config failed");

        assert_eq!(
            result.channel_results.len(),
            2,
            "both speakers must appear in channel_results; got keys: {:?}",
            result.channel_results.keys().collect::<Vec<_>>(),
        );
        assert!(
            result.channel_results.contains_key("left"),
            "'left' missing from channel_results: {:?}",
            result.channel_results.keys().collect::<Vec<_>>(),
        );
        assert!(
            result.channel_results.contains_key("right"),
            "'right' missing from channel_results: {:?}",
            result.channel_results.keys().collect::<Vec<_>>(),
        );
    }

    /// Realistic GPUI-style 2-speaker reproduction: mimics the Simple
    /// Wizard defaults (global optimizer, psychoacoustic + asymmetric + refine,
    /// target_response from_measurement, num_filters=7, peq_model=pk) against
    /// two non-flat curves with bass bumps. If the second speaker's
    /// optimization silently fails or hangs, both assertions below fail.
    #[test]
    fn test_generic_loop_gpui_simple_wizard_style_two_speakers() {
        use autoeq::roomeq::{TargetResponseConfig, TargetShape};

        // Build curves with a mild bass bump so the optimizer has
        // something to work on. Three different curves so speaker 2 is
        // not a trivial clone of speaker 1.
        fn make_curve_with_bump(base: f64, bump_db: f64) -> autoeq::Curve {
            let n = 200;
            let freq: Vec<f64> = (0..n)
                .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
                .collect();
            let spl: Vec<f64> = freq
                .iter()
                .map(|f| {
                    let bump = if *f < 150.0 {
                        let log_dist = (f.log10() - 60.0_f64.log10()).abs();
                        bump_db * (-log_dist * 5.0).exp()
                    } else {
                        0.0
                    };
                    base + bump
                })
                .collect();
            autoeq::Curve {
                freq: ndarray::Array1::from_vec(freq),
                spl: ndarray::Array1::from_vec(spl),
                phase: None,
                ..Default::default()
            }
        }

        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_curve_with_bump(80.0, 5.0))),
        );
        speakers.insert(
            "right".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_curve_with_bump(82.0, 7.0))),
        );

        // Mimics Simple Wizard defaults, just with a trimmed max_iter so
        // the test isn't glacial. The important bit is that `refine`,
        // `psychoacoustic`, `asymmetric_loss`, and `target_response` are
        // all enabled — the same shape a GPUI Simple run produces.
        let optimizer = OptimizerConfig {
            loss_type: "flat".to_string(),
            algorithm: "autoeq:de".to_string(),
            num_filters: 5,
            min_q: 0.5,
            max_q: 6.0,
            min_db: -12.0,
            max_db: 4.0,
            min_freq: 20.0,
            max_freq: 1600.0,
            max_iter: 500,
            population: 80,
            peq_model: "pk".to_string(),
            refine: true,
            local_algo: "cobyla".to_string(),
            psychoacoustic: true,
            asymmetric_loss: true,
            tolerance: 1e-5,
            atolerance: 1e-5,
            target_response: Some(TargetResponseConfig {
                shape: TargetShape::FromMeasurement,
                slope_db_per_octave: 0.0,
                reference_freq: 1000.0,
                curve_path: None,
                preference: Default::default(),
                broadband_precorrection: false,
                role_targets: None,
            }),
            ..OptimizerConfig::default()
        };

        let config = RoomConfig {
            version: "1.2.0".to_string(),
            system: None, // GPUI sets this to None
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer,
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = autoeq::roomeq::optimize_room(&config, 48000.0, None, None)
            .expect("GPUI-style optimize_room should succeed for 2 speakers");

        assert_eq!(
            result.channel_results.len(),
            2,
            "GPUI Simple Wizard style: both speakers must appear; got keys: {:?}",
            result.channel_results.keys().collect::<Vec<_>>(),
        );
        for name in ["left", "right"] {
            let ch = result
                .channel_results
                .get(name)
                .unwrap_or_else(|| panic!("missing channel_result for '{}'", name));
            assert!(
                !ch.biquads.is_empty(),
                "channel '{}' produced no biquad filters",
                name
            );
        }
    }

    /// Same regression but with three speakers — a surround-ish 2.1 layout
    /// without any `system` model set. If the second speaker's optimization
    /// hangs or silently aborts, the third one won't appear either.
    #[test]
    fn test_generic_loop_processes_three_speakers_when_system_is_none() {
        let mut speakers = HashMap::new();
        speakers.insert(
            "left".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "right".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(81.0))),
        );
        speakers.insert(
            "center".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(82.0))),
        );

        let config = RoomConfig {
            version: "1.2.0".to_string(),
            system: None,
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                max_iter: 100,
                num_filters: 3,
                ..OptimizerConfig::default()
            },
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = autoeq::roomeq::optimize_room(&config, 48000.0, None, None)
            .expect("Optimization of 3-speaker system:None config failed");

        assert_eq!(
            result.channel_results.len(),
            3,
            "all three speakers must appear in channel_results; got keys: {:?}",
            result.channel_results.keys().collect::<Vec<_>>(),
        );
    }
}
