#[cfg(test)]
mod tests {
    use autoeq::MeasurementSource;
    use autoeq::roomeq::{
        CrossoverConfig, OptimizerConfig, RoomConfig, SpeakerConfig, SubwooferStrategy,
        SystemConfig, SystemModel, optimize_room,
    };
    use std::collections::HashMap;

    // Helper to create a dummy curve in memory
    fn make_test_curve(base_level: f64) -> autoeq::Curve {
        let n = 100;
        let freq: Vec<f64> = (0..n)
            .map(|i| 20.0 * (1000.0f64).powf(i as f64 / n as f64))
            .collect();
        let spl: Vec<f64> = freq
            .iter()
            .map(|f| base_level + (f / 1000.0).ln() * 2.0)
            .collect();
        autoeq::Curve {
            freq: ndarray::Array1::from_vec(freq),
            spl: ndarray::Array1::from_vec(spl),
            phase: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_v2_1_system_config_stereo() {
        // Test basic mapping: Logical "L" -> "left_meas"
        let mut speakers = HashMap::new();
        speakers.insert(
            "left_meas".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "right_meas".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
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
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = optimize_room(&config, 48000.0, None, None).expect("Optimization failed");

        // Verify output channels match logical roles
        assert!(result.channels.contains_key("L"));
        assert!(result.channels.contains_key("R"));
        assert!(!result.channels.contains_key("left_meas"));
    }

    #[test]
    fn test_v2_1_system_config_2_1() {
        // Test 2.1 mapping: L/R/LFE
        let mut speakers = HashMap::new();
        speakers.insert(
            "left_meas".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "right_meas".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(80.0))),
        );
        speakers.insert(
            "sub_meas".to_string(),
            SpeakerConfig::Single(MeasurementSource::InMemory(make_test_curve(90.0))),
        );

        let mut system_speakers = HashMap::new();
        system_speakers.insert("L".to_string(), "left_meas".to_string());
        system_speakers.insert("R".to_string(), "right_meas".to_string());
        system_speakers.insert("LFE".to_string(), "sub_meas".to_string());

        let mut sub_mapping = HashMap::new();
        sub_mapping.insert("sub_meas".to_string(), "L".to_string()); // Align sub to L

        let mut crossovers = HashMap::new();
        crossovers.insert(
            "sub_xo".to_string(),
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
                model: SystemModel::Stereo, // 2.1 is stereo base
                speakers: system_speakers,
                subwoofers: Some(autoeq::roomeq::SubwooferSystemConfig {
                    config: SubwooferStrategy::Single,
                    crossover: Some("sub_xo".to_string()),
                    mapping: sub_mapping,
                }),
                bass_management: None,
            }),
            speakers,
            crossovers: Some(crossovers),
            target_curve: None,
            optimizer: OptimizerConfig::default(),
            recording_config: None,
            ctc: None,
            cea2034_cache: None,
        };

        let result = optimize_room(&config, 48000.0, None, None).expect("Optimization failed");

        assert!(result.channels.contains_key("L"));
        assert!(result.channels.contains_key("R"));
        assert!(result.channels.contains_key("LFE"));
    }
}
