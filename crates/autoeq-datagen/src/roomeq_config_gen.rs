//! Generate autoeq RoomConfig JSON files from scenarios
//!
//! Creates the configuration JSON that the `roomeq` optimizer reads.

use anyhow::Result;
use autoeq::roomeq::{
    CardioidConfig, CrossoverConfig, MultiSubGroup, OptimizerConfig, ProcessingMode, RoomConfig,
    SpeakerConfig, SubwooferStrategy, SubwooferSystemConfig, SystemConfig, SystemModel,
};
use autoeq::{MeasurementRef, MeasurementSingle, MeasurementSource};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::scenarios::Scenario;

/// Generate a roomeq JSON config for a scenario.
///
/// The config references CSV files by filename only (config.json sits alongside CSVs).
/// `_csv_dir` is accepted for API compatibility but paths are always relative filenames.
pub fn generate_config(scenario: &Scenario, _csv_dir: &Path) -> Result<RoomConfig> {
    let source_names = &scenario.source_names;

    let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
    let mut system_speakers: HashMap<String, String> = HashMap::new();

    let mut main_sources = Vec::new();
    let mut sub_sources = Vec::new();

    for name in source_names {
        if name.starts_with("sub") {
            sub_sources.push(name);
        } else {
            main_sources.push(name);
        }
    }

    // Add main speakers and map to logical roles
    for &name in &main_sources {
        let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
        speakers.insert(
            name.to_string(),
            SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(csv_path),
                speaker_name: None,
            })),
        );

        // Simple heuristic for logical mapping
        let role = match name.as_str() {
            "left" => "L",
            "right" => "R",
            "center" => "C",
            "surround_left" => "SL",
            "surround_right" => "SR",
            "top_front_left" => "TFL",
            "top_front_right" => "TFR",
            "top_rear_left" => "TRL",
            "top_rear_right" => "TRR",
            _ => name.as_str(), // Fallback
        };
        system_speakers.insert(role.to_string(), name.to_string());
    }

    let is_cardioid = sub_sources.iter().any(|s| s.as_str() == "sub_bottom")
        && sub_sources.iter().any(|s| s.as_str() == "sub_top");

    // Configure System Model
    let model = if is_cardioid {
        SystemModel::Custom // Use custom/legacy loop for Cardioid (no alignment workflow yet)
    } else if system_speakers.contains_key("C") || system_speakers.contains_key("SL") {
        SystemModel::HomeCinema
    } else {
        SystemModel::Stereo
    };

    // Configure Subwoofers & Crossovers
    let (subwoofers_config, crossovers) = if sub_sources.is_empty() {
        (None, None)
    } else if is_cardioid {
        let sub_key = "lfe".to_string();
        system_speakers.insert("LFE".to_string(), sub_key.clone());

        // Cardioid setup
        let front_path = PathBuf::from("sub_bottom_lp0.csv");
        let rear_path = PathBuf::from("sub_top_lp0.csv");

        let config = SpeakerConfig::Cardioid(CardioidConfig {
            name: "Cardioid Stack".to_string(),
            speaker_name: None,
            front: MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(front_path),
                speaker_name: None,
            }),
            rear: MeasurementSource::Single(MeasurementSingle {
                measurement: MeasurementRef::Path(rear_path),
                speaker_name: None,
            }),
            separation_meters: 0.5,
        });

        speakers.insert(sub_key, config);
        
        // No subwoofer system config (alignment) needed for Custom mode
        (None, None)
    } else {
        // Standard Single/Multi Sub
        let sub_key = "lfe".to_string();
        system_speakers.insert("LFE".to_string(), sub_key.clone());

        let (speaker_config, strategy) = if sub_sources.len() == 1 {
            let name = &sub_sources[0];
            let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
            (
                SpeakerConfig::Single(MeasurementSource::Single(MeasurementSingle {
                    measurement: MeasurementRef::Path(csv_path),
                    speaker_name: None,
                })),
                SubwooferStrategy::Single,
            )
        } else {
            let sub_sources_objs: Vec<MeasurementSource> = sub_sources
                .iter()
                .map(|name| {
                    let csv_path = PathBuf::from(format!("{name}_lp0.csv"));
                    MeasurementSource::Single(MeasurementSingle {
                        measurement: MeasurementRef::Path(csv_path),
                        speaker_name: None,
                    })
                })
                .collect();
            (
                SpeakerConfig::MultiSub(MultiSubGroup {
                    name: "subs".to_string(),
                    speaker_name: None,
                    subwoofers: sub_sources_objs,
                }),
                SubwooferStrategy::Mso,
            )
        };

        speakers.insert(sub_key.clone(), speaker_config);

        // Crossover definition
        let mut xovers = HashMap::new();
        xovers.insert(
            "first".to_string(),
            CrossoverConfig {
                crossover_type: "LR24".to_string(),
                frequency: Some(80.0),
                frequencies: None,
                frequency_range: None,
            },
        );

        // Subwoofer System Config
        let mut mapping = HashMap::new();
        mapping.insert(sub_key, "L".to_string());

        (
            Some(SubwooferSystemConfig {
                config: strategy,
                crossover: Some("first".to_string()),
                mapping,
            }),
            Some(xovers),
        )
    };

    let optimizer = OptimizerConfig {
        algorithm: "autoeq:de".to_string(),
        num_filters: 7,
        max_freq: 500.0,
        loss_type: "flat".to_string(),
        seed: Some(42),
        max_iter: 5000,
        refine: true,
        asymmetric_loss: true,
        processing_mode: ProcessingMode::LowLatency,
        ..OptimizerConfig::default()
    };

    let config = RoomConfig {
        version: "1.2.0".to_string(),
        system: Some(SystemConfig {
            model,
            speakers: system_speakers,
            subwoofers: subwoofers_config,
        }),
        speakers,
        crossovers,
        target_curve: None,
        group_delay: None,
        optimizer,
        recording_config: None,
    };

    Ok(config)
}

/// Write a roomeq config JSON file
pub fn write_config(config: &RoomConfig, output_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(output_path, json)?;
    Ok(())
}