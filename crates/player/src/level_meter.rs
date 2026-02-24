//! Level meter types and construction logic.
//!
//! Shared between all app frontends (GPUI, TUI, etc.)

use sotf_plugins::speaker_config::{
    MeterGroupSpec, get_meter_groups, get_meter_groups_by_channels, make_fallback_channel,
};

/// Channel group for level meter display
#[derive(Debug, Clone)]
pub struct ChannelGroup {
    pub name: String,
    pub channels: Vec<ChannelInfo>,
    pub muted: bool,
    pub soloed: bool,
    pub dimmed: bool,
}

/// Individual channel information
#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub index: usize,              // Index in loudness.channel_peaks
    pub name: String,              // e.g., "FL", "FR", "C"
    pub display_name: Vec<String>, // Multi-line display: ["F", "L"] or ["T", "B", "R"]
}

/// Convert a `MeterGroupSpec` slice into runtime `ChannelGroup` objects.
fn groups_from_specs(specs: &[MeterGroupSpec]) -> Vec<ChannelGroup> {
    specs
        .iter()
        .map(|group_spec| ChannelGroup {
            name: group_spec.name.to_string(),
            channels: group_spec
                .channels
                .iter()
                .map(|ch| ChannelInfo {
                    index: ch.index,
                    name: ch.label.to_string(),
                    display_name: ch.display_chars.iter().map(|s| (*s).to_string()).collect(),
                })
                .collect(),
            muted: false,
            soloed: false,
            dimmed: false,
        })
        .collect()
}

/// Build level meter channel groups for the given channel count and optional speaker config.
///
/// Returns an empty `Vec` when `num_channels == 0`.
pub fn build_level_meter_groups(
    num_channels: usize,
    speaker_config: Option<&str>,
) -> Vec<ChannelGroup> {
    if num_channels == 0 {
        return Vec::new();
    }

    // Try to resolve via explicit speaker config, then by channel count
    let meter_groups: Option<&[MeterGroupSpec]> = speaker_config
        .and_then(get_meter_groups)
        .or_else(|| get_meter_groups_by_channels(num_channels));

    if let Some(specs) = meter_groups {
        return groups_from_specs(specs);
    }

    // Fallback for unknown channel counts
    match num_channels {
        1 => {
            vec![ChannelGroup {
                name: "Mono".to_string(),
                channels: vec![ChannelInfo {
                    index: 0,
                    name: "M".to_string(),
                    display_name: vec!["M".to_string()],
                }],
                muted: false,
                soloed: false,
                dimmed: false,
            }]
        }
        4 => {
            vec![
                ChannelGroup {
                    name: "L/R".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 0,
                            name: "L".to_string(),
                            display_name: vec!["L".to_string()],
                        },
                        ChannelInfo {
                            index: 1,
                            name: "R".to_string(),
                            display_name: vec!["R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                },
                ChannelGroup {
                    name: "Surrounds".to_string(),
                    channels: vec![
                        ChannelInfo {
                            index: 2,
                            name: "SL".to_string(),
                            display_name: vec!["S".to_string(), "L".to_string()],
                        },
                        ChannelInfo {
                            index: 3,
                            name: "SR".to_string(),
                            display_name: vec!["S".to_string(), "R".to_string()],
                        },
                    ],
                    muted: false,
                    soloed: false,
                    dimmed: false,
                },
            ]
        }
        _ => {
            let channels: Vec<ChannelInfo> = (0..num_channels)
                .map(|i| {
                    let spec = make_fallback_channel(i);
                    ChannelInfo {
                        index: spec.index,
                        name: spec.label.to_string(),
                        display_name: spec
                            .display_chars
                            .iter()
                            .map(|s| (*s).to_string())
                            .collect(),
                    }
                })
                .collect();
            vec![ChannelGroup {
                name: "All Channels".to_string(),
                channels,
                muted: false,
                soloed: false,
                dimmed: false,
            }]
        }
    }
}
