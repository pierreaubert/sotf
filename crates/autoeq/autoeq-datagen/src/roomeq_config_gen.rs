//! Generate autoeq RoomConfig JSON files from scenarios
//!
//! Creates the configuration JSON that the `roomeq` optimizer reads.

use anyhow::Result;
use autoeq::roomeq::{
    CardioidConfig, CrossoverConfig, MultiSubGroup, OptimizerConfig, ProcessingMode, RoomConfig,
    SpeakerConfig, SpeakerGroup, SubwooferStrategy, SubwooferSystemConfig, SystemConfig,
    SystemModel,
};
use autoeq::{MeasurementRef, MeasurementSingle, MeasurementSource};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::scenarios::Scenario;

fn make_csv_source(name: &str) -> MeasurementSource {
    MeasurementSource::Single(MeasurementSingle {
        measurement: MeasurementRef::Path(PathBuf::from(format!("{name}_lp0.csv"))),
        speaker_name: None,
    })
}

fn make_crossovers() -> HashMap<String, CrossoverConfig> {
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
    xovers
}

/// Map source name to logical speaker role
fn role_for_name(name: &str) -> &str {
    match name {
        "left" => "L",
        "right" => "R",
        "center" => "C",
        "surround_left" => "SL",
        "surround_right" => "SR",
        "top_front_left" => "TFL",
        "top_front_right" => "TFR",
        "top_rear_left" => "TRL",
        "top_rear_right" => "TRR",
        _ => name,
    }
}

/// Generate a roomeq JSON config for a scenario.
///
/// The config references CSV files by filename only (config.json sits alongside CSVs).
/// `_csv_dir` is accepted for API compatibility but paths are always relative filenames.
pub fn generate_config(scenario: &Scenario, _csv_dir: &Path) -> Result<RoomConfig> {
    let source_names = &scenario.source_names;

    // 1. Detect group pattern: sources ending with "_sub" that have matching mains
    let mut group_pairs: Vec<(&str, &str)> = Vec::new(); // (main_name, sub_name)
    let mut group_sub_names: Vec<&str> = Vec::new();

    for name in source_names {
        if let Some(main_prefix) = name.strip_suffix("_sub")
            && source_names.iter().any(|n| n == main_prefix)
        {
            group_pairs.push((main_prefix, name.as_str()));
            group_sub_names.push(name.as_str());
        }
    }
    let is_group = !group_pairs.is_empty();

    // 2. Classify remaining sources into main vs sub
    let mut main_sources: Vec<&str> = Vec::new();
    let mut sub_sources: Vec<&str> = Vec::new();

    for name in source_names {
        if group_sub_names.contains(&name.as_str()) {
            continue; // handled by group logic
        }
        if name.starts_with("sub") {
            sub_sources.push(name);
        } else {
            main_sources.push(name);
        }
    }

    let mut speakers: HashMap<String, SpeakerConfig> = HashMap::new();
    let mut system_speakers: HashMap<String, String> = HashMap::new();

    if is_group {
        // Group mode: each main+sub pair becomes a SpeakerConfig::Group
        for (main_name, sub_name) in &group_pairs {
            let group = SpeakerGroup {
                name: format!("{main_name}_group"),
                speaker_name: None,
                measurements: vec![make_csv_source(main_name), make_csv_source(sub_name)],
                crossover: Some("first".to_string()),
            };
            speakers.insert(main_name.to_string(), SpeakerConfig::Group(group));
            let role = role_for_name(main_name);
            system_speakers.insert(role.to_string(), main_name.to_string());
        }

        // Add any non-grouped main speakers (shouldn't happen for 2.2 group, but be robust)
        for &name in &main_sources {
            if group_pairs.iter().any(|(m, _)| *m == name) {
                continue; // already handled
            }
            speakers.insert(
                name.to_string(),
                SpeakerConfig::Single(make_csv_source(name)),
            );
            let role = role_for_name(name);
            system_speakers.insert(role.to_string(), name.to_string());
        }

        // Group mode: Stereo model, crossovers defined, no subwoofers section
        let model = SystemModel::Stereo;
        let crossovers = Some(make_crossovers());

        let optimizer = make_optimizer();
        let config = RoomConfig {
            version: "1.2.0".to_string(),
            system: Some(SystemConfig {
                model,
                speakers: system_speakers,
                subwoofers: None,
            }),
            speakers,
            crossovers,
            target_curve: None,
            group_delay: None,
            optimizer,
            recording_config: None,
        };
        return Ok(config);
    }

    // Non-group path: add main speakers
    for &name in &main_sources {
        speakers.insert(
            name.to_string(),
            SpeakerConfig::Single(make_csv_source(name)),
        );
        let role = role_for_name(name);
        system_speakers.insert(role.to_string(), name.to_string());
    }

    let is_cardioid =
        sub_sources.contains(&"sub_bottom") && sub_sources.contains(&"sub_top");

    // Configure System Model
    let model = if system_speakers.contains_key("C") || system_speakers.contains_key("SL") {
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

        let config = SpeakerConfig::Cardioid(CardioidConfig {
            name: "Cardioid Stack".to_string(),
            speaker_name: None,
            front: make_csv_source("sub_bottom"),
            rear: make_csv_source("sub_top"),
            separation_meters: 0.5,
        });

        speakers.insert(sub_key.clone(), config);

        // Cardioid uses Stereo model with subwoofers section (single strategy)
        let mut mapping = HashMap::new();
        mapping.insert(sub_key, "L".to_string());

        (
            Some(SubwooferSystemConfig {
                config: SubwooferStrategy::Single,
                crossover: Some("first".to_string()),
                mapping,
            }),
            Some(make_crossovers()),
        )
    } else {
        // Standard Single/Multi Sub
        let sub_key = "lfe".to_string();
        system_speakers.insert("LFE".to_string(), sub_key.clone());

        let (speaker_config, strategy) = if sub_sources.len() == 1 {
            let name = sub_sources[0];
            (
                SpeakerConfig::Single(make_csv_source(name)),
                SubwooferStrategy::Single,
            )
        } else {
            let sub_sources_objs: Vec<MeasurementSource> =
                sub_sources.iter().map(|name| make_csv_source(name)).collect();
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

        let mut mapping = HashMap::new();
        mapping.insert(sub_key, "L".to_string());

        (
            Some(SubwooferSystemConfig {
                config: strategy,
                crossover: Some("first".to_string()),
                mapping,
            }),
            Some(make_crossovers()),
        )
    };

    let optimizer = make_optimizer();

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

fn make_optimizer() -> OptimizerConfig {
    OptimizerConfig {
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
    }
}

/// Write a roomeq config JSON file
pub fn write_config(config: &RoomConfig, output_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(output_path, json)?;
    Ok(())
}
