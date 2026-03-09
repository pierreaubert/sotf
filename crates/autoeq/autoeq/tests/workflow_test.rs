#[cfg(test)]
mod tests {
    use autoeq::roomeq::{
        CrossoverConfig, OptimizerConfig, RoomConfig, SpeakerConfig, SubwooferStrategy,
        SubwooferSystemConfig, SystemConfig, SystemModel,
    };
    use autoeq::MeasurementSource;
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
            }),
            speakers,
            crossovers: None,
            target_curve: None,
            optimizer: OptimizerConfig {
                max_iter: 100, // Fast
                ..OptimizerConfig::default()
            },
            recording_config: None,
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
            }),
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            optimizer: OptimizerConfig {
                max_iter: 100,
                ..OptimizerConfig::default()
            },
            recording_config: None,
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
}
