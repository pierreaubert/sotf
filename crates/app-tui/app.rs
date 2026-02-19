use crate::theme::Theme;
use sotf_audio::LoudnessData;
use sotf_audio::devices::AudioDevice;
use sotf_audio_player::{Album, MusicLibrary, PluginChain, PluginType, Track};
use sotf_plugins::speaker_config::{
    MeterGroupSpec, get_meter_groups, get_meter_groups_by_channels, make_fallback_channel,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Library,
    DirectoryManager,
    Queue,
    Plugins,
    Devices,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    AddDirectory,
    AddPlugin,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    BrowseSofaFile,
    BrowseIrFile,
    ShowHelp,
    ShowError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPane {
    Main,   // Main content area (library, queue, etc.)
    Meters, // Right column with level meters
}

/// Matrix plugin editor mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatrixEditMode {
    #[default]
    Header, // Editing input/output channels, preset
    Grid, // Editing matrix cells
}

/// Tree view mode for library
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryViewMode {
    Flat,     // Original list view
    TreeView, // Hierarchical artist → albums
}

/// Library sort order
pub use sotf_audio_player::library::LibrarySortOrder;

/// Channel filter options
pub use sotf_audio_player::library::ChannelFilter;

/// Artist node in tree view
#[derive(Debug, Clone)]
pub struct ArtistNode {
    pub artist: String,
    pub album_indices: Vec<usize>, // Indices into library.albums
    pub expanded: bool,
}

/// Tree item type for rendering
#[derive(Debug, Clone)]
pub enum TreeItem {
    Artist { name: String, expanded: bool },
    Album { index: usize },
}

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

/// Pending parameter update for zero-dropout updates
#[derive(Debug, Clone)]
pub struct PendingParameterUpdate {
    pub plugin_index: usize,
    pub param_id: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub album: Album,
    pub current_track_index: usize,
}

impl QueueItem {
    pub fn new(album: Album) -> Self {
        Self {
            album,
            current_track_index: 0,
        }
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.album.tracks.get(self.current_track_index)
    }

    pub fn next_track(&mut self) -> Option<&Track> {
        if self.current_track_index + 1 < self.album.tracks.len() {
            self.current_track_index += 1;
            self.current_track()
        } else {
            None
        }
    }

    pub fn previous_track(&mut self) -> Option<&Track> {
        if self.current_track_index > 0 {
            self.current_track_index -= 1;
            self.current_track()
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub item: QueueItem,
    pub expanded: bool,
}

impl QueueEntry {
    pub fn new(item: QueueItem) -> Self {
        Self {
            item,
            expanded: false,
        }
    }
}

/// Specification for a plugin parameter in the TUI
pub struct TuiParamSpec {
    pub name: String,
    pub value: String,
    pub unit: String,
}

pub trait TuiEditablePlugin {
    fn get_params(&self) -> Vec<TuiParamSpec>;
    fn adjust_param(&mut self, index: usize, delta: f64) -> bool;
}

impl TuiEditablePlugin for sotf_audio_player::PluginSettings {
    fn get_params(&self) -> Vec<TuiParamSpec> {
        match self {
            sotf_audio_player::PluginSettings::Gain { gain_db, .. } => vec![TuiParamSpec {
                name: "Gain".to_string(),
                value: format!("{:.1}", gain_db),
                unit: "dB".to_string(),
            }],
            sotf_audio_player::PluginSettings::EQ { filters, .. } => {
                let mut params = Vec::new();
                for (i, filter) in filters.iter().enumerate() {
                    params.push(TuiParamSpec {
                        name: format!("F{} Freq", i + 1),
                        value: format!("{:.0}", filter.frequency),
                        unit: "Hz".to_string(),
                    });
                    params.push(TuiParamSpec {
                        name: format!("F{} Q", i + 1),
                        value: format!("{:.2}", filter.q),
                        unit: "".to_string(),
                    });
                    params.push(TuiParamSpec {
                        name: format!("F{} Gain", i + 1),
                        value: format!("{:.1}", filter.gain_db),
                        unit: "dB".to_string(),
                    });
                    params.push(TuiParamSpec {
                        name: format!("F{} Type", i + 1),
                        value: format!("{:?}", filter.filter_type),
                        unit: "".to_string(),
                    });
                }
                params
            }
            sotf_audio_player::PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => vec![
                TuiParamSpec {
                    name: "Threshold".to_string(),
                    value: format!("{:.1}", threshold_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Ratio".to_string(),
                    value: format!("{:.1}", ratio),
                    unit: ":1".to_string(),
                },
                TuiParamSpec {
                    name: "Attack".to_string(),
                    value: format!("{:.1}", attack_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Release".to_string(),
                    value: format!("{:.1}", release_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Knee".to_string(),
                    value: format!("{:.1}", knee_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Makeup".to_string(),
                    value: format!("{:.1}", makeup_gain_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Mix".to_string(),
                    value: format!("{:.0}", mix * 100.0),
                    unit: "%".to_string(),
                },
                TuiParamSpec {
                    name: "Auto Makeup".to_string(),
                    value: (if *auto_makeup { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Link".to_string(),
                    value: (if *link_channels { "Linked" } else { "Unlinked" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Sidechain HPF".to_string(),
                    value: format!("{:.0}", sidechain_hpf_hz),
                    unit: "Hz".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => vec![
                TuiParamSpec {
                    name: "Threshold".to_string(),
                    value: format!("{:.1}", threshold_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Release".to_string(),
                    value: format!("{:.1}", release_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Lookahead".to_string(),
                    value: format!("{:.1}", lookahead_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Soft Limit".to_string(),
                    value: (if *soft { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Mix".to_string(),
                    value: format!("{:.0}", mix * 100.0),
                    unit: "%".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                ambient_boost,
                safety_cap_db,
                rear_ambient_boost,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
            } => vec![
                // 0: Speaker Config (structural)
                TuiParamSpec { name: "Speaker Config".into(), value: speaker_config.clone(), unit: "".into() },
                // 1-4: Gain parameters
                TuiParamSpec { name: "Front Direct".into(), value: format!("{:.2}", gain_front_direct), unit: "x".into() },
                TuiParamSpec { name: "Front Ambient".into(), value: format!("{:.2}", gain_front_ambient), unit: "x".into() },
                TuiParamSpec { name: "Rear Ambient".into(), value: format!("{:.2}", gain_rear_ambient), unit: "x".into() },
                TuiParamSpec { name: "Height Gain".into(), value: format!("{:.2}", height_gain), unit: "x".into() },
                // 5-6: LFE
                TuiParamSpec { name: "LFE Gain".into(), value: format!("{:.2}", lfe_gain), unit: "x".into() },
                TuiParamSpec { name: "LFE Cutoff".into(), value: format!("{:.0}", lfe_cutoff_hz), unit: "Hz".into() },
                // 7-9: Stereo/spatial
                TuiParamSpec { name: "Stereo Width".into(), value: format!("{:.2}", stereo_width), unit: "".into() },
                TuiParamSpec { name: "Center Spread".into(), value: format!("{:.2}", center_spread), unit: "".into() },
                TuiParamSpec { name: "Bandpass".into(), value: format!("{:.0}", bandpass_hz), unit: "Hz".into() },
                // 10-11: Subharmonic
                TuiParamSpec { name: "Subharmonic".into(), value: (if *enable_subharmonic_synth { "On" } else { "Off" }).into(), unit: "".into() },
                TuiParamSpec { name: "Sub Gain".into(), value: format!("{:.2}", subharmonic_gain), unit: "x".into() },
                // 12-13: Height routing
                TuiParamSpec { name: "HR Direct".into(), value: (if *enable_hr_direct { "On" } else { "Off" }).into(), unit: "".into() },
                TuiParamSpec { name: "HR Sharpen".into(), value: format!("{:.2}", hr_sharpen), unit: "".into() },
                // 14: Safety
                TuiParamSpec { name: "Safety Cap".into(), value: format!("{:.1}", safety_cap_db), unit: "dB".into() },
                // 15: Decorrelation mode
                TuiParamSpec { name: "Decor Mode".into(), value: format!("{}", decorrelation_mode), unit: "".into() },
                // 16-18: Extended subharmonic
                TuiParamSpec { name: "Sub Freq".into(), value: format!("{:.0}", subharmonic_freq_hz), unit: "Hz".into() },
                TuiParamSpec { name: "Sub Attack".into(), value: format!("{:.1}", subharmonic_attack_ms), unit: "ms".into() },
                TuiParamSpec { name: "Sub Release".into(), value: format!("{:.1}", subharmonic_release_ms), unit: "ms".into() },
                // 19-21: Decorrelation details
                TuiParamSpec { name: "Decor LFO Rate".into(), value: format!("{:.2}", decorrelation_lfo_rate_hz), unit: "Hz".into() },
                TuiParamSpec { name: "Velvet Duration".into(), value: format!("{:.0}", velvet_noise_duration_ms), unit: "ms".into() },
                TuiParamSpec { name: "Velvet Density".into(), value: format!("{:.0}", velvet_noise_density), unit: "".into() },
                // 22-24: Height details
                TuiParamSpec { name: "Height HF Cap".into(), value: format!("{:.0}", height_hf_cap_hz), unit: "Hz".into() },
                TuiParamSpec { name: "Height Trans Red".into(), value: format!("{:.2}", height_transient_reduction), unit: "".into() },
                TuiParamSpec { name: "Height Direct Leak".into(), value: format!("{:.2}", height_direct_leak), unit: "".into() },
                // 25-28: Surround/ambient
                TuiParamSpec { name: "Surr Direct Bleed".into(), value: format!("{:.2}", surround_direct_bleed), unit: "".into() },
                TuiParamSpec { name: "Rear Amb Boost".into(), value: format!("{:.2}", rear_ambient_boost), unit: "x".into() },
                TuiParamSpec { name: "Rear Late Refl".into(), value: format!("{:.2}", rear_late_reflection), unit: "".into() },
                TuiParamSpec { name: "Ambient Boost".into(), value: format!("{:.2}", ambient_boost), unit: "x".into() },
                // 29-31: Dialogue
                TuiParamSpec { name: "Dialogue Weight".into(), value: format!("{:.2}", dialogue_weight), unit: "".into() },
                TuiParamSpec { name: "Voice Freq Min".into(), value: format!("{:.0}", voice_freq_min_hz), unit: "Hz".into() },
                TuiParamSpec { name: "Voice Freq Max".into(), value: format!("{:.0}", voice_freq_max_hz), unit: "Hz".into() },
                // 32-34: Bypass diagnostics
                TuiParamSpec { name: "Bypass Decor".into(), value: (if *bypass_decorrelation { "On" } else { "Off" }).into(), unit: "".into() },
                TuiParamSpec { name: "Bypass Transient".into(), value: (if *bypass_transient_detection { "On" } else { "Off" }).into(), unit: "".into() },
                TuiParamSpec { name: "Bypass All".into(), value: (if *bypass_all_processing { "On" } else { "Off" }).into(), unit: "".into() },
            ],
            sotf_audio_player::PluginSettings::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
            } => vec![
                TuiParamSpec {
                    name: "SOFA File".to_string(),
                    value: if sofa_file.is_empty() {
                        "None".to_string()
                    } else {
                        PathBuf::from(sofa_file)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    },
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Input Channels".to_string(),
                    value: format!("{}", input_channels),
                    unit: "ch".to_string(),
                },
                TuiParamSpec {
                    name: "Optimization".to_string(),
                    value: (if *enable_optimization { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Externalization".to_string(),
                    value: format!("{:.2}", externalization),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Near-field".to_string(),
                    value: format!("{:.2}", near_field_strength),
                    unit: "".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::Convolution {
                ir_file,
                mix,
                gain_db,
            } => vec![
                TuiParamSpec {
                    name: "IR File".to_string(),
                    value: if ir_file.is_empty() {
                        "None".to_string()
                    } else {
                        PathBuf::from(ir_file)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    },
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Mix".to_string(),
                    value: format!("{:.0}", mix * 100.0),
                    unit: "%".to_string(),
                },
                TuiParamSpec {
                    name: "Gain".to_string(),
                    value: format!("{:.1}", gain_db),
                    unit: "dB".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::Gate {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => vec![
                TuiParamSpec {
                    name: "Threshold".to_string(),
                    value: format!("{:.1}", threshold_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Ratio".to_string(),
                    value: format!("{:.1}", ratio),
                    unit: ":1".to_string(),
                },
                TuiParamSpec {
                    name: "Attack".to_string(),
                    value: format!("{:.1}", attack_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Hold".to_string(),
                    value: format!("{:.1}", hold_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Release".to_string(),
                    value: format!("{:.1}", release_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Mix".to_string(),
                    value: format!("{:.0}", mix * 100.0),
                    unit: "%".to_string(),
                },
                TuiParamSpec {
                    name: "Link".to_string(),
                    value: (if *link_channels { "Linked" } else { "Unlinked" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Sidechain HPF".to_string(),
                    value: format!("{:.0}", sidechain_hpf_hz),
                    unit: "Hz".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            } => vec![
                TuiParamSpec {
                    name: "Low Freq".to_string(),
                    value: format!("{:.0}", low_freq),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "Low Gain".to_string(),
                    value: format!("{:.1}", low_gain),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "High Freq".to_string(),
                    value: format!("{:.0}", high_freq),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "High Gain".to_string(),
                    value: format!("{:.1}", high_gain),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Auto Gain".to_string(),
                    value: (if *auto_gain_enabled { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Max Auto Gain".to_string(),
                    value: format!("{:.1}", auto_gain_max_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Auto Gain Smooth".to_string(),
                    value: format!("{:.1}", auto_gain_smoothing_ms),
                    unit: "ms".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => vec![
                TuiParamSpec {
                    name: "Center Gain".to_string(),
                    value: format!("{:.1}", center_gain_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Surround Gain".to_string(),
                    value: format!("{:.1}", surround_gain_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Height Gain".to_string(),
                    value: format!("{:.1}", height_gain_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "LFE Gain".to_string(),
                    value: format!("{:.1}", lfe_gain_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Phase Coherence".to_string(),
                    value: (if *phase_coherence { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Blend Low".to_string(),
                    value: format!("{:.0}", phase_blend_low_hz),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "Blend High".to_string(),
                    value: format!("{:.0}", phase_blend_high_hz),
                    unit: "Hz".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => vec![
                TuiParamSpec {
                    name: "Width".to_string(),
                    value: format!("{:.2}", stereo_width),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Haas Delay".to_string(),
                    value: format!("{:.1}", haas_delay_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Panning EQ".to_string(),
                    value: (if *enable_comp_eq { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "EQ Depth".to_string(),
                    value: format!("{:.1}", comp_eq_depth_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Decor Low".to_string(),
                    value: format!("{:.0}", decor_low_hz),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "Decor High".to_string(),
                    value: format!("{:.0}", decor_high_hz),
                    unit: "Hz".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::BandSplit {
                frequency,
                crossover_type,
                ..
            } => vec![
                TuiParamSpec {
                    name: "Frequency".to_string(),
                    value: format!("{:.0}", frequency),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "Type".to_string(),
                    value: crossover_type.clone(),
                    unit: "".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::BandMerge { bands, .. } => vec![TuiParamSpec {
                name: "Bands".to_string(),
                value: format!("{}", bands),
                unit: "".to_string(),
            }],
            sotf_audio_player::PluginSettings::Expander {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => vec![
                TuiParamSpec {
                    name: "Threshold".to_string(),
                    value: format!("{:.1}", threshold_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Ratio".to_string(),
                    value: format!("{:.1}", ratio),
                    unit: ":1".to_string(),
                },
                TuiParamSpec {
                    name: "Attack".to_string(),
                    value: format!("{:.1}", attack_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Release".to_string(),
                    value: format!("{:.0}", release_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Range".to_string(),
                    value: format!("{:.1}", range_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Knee".to_string(),
                    value: format!("{:.1}", knee_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Hysteresis".to_string(),
                    value: format!("{:.1}", hysteresis_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Hold".to_string(),
                    value: format!("{:.0}", hold_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Mix".to_string(),
                    value: format!("{:.0}", mix * 100.0),
                    unit: "%".to_string(),
                },
                TuiParamSpec {
                    name: "Link".to_string(),
                    value: (if *link_channels { "Linked" } else { "Unlinked" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Sidechain HPF".to_string(),
                    value: format!("{:.0}", sidechain_hpf_hz),
                    unit: "Hz".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::MultibandCompressor {
                num_bands,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                mix,
                link_channels,
                bands,
                ..
            } => {
                let mut params = vec![
                    TuiParamSpec { name: "Bands".into(), value: format!("{}", num_bands), unit: "".into() },
                    TuiParamSpec { name: "Crossover 1".into(), value: format!("{:.0}", crossover_freq_1), unit: "Hz".into() },
                    TuiParamSpec { name: "Crossover 2".into(), value: format!("{:.0}", crossover_freq_2), unit: "Hz".into() },
                    TuiParamSpec { name: "Crossover 3".into(), value: format!("{:.0}", crossover_freq_3), unit: "Hz".into() },
                    TuiParamSpec { name: "Crossover 4".into(), value: format!("{:.0}", crossover_freq_4), unit: "Hz".into() },
                    TuiParamSpec { name: "Threshold".into(), value: format!("{:.1}", threshold_db), unit: "dB".into() },
                    TuiParamSpec { name: "Ratio".into(), value: format!("{:.1}", ratio), unit: ":1".into() },
                    TuiParamSpec { name: "Attack".into(), value: format!("{:.1}", attack_ms), unit: "ms".into() },
                    TuiParamSpec { name: "Release".into(), value: format!("{:.0}", release_ms), unit: "ms".into() },
                    TuiParamSpec { name: "Knee".into(), value: format!("{:.1}", knee_db), unit: "dB".into() },
                    TuiParamSpec { name: "Mix".into(), value: format!("{:.0}", mix * 100.0), unit: "%".into() },
                    TuiParamSpec { name: "Link".into(), value: (if *link_channels { "Linked" } else { "Unlinked" }).into(), unit: "".into() },
                ];
                // Per-band params: 8 per band (solo, bypass, threshold, ratio, attack, release, knee, makeup)
                for i in 0..*num_bands {
                    let band = bands.get(i);
                    params.push(TuiParamSpec { name: format!("B{} Solo", i+1), value: if band.is_some_and(|b| b.solo) { "On" } else { "Off" }.into(), unit: "".into() });
                    params.push(TuiParamSpec { name: format!("B{} Bypass", i+1), value: if band.is_some_and(|b| b.bypass) { "On" } else { "Off" }.into(), unit: "".into() });
                    params.push(TuiParamSpec { name: format!("B{} Threshold", i+1), value: band.and_then(|b| b.threshold_db).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "dB".into() });
                    params.push(TuiParamSpec { name: format!("B{} Ratio", i+1), value: band.and_then(|b| b.ratio).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: ":1".into() });
                    params.push(TuiParamSpec { name: format!("B{} Attack", i+1), value: band.and_then(|b| b.attack_ms).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "ms".into() });
                    params.push(TuiParamSpec { name: format!("B{} Release", i+1), value: band.and_then(|b| b.release_ms).map(|v| format!("{:.0}", v)).unwrap_or("Global".into()), unit: "ms".into() });
                    params.push(TuiParamSpec { name: format!("B{} Knee", i+1), value: band.and_then(|b| b.knee_db).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "dB".into() });
                    params.push(TuiParamSpec { name: format!("B{} Makeup", i+1), value: format!("{:.1}", band.map(|b| b.makeup_gain_db).unwrap_or(0.0)), unit: "dB".into() });
                }
                params
            }
            sotf_audio_player::PluginSettings::MultibandExpander {
                num_bands,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                bands,
                ..
            } => {
                let mut params = vec![
                    TuiParamSpec { name: "Bands".into(), value: format!("{}", num_bands), unit: "".into() },
                    TuiParamSpec { name: "Crossover 1".into(), value: format!("{:.0}", crossover_freq_1), unit: "Hz".into() },
                    TuiParamSpec { name: "Crossover 2".into(), value: format!("{:.0}", crossover_freq_2), unit: "Hz".into() },
                    TuiParamSpec { name: "Crossover 3".into(), value: format!("{:.0}", crossover_freq_3), unit: "Hz".into() },
                    TuiParamSpec { name: "Crossover 4".into(), value: format!("{:.0}", crossover_freq_4), unit: "Hz".into() },
                    TuiParamSpec { name: "Threshold".into(), value: format!("{:.1}", threshold_db), unit: "dB".into() },
                    TuiParamSpec { name: "Ratio".into(), value: format!("{:.1}", ratio), unit: ":1".into() },
                    TuiParamSpec { name: "Attack".into(), value: format!("{:.1}", attack_ms), unit: "ms".into() },
                    TuiParamSpec { name: "Release".into(), value: format!("{:.0}", release_ms), unit: "ms".into() },
                    TuiParamSpec { name: "Range".into(), value: format!("{:.1}", range_db), unit: "dB".into() },
                    TuiParamSpec { name: "Knee".into(), value: format!("{:.1}", knee_db), unit: "dB".into() },
                    TuiParamSpec { name: "Hysteresis".into(), value: format!("{:.1}", hysteresis_db), unit: "dB".into() },
                    TuiParamSpec { name: "Hold".into(), value: format!("{:.0}", hold_ms), unit: "ms".into() },
                    TuiParamSpec { name: "Mix".into(), value: format!("{:.0}", mix * 100.0), unit: "%".into() },
                    TuiParamSpec { name: "Link".into(), value: (if *link_channels { "Linked" } else { "Unlinked" }).into(), unit: "".into() },
                ];
                // Per-band params: 10 per band (solo, bypass, threshold, ratio, attack, release, range, knee, hysteresis, hold)
                for i in 0..*num_bands {
                    let band = bands.get(i);
                    params.push(TuiParamSpec { name: format!("B{} Solo", i+1), value: if band.is_some_and(|b| b.solo) { "On" } else { "Off" }.into(), unit: "".into() });
                    params.push(TuiParamSpec { name: format!("B{} Bypass", i+1), value: if band.is_some_and(|b| b.bypass) { "On" } else { "Off" }.into(), unit: "".into() });
                    params.push(TuiParamSpec { name: format!("B{} Threshold", i+1), value: band.and_then(|b| b.threshold_db).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "dB".into() });
                    params.push(TuiParamSpec { name: format!("B{} Ratio", i+1), value: band.and_then(|b| b.ratio).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: ":1".into() });
                    params.push(TuiParamSpec { name: format!("B{} Attack", i+1), value: band.and_then(|b| b.attack_ms).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "ms".into() });
                    params.push(TuiParamSpec { name: format!("B{} Release", i+1), value: band.and_then(|b| b.release_ms).map(|v| format!("{:.0}", v)).unwrap_or("Global".into()), unit: "ms".into() });
                    params.push(TuiParamSpec { name: format!("B{} Range", i+1), value: band.and_then(|b| b.range_db).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "dB".into() });
                    params.push(TuiParamSpec { name: format!("B{} Knee", i+1), value: band.and_then(|b| b.knee_db).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "dB".into() });
                    params.push(TuiParamSpec { name: format!("B{} Hysteresis", i+1), value: band.and_then(|b| b.hysteresis_db).map(|v| format!("{:.1}", v)).unwrap_or("Global".into()), unit: "dB".into() });
                    params.push(TuiParamSpec { name: format!("B{} Hold", i+1), value: band.and_then(|b| b.hold_ms).map(|v| format!("{:.0}", v)).unwrap_or("Global".into()), unit: "ms".into() });
                }
                params
            }
            sotf_audio_player::PluginSettings::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
            } => vec![
                TuiParamSpec {
                    name: "Distance".to_string(),
                    value: format!("{:.2}", distance_m),
                    unit: "m".to_string(),
                },
                TuiParamSpec {
                    name: "Speaker Angle".to_string(),
                    value: format!("{:.1}", speaker_angle_deg),
                    unit: "\u{00b0}".to_string(),
                },
                TuiParamSpec {
                    name: "Head Radius".to_string(),
                    value: format!("{:.4}", head_radius_m),
                    unit: "m".to_string(),
                },
                TuiParamSpec {
                    name: "Beta Base".to_string(),
                    value: format!("{:.4}", beta_base),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Beta Low Boost".to_string(),
                    value: format!("{:.1}", beta_low_freq_boost),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Beta High Boost".to_string(),
                    value: format!("{:.1}", beta_high_freq_boost),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Shadow Cutoff".to_string(),
                    value: format!("{:.0}", head_shadow_cutoff_hz),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "Shadow Slope".to_string(),
                    value: format!("{:.1}", head_shadow_slope_db_per_octave),
                    unit: "dB/oct".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::Denoiser {
                reduction_db,
                floor_db,
                smoothing,
                attack_ms,
                release_ms,
                low_latency,
                polyphonic_detection,
                transparency,
                dd_enabled,
                dd_alpha,
                psychoacoustic_masking,
                use_captured_profile,
            } => vec![
                TuiParamSpec {
                    name: "Reduction".to_string(),
                    value: format!("{:.1}", reduction_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Floor".to_string(),
                    value: format!("{:.1}", floor_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Smoothing".to_string(),
                    value: format!("{:.2}", smoothing),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Attack".to_string(),
                    value: format!("{:.1}", attack_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Release".to_string(),
                    value: format!("{:.1}", release_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Low Latency".to_string(),
                    value: (if *low_latency { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Polyphonic".to_string(),
                    value: (if *polyphonic_detection { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Transparency".to_string(),
                    value: format!("{:.2}", transparency),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "DD Enabled".to_string(),
                    value: (if *dd_enabled { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "DD Alpha".to_string(),
                    value: format!("{:.3}", dd_alpha),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Psychoacoustic".to_string(),
                    value: (if *psychoacoustic_masking { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Use Profile".to_string(),
                    value: (if *use_captured_profile { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => vec![
                TuiParamSpec {
                    name: "Correction".to_string(),
                    value: format!("{:.2}", correction_strength),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Analysis Window".to_string(),
                    value: format!("{:.1}", analysis_window_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Drift Smoothing".to_string(),
                    value: format!("{:.3}", drift_smoothing),
                    unit: "".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::ABCompare {
                mix,
                mix_mode,
                selected_path,
                bypass,
                auto_gain_enabled,
                loudness_type,
                max_auto_gain_db,
                gain_smoothing_ms,
                mix_transition_ms,
                path_a_config,
                path_b_config,
            } => vec![
                TuiParamSpec {
                    name: "Mix (A/B)".to_string(),
                    value: format!("{:.2}", mix),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Mix Mode".to_string(),
                    value: (if *mix_mode == 0 { "Pot" } else { "Binary" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Selected Path".to_string(),
                    value: (if *selected_path == 0 { "A" } else { "B" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Bypass".to_string(),
                    value: (if *bypass { "Yes" } else { "No" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Auto Gain".to_string(),
                    value: (if *auto_gain_enabled { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Loudness Type".to_string(),
                    value: (if *loudness_type == 0 {
                        "Momentary"
                    } else {
                        "Short-term"
                    })
                    .to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Max Auto Gain".to_string(),
                    value: format!("{:.1}", max_auto_gain_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Gain Smoothing".to_string(),
                    value: format!("{:.1}", gain_smoothing_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Mix Transition".to_string(),
                    value: format!("{:.1}", mix_transition_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Path A".to_string(),
                    value: path_a_config.clone(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Path B".to_string(),
                    value: path_b_config.clone(),
                    unit: "".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::SpectrumAnalyzer {
                num_bins,
                min_freq,
                max_freq,
                smoothing,
                tilt_correction,
                tilt_reference,
            } => vec![
                TuiParamSpec {
                    name: "Bins".to_string(),
                    value: format!("{}", num_bins),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Min Freq".to_string(),
                    value: format!("{:.0}", min_freq),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "Max Freq".to_string(),
                    value: format!("{:.0}", max_freq),
                    unit: "Hz".to_string(),
                },
                TuiParamSpec {
                    name: "Smoothing".to_string(),
                    value: format!("{:.2}", smoothing),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Tilt Correction".to_string(),
                    value: format!("{:?}", tilt_correction),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Tilt Reference".to_string(),
                    value: format!("{:?}", tilt_reference),
                    unit: "".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::LoudnessMonitor => vec![],
            sotf_audio_player::PluginSettings::ChannelMuteSolo { enabled, .. } => {
                vec![TuiParamSpec {
                    name: "Enabled".to_string(),
                    value: (if *enabled { "Active" } else { "Bypassed" }).to_string(),
                    unit: "".to_string(),
                }]
            }
            sotf_audio_player::PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } => vec![
                TuiParamSpec {
                    name: "Input Ch".to_string(),
                    value: format!("{}", input_channels),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Output Ch".to_string(),
                    value: format!("{}", output_channels),
                    unit: "".to_string(),
                },
            ],
            sotf_audio_player::PluginSettings::FletcherMunson {
                reference_level_db,
                enabled,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                ..
            } => vec![
                TuiParamSpec {
                    name: "Reference Level".to_string(),
                    value: format!("{:.1}", reference_level_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Enabled".to_string(),
                    value: (if *enabled { "Yes" } else { "No" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Smoothing".to_string(),
                    value: format!("{:.1}", smoothing_ms),
                    unit: "ms".to_string(),
                },
                TuiParamSpec {
                    name: "Auto Gain".to_string(),
                    value: (if *auto_gain_enabled { "On" } else { "Off" }).to_string(),
                    unit: "".to_string(),
                },
                TuiParamSpec {
                    name: "Max Auto Gain".to_string(),
                    value: format!("{:.1}", auto_gain_max_db),
                    unit: "dB".to_string(),
                },
                TuiParamSpec {
                    name: "Auto Gain Smooth".to_string(),
                    value: format!("{:.1}", auto_gain_smoothing_ms),
                    unit: "ms".to_string(),
                },
            ],
        }
    }

    fn adjust_param(&mut self, index: usize, delta: f64) -> bool {
        match self {
            sotf_audio_player::PluginSettings::Gain { gain_db, .. } => {
                if index == 0 {
                    *gain_db = (*gain_db + delta * 0.5).clamp(-40.0, 40.0);
                    return true;
                }
            }
            sotf_audio_player::PluginSettings::EQ { filters, .. } => {
                let filter_idx = index / 4;
                let param_idx = index % 4;
                if let Some(filter) = filters.get_mut(filter_idx) {
                    match param_idx {
                        0 => {
                            filter.frequency =
                                (filter.frequency + delta * 10.0).clamp(20.0, 20000.0)
                        }
                        1 => filter.q = (filter.q + delta * 0.1).clamp(0.1, 10.0),
                        2 => filter.gain_db = (filter.gain_db + delta * 0.5).clamp(-24.0, 24.0),
                        3 => {
                            use sotf_audio_player::BiquadFilterType;
                            let types = [
                                BiquadFilterType::Peak,
                                BiquadFilterType::Lowshelf,
                                BiquadFilterType::Highshelf,
                                BiquadFilterType::Lowpass,
                                BiquadFilterType::Highpass,
                                BiquadFilterType::Bandpass,
                                BiquadFilterType::Notch,
                            ];
                            let current_idx = types
                                .iter()
                                .position(|t| *t == filter.filter_type)
                                .unwrap_or(0);
                            let new_idx = if delta > 0.0 {
                                (current_idx + 1) % types.len()
                            } else {
                                (current_idx + types.len() - 1) % types.len()
                            };
                            filter.filter_type = types[new_idx];
                        }
                        _ => return false,
                    }
                    return true;
                }
            }
            sotf_audio_player::PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => {
                match index {
                    0 => *threshold_db = (*threshold_db + delta).clamp(-60.0, 0.0),
                    1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 20.0),
                    2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                    3 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
                    4 => *knee_db = (*knee_db + delta * 0.1).clamp(0.0, 12.0),
                    5 => *makeup_gain_db = (*makeup_gain_db + delta * 0.1).clamp(-20.0, 20.0),
                    6 => *mix = (*mix + delta * 0.01).clamp(0.0, 1.0),
                    7 => *auto_makeup = !*auto_makeup,
                    8 => *link_channels = !*link_channels,
                    9 => *sidechain_hpf_hz = (*sidechain_hpf_hz + delta).clamp(20.0, 500.0),
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => {
                match index {
                    0 => *threshold_db = (*threshold_db + delta * 0.1).clamp(-20.0, 0.0),
                    1 => *release_ms = (*release_ms + delta).clamp(1.0, 500.0),
                    2 => *lookahead_ms = (*lookahead_ms + delta * 0.1).clamp(0.0, 20.0),
                    3 => *soft = !*soft,
                    4 => *mix = (*mix + delta * 0.05).clamp(0.0, 1.0),
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                stereo_width,
                center_spread,
                surround_direct_bleed,
                rear_late_reflection,
                lfe_cutoff_hz,
                lfe_gain,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                subharmonic_freq_hz,
                subharmonic_attack_ms,
                subharmonic_release_ms,
                decorrelation_mode,
                decorrelation_lfo_rate_hz,
                velvet_noise_duration_ms,
                velvet_noise_density,
                enable_hr_direct,
                hr_sharpen,
                height_hf_cap_hz,
                height_transient_reduction,
                height_direct_leak,
                ambient_boost,
                safety_cap_db,
                rear_ambient_boost,
                dialogue_weight,
                voice_freq_min_hz,
                voice_freq_max_hz,
                bypass_decorrelation,
                bypass_transient_detection,
                bypass_all_processing,
            } => {
                use sotf_plugins::param_specs::upmixer::*;
                // Indices must match get_params() order
                match index {
                    0 => {
                        let configs = [
                            "2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4",
                            "9.1.4", "9.1.6",
                        ];
                        let current_idx = configs
                            .iter()
                            .position(|&c| c == speaker_config.as_str())
                            .unwrap_or(2);
                        let new_idx = if delta > 0.0 {
                            (current_idx + 1) % configs.len()
                        } else {
                            (current_idx + configs.len() - 1) % configs.len()
                        };
                        *speaker_config = configs[new_idx].to_string();
                    }
                    1 => *gain_front_direct = (*gain_front_direct + delta * 0.05).clamp(GAIN_FRONT_DIRECT_MIN as f64, GAIN_FRONT_DIRECT_MAX as f64),
                    2 => *gain_front_ambient = (*gain_front_ambient + delta * 0.05).clamp(GAIN_FRONT_AMBIENT_MIN as f64, GAIN_FRONT_AMBIENT_MAX as f64),
                    3 => *gain_rear_ambient = (*gain_rear_ambient + delta * 0.05).clamp(GAIN_REAR_AMBIENT_MIN as f64, GAIN_REAR_AMBIENT_MAX as f64),
                    4 => *height_gain = (*height_gain + delta * 0.05).clamp(GAIN_HEIGHT_MIN as f64, GAIN_HEIGHT_MAX as f64),
                    5 => *lfe_gain = (*lfe_gain + delta * 0.05).clamp(LFE_GAIN_MIN as f64, LFE_GAIN_MAX as f64),
                    6 => *lfe_cutoff_hz = (*lfe_cutoff_hz + delta * 5.0).clamp(LFE_CUTOFF_HZ_MIN as f64, LFE_CUTOFF_HZ_MAX as f64),
                    7 => *stereo_width = (*stereo_width + delta * 0.05).clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64),
                    8 => *center_spread = (*center_spread + delta * 0.05).clamp(CENTER_SPREAD_MIN as f64, CENTER_SPREAD_MAX as f64),
                    9 => *bandpass_hz = (*bandpass_hz + delta * 5.0).clamp(BANDPASS_HZ_MIN as f64, BANDPASS_HZ_MAX as f64),
                    10 => *enable_subharmonic_synth = !*enable_subharmonic_synth,
                    11 => *subharmonic_gain = (*subharmonic_gain + delta * 0.05).clamp(SUBHARMONIC_GAIN_MIN as f64, SUBHARMONIC_GAIN_MAX as f64),
                    12 => *enable_hr_direct = !*enable_hr_direct,
                    13 => *hr_sharpen = (*hr_sharpen + delta * 0.05).clamp(HR_SHARPEN_MIN as f64, HR_SHARPEN_MAX as f64),
                    14 => *safety_cap_db = (*safety_cap_db + delta * 0.1).clamp(SAFETY_CAP_DB_MIN as f64, SAFETY_CAP_DB_MAX as f64),
                    15 => *decorrelation_mode = (*decorrelation_mode + 1) % 2,
                    16 => *subharmonic_freq_hz = (*subharmonic_freq_hz + delta * 2.0).clamp(SUBHARMONIC_FREQ_HZ_MIN as f64, SUBHARMONIC_FREQ_HZ_MAX as f64),
                    17 => *subharmonic_attack_ms = (*subharmonic_attack_ms + delta * 2.0).clamp(SUBHARMONIC_ATTACK_MS_MIN as f64, SUBHARMONIC_ATTACK_MS_MAX as f64),
                    18 => *subharmonic_release_ms = (*subharmonic_release_ms + delta * 10.0).clamp(SUBHARMONIC_RELEASE_MS_MIN as f64, SUBHARMONIC_RELEASE_MS_MAX as f64),
                    19 => *decorrelation_lfo_rate_hz = (*decorrelation_lfo_rate_hz + delta * 0.02).clamp(DECORRELATION_LFO_RATE_HZ_MIN as f64, DECORRELATION_LFO_RATE_HZ_MAX as f64),
                    20 => *velvet_noise_duration_ms = (*velvet_noise_duration_ms + delta * 2.0).clamp(VELVET_NOISE_DURATION_MS_MIN as f64, VELVET_NOISE_DURATION_MS_MAX as f64),
                    21 => *velvet_noise_density = (*velvet_noise_density + delta * 100.0).clamp(VELVET_NOISE_DENSITY_MIN as f64, VELVET_NOISE_DENSITY_MAX as f64),
                    22 => *height_hf_cap_hz = (*height_hf_cap_hz + delta * 200.0).clamp(HEIGHT_HF_CAP_HZ_MIN as f64, HEIGHT_HF_CAP_HZ_MAX as f64),
                    23 => *height_transient_reduction = (*height_transient_reduction + delta * 0.05).clamp(HEIGHT_TRANSIENT_REDUCTION_MIN as f64, HEIGHT_TRANSIENT_REDUCTION_MAX as f64),
                    24 => *height_direct_leak = (*height_direct_leak + delta * 0.02).clamp(HEIGHT_DIRECT_LEAK_MIN as f64, HEIGHT_DIRECT_LEAK_MAX as f64),
                    25 => *surround_direct_bleed = (*surround_direct_bleed + delta * 0.05).clamp(SURROUND_DIRECT_BLEED_MIN as f64, SURROUND_DIRECT_BLEED_MAX as f64),
                    26 => *rear_ambient_boost = (*rear_ambient_boost + delta * 0.05).clamp(REAR_AMBIENT_BOOST_MIN as f64, REAR_AMBIENT_BOOST_MAX as f64),
                    27 => *rear_late_reflection = (*rear_late_reflection + delta * 0.02).clamp(REAR_LATE_REFLECTION_MIN as f64, REAR_LATE_REFLECTION_MAX as f64),
                    28 => *ambient_boost = (*ambient_boost + delta * 0.05).clamp(AMBIENT_BOOST_MIN as f64, AMBIENT_BOOST_MAX as f64),
                    29 => *dialogue_weight = (*dialogue_weight + delta * 0.05).clamp(DIALOGUE_WEIGHT_MIN as f64, DIALOGUE_WEIGHT_MAX as f64),
                    30 => *voice_freq_min_hz = (*voice_freq_min_hz + delta * 20.0).clamp(VOICE_FREQ_MIN_HZ_MIN as f64, VOICE_FREQ_MIN_HZ_MAX as f64),
                    31 => *voice_freq_max_hz = (*voice_freq_max_hz + delta * 100.0).clamp(VOICE_FREQ_MAX_HZ_MIN as f64, VOICE_FREQ_MAX_HZ_MAX as f64),
                    32 => *bypass_decorrelation = !*bypass_decorrelation,
                    33 => *bypass_transient_detection = !*bypass_transient_detection,
                    34 => *bypass_all_processing = !*bypass_all_processing,
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Gate {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => {
                match index {
                    0 => *threshold_db = (*threshold_db + delta).clamp(-60.0, 0.0),
                    1 => *ratio = (*ratio + delta * 0.1).clamp(1.0, 20.0),
                    2 => *attack_ms = (*attack_ms + delta * 0.1).clamp(0.1, 100.0),
                    3 => *hold_ms = (*hold_ms + delta).clamp(0.0, 500.0),
                    4 => *release_ms = (*release_ms + delta).clamp(1.0, 1000.0),
                    5 => *mix = (*mix + delta * 0.01).clamp(0.0, 1.0),
                    6 => *link_channels = !*link_channels,
                    7 => *sidechain_hpf_hz = (*sidechain_hpf_hz + delta).clamp(20.0, 500.0),
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
            } => {
                use sotf_plugins::param_specs::loudness_compensation::*;
                match index {
                    0 => *low_freq = (*low_freq + delta * 5.0).clamp(LOW_FREQ_MIN as f64, LOW_FREQ_MAX as f64),
                    1 => *low_gain = (*low_gain + delta * 0.5).clamp(LOW_GAIN_MIN as f64, LOW_GAIN_MAX as f64),
                    2 => *high_freq = (*high_freq + delta * 100.0).clamp(HIGH_FREQ_MIN as f64, HIGH_FREQ_MAX as f64),
                    3 => *high_gain = (*high_gain + delta * 0.5).clamp(HIGH_GAIN_MIN as f64, HIGH_GAIN_MAX as f64),
                    4 => *auto_gain_enabled = !*auto_gain_enabled,
                    5 => *auto_gain_max_db = (*auto_gain_max_db + delta).clamp(0.0, 24.0),
                    6 => *auto_gain_smoothing_ms = (*auto_gain_smoothing_ms + delta * 5.0).clamp(1.0, 1000.0),
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::BinauralDecoder {
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
                ..
            } => {
                match index {
                    0 => return false, // SOFA file - not adjustable with delta
                    1 => {
                        *input_channels =
                            (*input_channels as i64 + delta as i64).clamp(2, 16) as usize
                    }
                    2 => *enable_optimization = !*enable_optimization,
                    3 => *externalization = (*externalization + delta * 0.05).clamp(0.0, 1.0),
                    4 => {
                        *near_field_strength = (*near_field_strength + delta * 0.05).clamp(0.0, 1.0)
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Convolution { mix, gain_db, .. } => {
                match index {
                    0 => return false, // IR file - not adjustable with delta
                    1 => *mix = (*mix + delta * 0.05).clamp(0.0, 1.0),
                    2 => *gain_db = (*gain_db + delta * 0.5).clamp(-40.0, 40.0),
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Downmix {
                center_gain_db,
                surround_gain_db,
                height_gain_db,
                lfe_gain_db,
                phase_coherence,
                phase_blend_low_hz,
                phase_blend_high_hz,
                ..
            } => {
                use sotf_plugins::param_specs::downmix::*;
                match index {
                    0 => {
                        *center_gain_db = (*center_gain_db + delta * 0.5)
                            .clamp(CENTER_GAIN_DB_MIN as f64, CENTER_GAIN_DB_MAX as f64)
                    }
                    1 => {
                        *surround_gain_db = (*surround_gain_db + delta * 0.5)
                            .clamp(SURROUND_GAIN_DB_MIN as f64, SURROUND_GAIN_DB_MAX as f64)
                    }
                    2 => {
                        *height_gain_db = (*height_gain_db + delta * 0.5)
                            .clamp(HEIGHT_GAIN_DB_MIN as f64, HEIGHT_GAIN_DB_MAX as f64)
                    }
                    3 => {
                        *lfe_gain_db = (*lfe_gain_db + delta * 0.5)
                            .clamp(LFE_GAIN_DB_MIN as f64, LFE_GAIN_DB_MAX as f64)
                    }
                    4 => *phase_coherence = !*phase_coherence,
                    5 => {
                        *phase_blend_low_hz = (*phase_blend_low_hz + delta * 10.0)
                            .clamp(PHASE_BLEND_LOW_HZ_MIN as f64, PHASE_BLEND_LOW_HZ_MAX as f64)
                    }
                    6 => {
                        *phase_blend_high_hz = (*phase_blend_high_hz + delta * 10.0).clamp(
                            PHASE_BLEND_HIGH_HZ_MIN as f64,
                            PHASE_BLEND_HIGH_HZ_MAX as f64,
                        )
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::MonoToStereo {
                stereo_width,
                haas_delay_ms,
                enable_comp_eq,
                comp_eq_depth_db,
                decor_low_hz,
                decor_high_hz,
            } => {
                use sotf_plugins::param_specs::mono_to_stereo::*;
                match index {
                    0 => {
                        *stereo_width = (*stereo_width + delta * 0.05)
                            .clamp(STEREO_WIDTH_MIN as f64, STEREO_WIDTH_MAX as f64)
                    }
                    1 => {
                        *haas_delay_ms = (*haas_delay_ms + delta * 0.1)
                            .clamp(HAAS_DELAY_MS_MIN as f64, HAAS_DELAY_MS_MAX as f64)
                    }
                    2 => *enable_comp_eq = !*enable_comp_eq,
                    3 => {
                        *comp_eq_depth_db = (*comp_eq_depth_db + delta * 0.1)
                            .clamp(COMP_EQ_DEPTH_DB_MIN as f64, COMP_EQ_DEPTH_DB_MAX as f64)
                    }
                    4 => {
                        *decor_low_hz = (*decor_low_hz + delta * 10.0)
                            .clamp(DECOR_LOW_HZ_MIN as f64, DECOR_LOW_HZ_MAX as f64)
                    }
                    5 => {
                        *decor_high_hz = (*decor_high_hz + delta * 10.0)
                            .clamp(DECOR_HIGH_HZ_MIN as f64, DECOR_HIGH_HZ_MAX as f64)
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Denoiser {
                reduction_db,
                floor_db,
                smoothing,
                attack_ms,
                release_ms,
                low_latency,
                polyphonic_detection,
                transparency,
                dd_enabled,
                dd_alpha,
                psychoacoustic_masking,
                use_captured_profile,
            } => {
                use sotf_plugins::param_specs::denoiser::*;
                match index {
                    0 => {
                        *reduction_db = (*reduction_db + delta)
                            .clamp(REDUCTION_DB_MIN as f64, REDUCTION_DB_MAX as f64)
                    }
                    1 => {
                        *floor_db =
                            (*floor_db + delta).clamp(FLOOR_DB_MIN as f64, FLOOR_DB_MAX as f64)
                    }
                    2 => {
                        *smoothing = (*smoothing + delta * 0.01)
                            .clamp(SMOOTHING_MIN as f64, SMOOTHING_MAX as f64)
                    }
                    3 => {
                        *attack_ms = (*attack_ms + delta * 0.1)
                            .clamp(ATTACK_MS_MIN as f64, ATTACK_MS_MAX as f64)
                    }
                    4 => {
                        *release_ms = (*release_ms + delta)
                            .clamp(RELEASE_MS_MIN as f64, RELEASE_MS_MAX as f64)
                    }
                    5 => *low_latency = !*low_latency,
                    6 => *polyphonic_detection = !*polyphonic_detection,
                    7 => {
                        *transparency = (*transparency + delta * 0.05)
                            .clamp(TRANSPARENCY_MIN as f64, TRANSPARENCY_MAX as f64)
                    }
                    8 => *dd_enabled = !*dd_enabled,
                    9 => {
                        *dd_alpha = (*dd_alpha + delta * 0.01)
                            .clamp(DD_ALPHA_MIN as f64, DD_ALPHA_MAX as f64)
                    }
                    10 => *psychoacoustic_masking = !*psychoacoustic_masking,
                    11 => *use_captured_profile = !*use_captured_profile,
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::BandSplit {
                frequency,
                crossover_type,
                ..
            } => {
                use sotf_plugins::param_specs::band_split::*;
                match index {
                    0 => {
                        *frequency = (*frequency + delta * 10.0).clamp(FREQUENCY_MIN, FREQUENCY_MAX)
                    }
                    1 => {
                        *crossover_type = if crossover_type == "LR24" {
                            "LR48".to_string()
                        } else {
                            "LR24".to_string()
                        };
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::BandMerge { bands, .. } => {
                use sotf_plugins::param_specs::band_merge::*;
                match index {
                    0 => {
                        *bands = ((*bands as i64) + delta as i64)
                            .clamp(BANDS_MIN as i64, BANDS_MAX as i64)
                            as usize
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Expander {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => {
                use sotf_plugins::param_specs::expander::*;
                match index {
                    0 => {
                        *threshold_db = (*threshold_db + delta)
                            .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64)
                    }
                    1 => *ratio = (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64),
                    2 => {
                        *attack_ms =
                            (*attack_ms + delta * 0.1).clamp(ATTACK_MIN as f64, ATTACK_MAX as f64)
                    }
                    3 => {
                        *release_ms =
                            (*release_ms + delta).clamp(RELEASE_MIN as f64, RELEASE_MAX as f64)
                    }
                    4 => *range_db = (*range_db + delta).clamp(RANGE_MIN as f64, RANGE_MAX as f64),
                    5 => {
                        *knee_db = (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64)
                    }
                    6 => {
                        *hysteresis_db = (*hysteresis_db + delta * 0.1)
                            .clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64)
                    }
                    7 => *hold_ms = (*hold_ms + delta).clamp(HOLD_MIN as f64, HOLD_MAX as f64),
                    8 => *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64),
                    9 => *link_channels = !*link_channels,
                    10 => {
                        *sidechain_hpf_hz = (*sidechain_hpf_hz + delta)
                            .clamp(SIDECHAIN_HPF_HZ_MIN as f64, SIDECHAIN_HPF_HZ_MAX as f64)
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::MultibandCompressor {
                num_bands,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                mix,
                link_channels,
                bands,
                ..
            } => {
                use sotf_plugins::param_specs::multiband_compressor::*;
                const GLOBAL_COUNT: usize = 12;
                const BAND_PARAMS: usize = 8; // solo, bypass, threshold, ratio, attack, release, knee, makeup
                match index {
                    0 => {
                        let new_bands = ((*num_bands as i64) + delta as i64)
                            .clamp(NUM_BANDS_MIN as i64, NUM_BANDS_MAX as i64)
                            as usize;
                        *num_bands = new_bands;
                        // Resize bands vector to match
                        bands.resize_with(new_bands, Default::default);
                    }
                    1 => {
                        *crossover_freq_1 = (*crossover_freq_1 + delta * 5.0)
                            .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64)
                    }
                    2 => {
                        *crossover_freq_2 = (*crossover_freq_2 + delta * 10.0)
                            .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64)
                    }
                    3 => {
                        *crossover_freq_3 = (*crossover_freq_3 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64)
                    }
                    4 => {
                        *crossover_freq_4 = (*crossover_freq_4 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64)
                    }
                    5 => {
                        *threshold_db = (*threshold_db + delta)
                            .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64)
                    }
                    6 => *ratio = (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64),
                    7 => {
                        *attack_ms =
                            (*attack_ms + delta * 0.1).clamp(ATTACK_MIN as f64, ATTACK_MAX as f64)
                    }
                    8 => {
                        *release_ms =
                            (*release_ms + delta).clamp(RELEASE_MIN as f64, RELEASE_MAX as f64)
                    }
                    9 => {
                        *knee_db = (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64)
                    }
                    10 => *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64),
                    11 => *link_channels = !*link_channels,
                    _ => {
                        // Per-band parameters
                        let band_offset = index - GLOBAL_COUNT;
                        let band_idx = band_offset / BAND_PARAMS;
                        let param_in_band = band_offset % BAND_PARAMS;
                        // Ensure bands vec is large enough
                        if band_idx >= bands.len() {
                            bands.resize_with(band_idx + 1, Default::default);
                        }
                        let band = &mut bands[band_idx];
                        match param_in_band {
                            0 => band.solo = !band.solo,
                            1 => band.bypass = !band.bypass,
                            2 => { // threshold: toggle between Global/override
                                band.threshold_db = match band.threshold_db {
                                    None => Some(*threshold_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < THRESHOLD_MIN { None } else { Some(new_v.clamp(THRESHOLD_MIN, THRESHOLD_MAX)) }
                                    }
                                };
                            }
                            3 => { // ratio
                                band.ratio = match band.ratio {
                                    None => Some(*ratio as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < RATIO_MIN { None } else { Some(new_v.clamp(RATIO_MIN, RATIO_MAX)) }
                                    }
                                };
                            }
                            4 => { // attack
                                band.attack_ms = match band.attack_ms {
                                    None => Some(*attack_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < ATTACK_MIN { None } else { Some(new_v.clamp(ATTACK_MIN, ATTACK_MAX)) }
                                    }
                                };
                            }
                            5 => { // release
                                band.release_ms = match band.release_ms {
                                    None => Some(*release_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < RELEASE_MIN { None } else { Some(new_v.clamp(RELEASE_MIN, RELEASE_MAX)) }
                                    }
                                };
                            }
                            6 => { // knee
                                band.knee_db = match band.knee_db {
                                    None => Some(*knee_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < KNEE_MIN { None } else { Some(new_v.clamp(KNEE_MIN, KNEE_MAX)) }
                                    }
                                };
                            }
                            7 => { // makeup gain
                                band.makeup_gain_db = (band.makeup_gain_db + delta as f32 * 0.5).clamp(-24.0, 24.0);
                            }
                            _ => return false,
                        }
                    }
                }
                return true;
            }
            sotf_audio_player::PluginSettings::MultibandExpander {
                num_bands,
                crossover_freq_1,
                crossover_freq_2,
                crossover_freq_3,
                crossover_freq_4,
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                range_db,
                knee_db,
                hysteresis_db,
                hold_ms,
                mix,
                link_channels,
                bands,
                ..
            } => {
                use sotf_plugins::param_specs::multiband_expander::*;
                const GLOBAL_COUNT: usize = 15;
                const BAND_PARAMS: usize = 10; // solo, bypass, threshold, ratio, attack, release, range, knee, hysteresis, hold
                match index {
                    0 => {
                        let new_bands = ((*num_bands as i64) + delta as i64)
                            .clamp(NUM_BANDS_MIN as i64, NUM_BANDS_MAX as i64)
                            as usize;
                        *num_bands = new_bands;
                        bands.resize_with(new_bands, Default::default);
                    }
                    1 => {
                        *crossover_freq_1 = (*crossover_freq_1 + delta * 5.0)
                            .clamp(CROSSOVER_FREQ_1_MIN as f64, CROSSOVER_FREQ_1_MAX as f64)
                    }
                    2 => {
                        *crossover_freq_2 = (*crossover_freq_2 + delta * 10.0)
                            .clamp(CROSSOVER_FREQ_2_MIN as f64, CROSSOVER_FREQ_2_MAX as f64)
                    }
                    3 => {
                        *crossover_freq_3 = (*crossover_freq_3 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_3_MIN as f64, CROSSOVER_FREQ_3_MAX as f64)
                    }
                    4 => {
                        *crossover_freq_4 = (*crossover_freq_4 + delta * 50.0)
                            .clamp(CROSSOVER_FREQ_4_MIN as f64, CROSSOVER_FREQ_4_MAX as f64)
                    }
                    5 => {
                        *threshold_db = (*threshold_db + delta)
                            .clamp(THRESHOLD_MIN as f64, THRESHOLD_MAX as f64)
                    }
                    6 => *ratio = (*ratio + delta * 0.1).clamp(RATIO_MIN as f64, RATIO_MAX as f64),
                    7 => {
                        *attack_ms =
                            (*attack_ms + delta * 0.1).clamp(ATTACK_MIN as f64, ATTACK_MAX as f64)
                    }
                    8 => {
                        *release_ms =
                            (*release_ms + delta).clamp(RELEASE_MIN as f64, RELEASE_MAX as f64)
                    }
                    9 => *range_db = (*range_db + delta).clamp(RANGE_MIN as f64, RANGE_MAX as f64),
                    10 => {
                        *knee_db = (*knee_db + delta * 0.1).clamp(KNEE_MIN as f64, KNEE_MAX as f64)
                    }
                    11 => {
                        *hysteresis_db = (*hysteresis_db + delta * 0.1)
                            .clamp(HYSTERESIS_MIN as f64, HYSTERESIS_MAX as f64)
                    }
                    12 => *hold_ms = (*hold_ms + delta).clamp(HOLD_MIN as f64, HOLD_MAX as f64),
                    13 => *mix = (*mix + delta * 0.01).clamp(MIX_MIN as f64, MIX_MAX as f64),
                    14 => *link_channels = !*link_channels,
                    _ => {
                        // Per-band parameters
                        let band_offset = index - GLOBAL_COUNT;
                        let band_idx = band_offset / BAND_PARAMS;
                        let param_in_band = band_offset % BAND_PARAMS;
                        if band_idx >= bands.len() {
                            bands.resize_with(band_idx + 1, Default::default);
                        }
                        let band = &mut bands[band_idx];
                        match param_in_band {
                            0 => band.solo = !band.solo,
                            1 => band.bypass = !band.bypass,
                            2 => { // threshold
                                band.threshold_db = match band.threshold_db {
                                    None => Some(*threshold_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < THRESHOLD_MIN { None } else { Some(new_v.clamp(THRESHOLD_MIN, THRESHOLD_MAX)) }
                                    }
                                };
                            }
                            3 => { // ratio
                                band.ratio = match band.ratio {
                                    None => Some(*ratio as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < RATIO_MIN { None } else { Some(new_v.clamp(RATIO_MIN, RATIO_MAX)) }
                                    }
                                };
                            }
                            4 => { // attack
                                band.attack_ms = match band.attack_ms {
                                    None => Some(*attack_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < ATTACK_MIN { None } else { Some(new_v.clamp(ATTACK_MIN, ATTACK_MAX)) }
                                    }
                                };
                            }
                            5 => { // release
                                band.release_ms = match band.release_ms {
                                    None => Some(*release_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < RELEASE_MIN { None } else { Some(new_v.clamp(RELEASE_MIN, RELEASE_MAX)) }
                                    }
                                };
                            }
                            6 => { // range
                                band.range_db = match band.range_db {
                                    None => Some(*range_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < RANGE_MIN { None } else { Some(new_v.clamp(RANGE_MIN, RANGE_MAX)) }
                                    }
                                };
                            }
                            7 => { // knee
                                band.knee_db = match band.knee_db {
                                    None => Some(*knee_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < KNEE_MIN { None } else { Some(new_v.clamp(KNEE_MIN, KNEE_MAX)) }
                                    }
                                };
                            }
                            8 => { // hysteresis
                                band.hysteresis_db = match band.hysteresis_db {
                                    None => Some(*hysteresis_db as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32 * 0.1;
                                        if new_v < HYSTERESIS_MIN { None } else { Some(new_v.clamp(HYSTERESIS_MIN, HYSTERESIS_MAX)) }
                                    }
                                };
                            }
                            9 => { // hold
                                band.hold_ms = match band.hold_ms {
                                    None => Some(*hold_ms as f32),
                                    Some(v) => {
                                        let new_v = v + delta as f32;
                                        if new_v < HOLD_MIN { None } else { Some(new_v.clamp(HOLD_MIN, HOLD_MAX)) }
                                    }
                                };
                            }
                            _ => return false,
                        }
                    }
                }
                return true;
            }
            sotf_audio_player::PluginSettings::XTC {
                distance_m,
                speaker_angle_deg,
                head_radius_m,
                beta_base,
                beta_low_freq_boost,
                beta_high_freq_boost,
                head_shadow_cutoff_hz,
                head_shadow_slope_db_per_octave,
            } => {
                use sotf_plugins::param_specs::xtc::*;
                match index {
                    0 => {
                        *distance_m =
                            (*distance_m + delta * 0.1).clamp(DISTANCE_M_MIN, DISTANCE_M_MAX)
                    }
                    1 => {
                        *speaker_angle_deg = (*speaker_angle_deg + delta)
                            .clamp(SPEAKER_ANGLE_DEG_MIN, SPEAKER_ANGLE_DEG_MAX)
                    }
                    2 => {
                        *head_radius_m = (*head_radius_m + delta * 0.001)
                            .clamp(HEAD_RADIUS_M_MIN, HEAD_RADIUS_M_MAX)
                    }
                    3 => {
                        *beta_base =
                            (*beta_base + delta * 0.0001).clamp(BETA_BASE_MIN, BETA_BASE_MAX)
                    }
                    4 => {
                        *beta_low_freq_boost = (*beta_low_freq_boost + delta)
                            .clamp(BETA_LOW_FREQ_BOOST_MIN, BETA_LOW_FREQ_BOOST_MAX)
                    }
                    5 => {
                        *beta_high_freq_boost = (*beta_high_freq_boost + delta)
                            .clamp(BETA_HIGH_FREQ_BOOST_MIN, BETA_HIGH_FREQ_BOOST_MAX)
                    }
                    6 => {
                        *head_shadow_cutoff_hz = (*head_shadow_cutoff_hz + delta * 100.0)
                            .clamp(HEAD_SHADOW_CUTOFF_HZ_MIN, HEAD_SHADOW_CUTOFF_HZ_MAX)
                    }
                    7 => {
                        *head_shadow_slope_db_per_octave =
                            (*head_shadow_slope_db_per_octave + delta * 0.5).clamp(
                                HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MIN,
                                HEAD_SHADOW_SLOPE_DB_PER_OCTAVE_MAX,
                            )
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Pnd {
                correction_strength,
                analysis_window_ms,
                drift_smoothing,
            } => {
                use sotf_plugins::param_specs::pnd::*;
                match index {
                    0 => {
                        *correction_strength = (*correction_strength + delta * 0.05).clamp(
                            CORRECTION_STRENGTH_MIN as f64,
                            CORRECTION_STRENGTH_MAX as f64,
                        )
                    }
                    1 => {
                        *analysis_window_ms = (*analysis_window_ms + delta * 5.0)
                            .clamp(ANALYSIS_WINDOW_MS_MIN as f64, ANALYSIS_WINDOW_MS_MAX as f64)
                    }
                    2 => {
                        *drift_smoothing = (*drift_smoothing + delta * 0.01)
                            .clamp(DRIFT_SMOOTHING_MIN as f64, DRIFT_SMOOTHING_MAX as f64)
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::ABCompare {
                mix,
                mix_mode,
                selected_path,
                bypass,
                auto_gain_enabled,
                loudness_type,
                max_auto_gain_db,
                gain_smoothing_ms,
                mix_transition_ms,
                path_a_config: _,
                path_b_config: _,
            } => {
                use sotf_plugins::param_specs::ab_compare::*;
                match index {
                    0 => *mix = (*mix + delta * 0.05).clamp(MIX_MIN, MIX_MAX),
                    1 => *mix_mode = if *mix_mode == 0 { 1 } else { 0 },
                    2 => *selected_path = if *selected_path == 0 { 1 } else { 0 },
                    3 => *bypass = !*bypass,
                    4 => *auto_gain_enabled = !*auto_gain_enabled,
                    5 => *loudness_type = if *loudness_type == 0 { 1 } else { 0 },
                    6 => {
                        *max_auto_gain_db = (*max_auto_gain_db + delta)
                            .clamp(MAX_AUTO_GAIN_DB_MIN, MAX_AUTO_GAIN_DB_MAX)
                    }
                    7 => {
                        *gain_smoothing_ms = (*gain_smoothing_ms + delta * 5.0)
                            .clamp(GAIN_SMOOTHING_MS_MIN, GAIN_SMOOTHING_MS_MAX)
                    }
                    8 => {
                        *mix_transition_ms = (*mix_transition_ms + delta * 5.0)
                            .clamp(MIX_TRANSITION_MS_MIN, MIX_TRANSITION_MS_MAX)
                    }
                    9 | 10 => return false, // path_a/path_b configs are strings, not adjustable with delta
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::SpectrumAnalyzer {
                num_bins,
                min_freq,
                max_freq,
                smoothing,
                tilt_correction,
                tilt_reference,
            } => {
                use sotf_plugins::param_specs::spectrum::*;
                use sotf_plugins::{SpectralTiltCorrection, TiltReferenceFreq};
                match index {
                    0 => {
                        *num_bins = ((*num_bins as i64) + delta as i64)
                            .clamp(NUM_BINS_MIN as i64, NUM_BINS_MAX as i64)
                            as usize
                    }
                    1 => *min_freq = (*min_freq + delta as f32).clamp(MIN_FREQ_MIN, MIN_FREQ_MAX),
                    2 => {
                        *max_freq =
                            (*max_freq + delta as f32 * 100.0).clamp(MAX_FREQ_MIN, MAX_FREQ_MAX)
                    }
                    3 => {
                        *smoothing =
                            (*smoothing + delta as f32 * 0.01).clamp(SMOOTHING_MIN, SMOOTHING_MAX)
                    }
                    4 => {
                        // Cycle through tilt correction modes
                        let modes = [
                            SpectralTiltCorrection::None,
                            SpectralTiltCorrection::ThreeDbPerOctave,
                            SpectralTiltCorrection::SixDbPerOctave,
                            SpectralTiltCorrection::Pink,
                        ];
                        let current = modes.iter().position(|m| m == tilt_correction).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % modes.len()
                        } else {
                            (current + modes.len() - 1) % modes.len()
                        };
                        *tilt_correction = modes[next];
                    }
                    5 => {
                        // Cycle through tilt reference modes
                        let modes = [
                            TiltReferenceFreq::Standard,
                            TiltReferenceFreq::OneKilohertz,
                            TiltReferenceFreq::TwoKilohertz,
                            TiltReferenceFreq::MinFreq,
                        ];
                        let current = modes.iter().position(|m| m == tilt_reference).unwrap_or(0);
                        let next = if delta > 0.0 {
                            (current + 1) % modes.len()
                        } else {
                            (current + modes.len() - 1) % modes.len()
                        };
                        *tilt_reference = modes[next];
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::LoudnessMonitor => return false,
            sotf_audio_player::PluginSettings::ChannelMuteSolo { enabled, .. } => {
                match index {
                    0 => *enabled = !*enabled,
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } => {
                use sotf_plugins::param_specs::hal::*;
                match index {
                    0 => {
                        *input_channels = ((*input_channels as i64) + delta as i64)
                            .clamp(CHANNELS_MIN as i64, CHANNELS_MAX as i64)
                            as usize
                    }
                    1 => {
                        *output_channels = ((*output_channels as i64) + delta as i64)
                            .clamp(CHANNELS_MIN as i64, CHANNELS_MAX as i64)
                            as usize
                    }
                    _ => return false,
                }
                return true;
            }
            sotf_audio_player::PluginSettings::FletcherMunson {
                reference_level_db,
                enabled,
                smoothing_ms,
                auto_gain_enabled,
                auto_gain_max_db,
                auto_gain_smoothing_ms,
                ..
            } => {
                use sotf_plugins::param_specs::fletcher_munson::*;
                match index {
                    0 => {
                        *reference_level_db = (*reference_level_db + delta)
                            .clamp(REFERENCE_LEVEL_DB_MIN as f64, REFERENCE_LEVEL_DB_MAX as f64)
                    }
                    1 => *enabled = !*enabled,
                    2 => {
                        *smoothing_ms = (*smoothing_ms + delta)
                            .clamp(SMOOTHING_MS_MIN as f64, SMOOTHING_MS_MAX as f64)
                    }
                    3 => *auto_gain_enabled = !*auto_gain_enabled,
                    4 => {
                        *auto_gain_max_db = (*auto_gain_max_db + delta)
                            .clamp(AUTO_GAIN_MAX_DB_MIN as f64, AUTO_GAIN_MAX_DB_MAX as f64)
                    }
                    5 => {
                        *auto_gain_smoothing_ms = (*auto_gain_smoothing_ms + delta * 5.0).clamp(
                            AUTO_GAIN_SMOOTHING_MS_MIN as f64,
                            AUTO_GAIN_SMOOTHING_MS_MAX as f64,
                        )
                    }
                    _ => return false,
                }
                return true;
            }
        }
        false
    }
}

#[derive(Debug)]
pub struct App {
    pub library: MusicLibrary,
    pub queue: Vec<QueueEntry>,
    pub current_screen: Screen,
    pub input_mode: InputMode,
    pub focused_pane: FocusedPane, // Which pane (Main or Meters) has focus

    // Theme
    pub theme: Theme,

    // UI state
    pub search_query: String,
    pub directory_input: String,
    pub plugin_file_input: String, // For save/load plugin chain
    pub apo_file_input: String,    // For loading APO EQ files
    pub sofa_file_input: String,   // For loading SOFA HRTF files
    pub selected_album_index: usize,
    pub selected_directory_index: usize,
    pub selected_queue_index: usize,
    pub selected_queue_track_index: Option<usize>, // None = album header, Some(i) = track i
    pub selected_plugin_index: usize,
    pub add_plugin_selected_index: usize, // For plugin add dialog
    pub album_list_offset: usize,
    pub status_message: Option<String>, // For displaying save/load status
    pub error_message: Option<String>,  // For displaying decode/playback errors in a modal

    // Cached filtered results
    pub cached_filtered_albums: Vec<Album>,
    pub needs_filter_update: bool,

    // Autocomplete state
    pub autocomplete_suggestions: Vec<String>,
    pub autocomplete_index: usize,

    // Plugin preset selection
    pub available_plugin_presets: Vec<String>, // List of preset filenames
    pub selected_preset_index: usize,

    // Library tree view
    pub library_view_mode: LibraryViewMode,
    pub library_sort_order: LibrarySortOrder,
    pub channel_filter: ChannelFilter,
    pub artist_tree: Vec<ArtistNode>,
    pub selected_tree_index: usize, // Index in flattened tree (artists + visible albums)

    // Plugin system
    pub plugin_chain: PluginChain,
    pub needs_plugin_update: bool,
    pub pending_param_update: Option<PendingParameterUpdate>,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize, // Which parameter is selected in edit mode
    pub plugin_update_last_attempt: Option<std::time::Instant>,
    pub plugin_update_retry_count: u32,
    pub plugin_update_in_progress: bool,

    // Matrix editor state
    pub matrix_edit_mode: MatrixEditMode, // Header (channels/preset) or Grid (cells)
    pub matrix_grid_row: usize,           // Selected output row in grid
    pub matrix_grid_col: usize,           // Selected input column in grid
    pub matrix_header_selection: usize,   // 0 = Input Channels, 1 = Output Channels, 2 = Preset

    // Playback state
    pub is_playing: bool,
    pub current_queue_index: Option<usize>,
    pub volume: f32,
    pub position_secs: f64,
    pub current_sample_rate: Option<u32>, // Actual playback rate from engine

    // Play tracking for statistics (30s threshold)
    pub current_track_path: Option<PathBuf>,
    pub current_track_start_time: Option<std::time::Instant>,
    pub current_track_already_recorded: bool,

    // Loudness monitoring
    pub loudness_info: Option<LoudnessData>,

    // Level meters
    pub level_meter_groups: Vec<ChannelGroup>,
    pub selected_level_meter_group: usize,
    pub level_meter_control_selection: usize, // 0 = Mute, 1 = Solo, 2 = Dim
    /// Cached channel count to avoid rebuilding meter groups every frame
    pub level_meter_last_channel_count: usize,
    /// Cached speaker config to avoid rebuilding meter groups every frame
    pub level_meter_last_speaker_config: Option<String>,

    // Audio devices
    pub output_devices: Vec<AudioDevice>,
    pub selected_output_device_index: usize,
    pub current_output_device_name: Option<String>,

    // Flags
    pub should_quit: bool,
    pub needs_rescan: bool,
    pub needs_redraw: bool,

    // Scan progress
    pub scan_in_progress: bool,
    pub scan_progress_tracks: usize,
    pub scan_progress_albums: usize,
    pub library_scanner: Option<sotf_audio_player::LibraryScanner>,

    // Maintenance progress
    pub maintenance_in_progress: bool,
    pub maintenance_progress_checked: usize,
    pub maintenance_progress_total: usize,

    // ReplayGain scanner manager
    pub replay_gain_manager: sotf_audio_player::ReplayGainScanManager,

    // Waveform scanner progress
    // Waveform scanner manager
    pub waveform_manager: sotf_audio_player::WaveformScanManager,

    // Last loaded plugin preset name (for config persistence)
    pub last_loaded_preset: Option<String>,

    // File browser state
    pub file_browser_items: Vec<PathBuf>,
    pub selected_file_index: usize,
    pub current_browser_dir: PathBuf,
    pub file_browser_extension: Option<String>, // Filter by extension (.sofa, .wav)

    // Album cover image display
    pub album_images: Vec<PathBuf>, // List of image files in current album directory
    pub selected_image_index: usize, // Current image being displayed
    pub image_picker: Option<ratatui_image::picker::Picker>, // Image protocol picker
}

impl App {
    pub fn new(theme: Theme) -> Self {
        // Try to create library with database, fallback to simple library
        let library = MusicLibrary::with_database().unwrap_or_else(|e| {
            log::warn!(
                "Failed to initialize database, using in-memory library: {}",
                e
            );
            MusicLibrary::new()
        });

        Self {
            library,
            queue: Vec::new(),
            current_screen: Screen::Library,
            input_mode: InputMode::Normal,
            focused_pane: FocusedPane::Main,
            theme,
            search_query: String::new(),
            directory_input: String::new(),
            plugin_file_input: String::new(),
            apo_file_input: String::new(),
            sofa_file_input: String::new(),
            selected_album_index: 0,
            selected_directory_index: 0,
            selected_queue_index: 0,
            selected_queue_track_index: None,
            selected_plugin_index: 0,
            add_plugin_selected_index: 0,
            album_list_offset: 0,
            status_message: None,
            error_message: None,
            cached_filtered_albums: Vec::new(),
            needs_filter_update: true,
            autocomplete_suggestions: Vec::new(),
            autocomplete_index: 0,
            available_plugin_presets: Vec::new(),
            selected_preset_index: 0,
            library_view_mode: LibraryViewMode::Flat,
            library_sort_order: LibrarySortOrder::Artist,
            channel_filter: ChannelFilter::All,
            artist_tree: Vec::new(),
            selected_tree_index: 0,
            plugin_chain: {
                let mut chain = PluginChain::with_default_rack();
                // Add ChannelMuteSolo plugin before the permanent tail (disabled by default)
                let idx = chain.user_plugin_insert_index();
                chain.insert_plugin(idx, &PluginType::ChannelMuteSolo);
                chain
            },
            needs_plugin_update: false,
            pending_param_update: None,
            editing_plugin_index: None,
            plugin_param_selection: 0,
            plugin_update_last_attempt: None,
            plugin_update_retry_count: 0,
            plugin_update_in_progress: false,
            matrix_edit_mode: MatrixEditMode::Header,
            matrix_grid_row: 0,
            matrix_grid_col: 0,
            matrix_header_selection: 0,
            is_playing: false,
            current_queue_index: None,
            volume: 0.1, // Start at 10% volume
            position_secs: 0.0,
            current_sample_rate: None,
            current_track_path: None,
            current_track_start_time: None,
            current_track_already_recorded: false,
            loudness_info: None,
            level_meter_groups: Vec::new(),
            selected_level_meter_group: 0,
            level_meter_control_selection: 0,
            level_meter_last_channel_count: 0,
            level_meter_last_speaker_config: None,
            output_devices: Vec::new(),
            selected_output_device_index: 0,
            current_output_device_name: None,
            should_quit: false,
            needs_rescan: false,
            needs_redraw: true,
            scan_in_progress: false,
            scan_progress_tracks: 0,
            scan_progress_albums: 0,
            library_scanner: None,
            maintenance_in_progress: false,
            maintenance_progress_checked: 0,
            maintenance_progress_total: 0,
            replay_gain_manager: sotf_audio_player::ReplayGainScanManager::new(),
            waveform_manager: sotf_audio_player::WaveformScanManager::new(),
            last_loaded_preset: None,
            file_browser_items: Vec::new(),
            selected_file_index: 0,
            current_browser_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            file_browser_extension: None,
            album_images: Vec::new(),
            selected_image_index: 0,
            image_picker: None,
        }
    }

    /// Load library from database if available
    pub fn load_library_from_database(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.library.load_from_database()?;
        self.rebuild_artist_tree();
        // Update last scan times for directories from database
        self.update_directory_scan_times();
        Ok(())
    }

    /// Update directory scan times from database
    fn update_directory_scan_times(&mut self) {
        self.library.update_directory_scan_times();
    }

    pub fn load_output_devices(&mut self) {
        // Load available output devices
        if let Ok(devices_map) = sotf_audio::devices::get_audio_devices()
            && let Some(output_devices) = devices_map.get("output")
        {
            self.output_devices = output_devices.clone();
            // Find the default device
            if let Some(default_idx) = output_devices.iter().position(|d| d.is_default) {
                self.selected_output_device_index = default_idx;
                self.current_output_device_name = output_devices[default_idx].name.clone().into();
            }
        }
    }

    pub fn select_next_output_device(&mut self) {
        if !self.output_devices.is_empty() {
            self.selected_output_device_index =
                (self.selected_output_device_index + 1) % self.output_devices.len();
        }
    }

    pub fn select_previous_output_device(&mut self) {
        if !self.output_devices.is_empty() {
            if self.selected_output_device_index == 0 {
                self.selected_output_device_index = self.output_devices.len() - 1;
            } else {
                self.selected_output_device_index -= 1;
            }
        }
    }

    pub fn get_selected_output_device(&self) -> Option<&AudioDevice> {
        self.output_devices.get(self.selected_output_device_index)
    }

    /// Get the maximum output channels supported by the selected device
    pub fn get_device_max_channels(&self) -> Option<usize> {
        self.get_selected_output_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.channels as usize)
    }

    /// Get current device sample rate or fallback to 48kHz
    pub fn get_current_sample_rate(&self) -> f64 {
        self.get_selected_output_device()
            .and_then(|device| device.default_config.as_ref())
            .map(|config| config.sample_rate as f64)
            .unwrap_or(48000.0)
    }

    /// Get the target sample rate for a track, accounting for device capabilities
    pub fn get_target_sample_rate(&self, track_sample_rate: u32) -> f64 {
        sotf_audio::select_output_sample_rate(
            track_sample_rate,
            self.current_output_device_name.as_deref(),
        ) as f64
    }

    /// Get filtered albums, using cache if available
    pub fn filtered_albums(&mut self) -> &[Album] {
        if self.needs_filter_update {
            self.cached_filtered_albums = self.library.get_filtered_albums(
                &self.search_query,
                self.library_sort_order,
                self.channel_filter,
            );
            self.needs_filter_update = false;
        }
        &self.cached_filtered_albums
    }

    /// Mark filtered albums cache as dirty
    pub fn request_filter_update(&mut self) {
        self.needs_filter_update = true;
    }

    pub fn add_album_to_queue(&mut self) -> Option<PathBuf> {
        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.is_playing;

        // Use a local copy to avoid borrow issues while mutating queue
        let index = self.selected_album_index;
        let album = self.filtered_albums().get(index)?.clone();

        // Remove any existing entry for the same album (by artist + title)
        let artist = album.artist();
        let title = &album.title;
        let removed_was_current = self.remove_duplicate_album(&artist, title);

        self.queue.push(QueueEntry::new(QueueItem::new(album)));

        // Auto-play if queue was empty, nothing was playing, or we removed the currently playing album
        if was_empty || was_not_playing || removed_was_current {
            return self.start_queue();
        }
        None
    }

    /// Remove an album from the queue by artist + title match.
    /// Returns true if the removed entry was the currently playing one.
    fn remove_duplicate_album(&mut self, artist: &str, title: &str) -> bool {
        if let Some(pos) = self
            .queue
            .iter()
            .position(|e| e.item.album.artist() == artist && e.item.album.title == title)
        {
            self.queue.remove(pos);
            let was_current = self.current_queue_index == Some(pos);

            // Adjust current_queue_index after removal
            if let Some(idx) = self.current_queue_index {
                if pos < idx {
                    self.current_queue_index = Some(idx - 1);
                } else if pos == idx {
                    // Currently playing album was removed; will be re-added at end
                    self.current_queue_index = None;
                }
            }

            // Adjust selected_queue_index after removal
            if pos < self.selected_queue_index && self.selected_queue_index > 0 {
                self.selected_queue_index -= 1;
            }

            was_current
        } else {
            false
        }
    }

    pub fn remove_from_queue(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);

            // Adjust current queue index if needed
            if let Some(current_idx) = self.current_queue_index {
                if current_idx == index {
                    // We deleted the currently playing album
                    if self.queue.is_empty() {
                        // Queue is now empty
                        self.current_queue_index = None;
                        self.is_playing = false;
                    } else if index < self.queue.len() {
                        // There are albums after the deleted one, stay at same index
                        // (items have shifted down, so index now points to the next album)
                        self.current_queue_index = Some(index);
                        // Reset to first track of the new album at this position
                        if let Some(entry) = self.queue.get_mut(index) {
                            entry.item.current_track_index = 0;
                        }
                    } else if index > 0 {
                        // Deleted last album, move to previous album
                        self.current_queue_index = Some(index - 1);
                        // Stay on whatever track was playing in that album
                    } else {
                        // Queue is empty
                        self.current_queue_index = None;
                        self.is_playing = false;
                    }
                } else if current_idx > index {
                    // Deleted an album before the current one, adjust index
                    self.current_queue_index = Some(current_idx - 1);
                }
            }
            if self.selected_queue_index >= self.queue.len() && self.selected_queue_index > 0 {
                self.selected_queue_index = self.queue.len() - 1;
            }
            self.selected_queue_track_index = None;
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
        self.current_queue_index = None;
        self.selected_queue_index = 0;
        self.selected_queue_track_index = None;
        self.is_playing = false;
    }

    pub fn toggle_queue_item_expansion(&mut self) {
        if let Some(entry) = self.queue.get_mut(self.selected_queue_index) {
            entry.expanded = !entry.expanded;
            if !entry.expanded {
                self.selected_queue_track_index = None;
            }
        }
    }

    pub fn expand_queue_item(&mut self) {
        if let Some(entry) = self.queue.get_mut(self.selected_queue_index) {
            entry.expanded = true;
        }
    }

    pub fn collapse_queue_item(&mut self) {
        if self.selected_queue_track_index.is_some() {
            // On a track: move back to album header
            self.selected_queue_track_index = None;
        } else if let Some(entry) = self.queue.get_mut(self.selected_queue_index) {
            entry.expanded = false;
        }
    }

    pub fn select_next_album(&mut self) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_album_index = (self.selected_album_index + 1) % count;
        }
    }

    pub fn select_previous_album(&mut self) {
        let count = self.filtered_albums().len();
        if count > 0 {
            if self.selected_album_index == 0 {
                self.selected_album_index = count - 1;
            } else {
                self.selected_album_index -= 1;
            }
        }
    }

    pub fn page_down_albums(&mut self, page_size: usize) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_album_index = (self.selected_album_index + page_size).min(count - 1);
        }
    }

    pub fn page_up_albums(&mut self, page_size: usize) {
        let count = self.filtered_albums().len();
        if count > 0 {
            self.selected_album_index = self.selected_album_index.saturating_sub(page_size);
        }
    }

    pub fn page_down_tree(&mut self, page_size: usize) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.selected_tree_index =
                (self.selected_tree_index + page_size).min(tree_items.len() - 1);
        }
    }

    pub fn page_up_tree(&mut self, page_size: usize) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.selected_tree_index = self.selected_tree_index.saturating_sub(page_size);
        }
    }

    pub fn select_next_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.selected_directory_index = (self.selected_directory_index + 1) % tree_items.len();
        }
    }

    pub fn select_previous_directory(&mut self) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            if self.selected_directory_index == 0 {
                self.selected_directory_index = tree_items.len() - 1;
            } else {
                self.selected_directory_index -= 1;
            }
        }
    }

    pub fn page_down_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.selected_directory_index =
                (self.selected_directory_index + page_size).min(tree_items.len() - 1);
        }
    }

    pub fn page_up_directories(&mut self, page_size: usize) {
        let tree_items = self.get_directory_tree_items();
        if !tree_items.is_empty() {
            self.selected_directory_index = self.selected_directory_index.saturating_sub(page_size);
        }
    }

    pub fn select_next_queue_item(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        let entry = &self.queue[self.selected_queue_index];
        if entry.expanded {
            match self.selected_queue_track_index {
                None => {
                    // On album header of expanded album → move to first track
                    self.selected_queue_track_index = Some(0);
                }
                Some(ti) if ti + 1 < entry.item.album.tracks.len() => {
                    // Move to next track within album
                    self.selected_queue_track_index = Some(ti + 1);
                }
                Some(_) => {
                    // Past last track → move to next album header
                    self.selected_queue_track_index = None;
                    self.selected_queue_index =
                        (self.selected_queue_index + 1) % self.queue.len();
                }
            }
        } else {
            // Collapsed album → move to next album
            self.selected_queue_track_index = None;
            self.selected_queue_index = (self.selected_queue_index + 1) % self.queue.len();
        }
    }

    pub fn select_previous_queue_item(&mut self) {
        if self.queue.is_empty() {
            return;
        }

        match self.selected_queue_track_index {
            Some(0) => {
                // First track → move back to album header
                self.selected_queue_track_index = None;
            }
            Some(ti) => {
                // Move to previous track
                self.selected_queue_track_index = Some(ti - 1);
            }
            None => {
                // On album header → move to previous album
                if self.selected_queue_index == 0 {
                    self.selected_queue_index = self.queue.len() - 1;
                } else {
                    self.selected_queue_index -= 1;
                }
                // If the previous album is expanded, land on its last track
                let prev = &self.queue[self.selected_queue_index];
                if prev.expanded && !prev.item.album.tracks.is_empty() {
                    self.selected_queue_track_index =
                        Some(prev.item.album.tracks.len() - 1);
                } else {
                    self.selected_queue_track_index = None;
                }
            }
        }
    }

    pub fn add_directory(&mut self, path: PathBuf) {
        match self.library.add_directory(path) {
            Ok(needs_scan) => {
                if needs_scan {
                    self.needs_rescan = true;
                    self.status_message = Some("Directory added. Press 's' to scan.".to_string());
                } else {
                    self.status_message = Some("Directory already exists.".to_string());
                }
            }
            Err(msg) => {
                self.status_message = Some(msg);
            }
        }
    }

    /// Add a directory without triggering rescan (for startup initialization)
    pub fn add_directory_quiet(&mut self, path: PathBuf) {
        let _ = self.library.add_directory(path);
    }

    pub fn remove_selected_directory(&mut self) {
        if self
            .library
            .remove_directory(self.selected_directory_index)
            .is_some()
        {
            if self.selected_directory_index >= self.library.directories.len()
                && self.selected_directory_index > 0
            {
                self.selected_directory_index = self.library.directories.len() - 1;
            }
            self.needs_rescan = true;
        }
    }

    pub fn toggle_directory_expansion(&mut self) {
        // Find which directory in the tree we're selecting
        let tree_items = self.get_directory_tree_items();
        if let Some((path, _, _)) = tree_items.get(self.selected_directory_index) {
            // Helper to find and toggle directory recursively
            fn toggle_recursive(
                directories: &mut [sotf_audio_player::DirectoryInfo],
                target_path: &std::path::Path,
            ) -> bool {
                for dir in directories {
                    if dir.path == target_path {
                        dir.expanded = !dir.expanded;
                        return true;
                    }
                    if dir.expanded {
                        if toggle_recursive(&mut dir.subdirectories, target_path) {
                            return true;
                        }
                    }
                }
                false
            }

            toggle_recursive(&mut self.library.directories, path);
        }
    }

    /// Get flattened directory tree for display
    pub fn get_directory_tree_items(&self) -> Vec<(PathBuf, usize, bool)> {
        self.library.get_directory_tree_items()
    }

    /// Start library scan (non-blocking background scan)
    pub fn start_library_scan(&mut self) {
        if self.scan_in_progress {
            return; // Already scanning
        }

        // Collect directories to scan
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();

        if directories.is_empty() {
            self.status_message = Some("No directories to scan".to_string());
            return;
        }

        // Start background scanner
        let scanner = sotf_audio_player::LibraryScanner::start(directories);
        self.library_scanner = Some(scanner);

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Starting library scan...".to_string());
        log::info!("Started background library scan");
    }

    /// Start force library scan (non-blocking background scan, rescans ALL files)
    ///
    /// Unlike `start_library_scan()`, this rescans all files regardless of modification time.
    /// ReplayGain values are preserved (not overwritten).
    pub fn start_force_library_scan(&mut self) {
        if self.scan_in_progress {
            return; // Already scanning
        }

        // Collect directories to scan
        let directories: Vec<std::path::PathBuf> = self
            .library
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect();

        if directories.is_empty() {
            self.status_message = Some("No directories to scan".to_string());
            return;
        }

        // Start background scanner with force=true
        let scanner = sotf_audio_player::LibraryScanner::start_force(directories);
        self.library_scanner = Some(scanner);

        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Starting FORCE library scan (all files)...".to_string());
        log::info!("Started FORCE background library scan");
    }

    /// Check progress of background library scan
    pub fn check_library_scan_progress(&mut self) {
        if !self.scan_in_progress {
            return;
        }

        // Collect messages first to avoid borrow issues
        let messages: Vec<_> = {
            let scanner = match &self.library_scanner {
                Some(s) => s,
                None => return,
            };
            let mut msgs = Vec::new();
            while let Some(msg) = scanner.try_recv() {
                msgs.push(msg);
            }
            msgs
        };

        // Process collected messages
        for msg in messages {
            use sotf_audio_player::LibraryScanMessage;

            match msg {
                LibraryScanMessage::Progress { tracks, albums } => {
                    self.scan_progress_tracks = tracks;
                    self.scan_progress_albums = albums;
                    self.status_message = Some(format!(
                        "Scanning: {} tracks, {} albums found...",
                        tracks, albums
                    ));
                }
                LibraryScanMessage::Complete { tracks, albums } => {
                    self.scan_in_progress = false;
                    self.library_scanner = None;
                    self.needs_rescan = false;
                    self.status_message = Some(format!(
                        "Scan complete: {} tracks in {} albums",
                        tracks, albums
                    ));
                    log::info!(
                        "Library scan complete: {} tracks in {} albums",
                        tracks,
                        albums
                    );

                    // Reload library from database to get the new data
                    if let Err(e) = self.library.load_from_database() {
                        log::error!("Failed to reload library after scan: {}", e);
                    }
                    self.rebuild_artist_tree();

                    // Start background waveform scan for new tracks
                    if let Err(e) = self.start_waveform_scan() {
                        log::warn!("Failed to start waveform scan: {}", e);
                    }
                }
                LibraryScanMessage::Error { message } => {
                    self.scan_in_progress = false;
                    self.library_scanner = None;
                    self.status_message = Some(format!("Scan failed: {}", message));
                    log::error!("Library scan failed: {}", message);
                }
            }
        }
    }

    pub fn scan_library(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.scan_in_progress = true;
        self.scan_progress_tracks = 0;
        self.scan_progress_albums = 0;
        self.status_message = Some("Scanning library...".to_string());

        // Create shared progress state
        let progress_tracks = Arc::new(Mutex::new(0usize));
        let progress_albums = Arc::new(Mutex::new(0usize));
        let last_update_tracks = Arc::new(Mutex::new(0usize));

        let progress_tracks_clone = Arc::clone(&progress_tracks);
        let progress_albums_clone = Arc::clone(&progress_albums);
        let last_update_clone = Arc::clone(&last_update_tracks);

        // Use progress callback to update shared progress
        let result = self.library.scan_with_progress(move |tracks, albums| {
            let should_update = if let Ok(last) = last_update_clone.lock() {
                tracks - *last >= 1000 || tracks == 0
            } else {
                false
            };

            if should_update {
                if let Ok(mut pt) = progress_tracks_clone.lock() {
                    *pt = tracks;
                }
                if let Ok(mut pa) = progress_albums_clone.lock() {
                    *pa = albums;
                }
                if let Ok(mut last) = last_update_clone.lock() {
                    *last = tracks;
                }
                log::info!("Scan progress: {} tracks, {} albums found", tracks, albums);
            }
        });

        // Update app state with final progress
        if let Ok(pt) = progress_tracks.lock() {
            self.scan_progress_tracks = *pt;
        }
        if let Ok(pa) = progress_albums.lock() {
            self.scan_progress_albums = *pa;
        }

        self.scan_in_progress = false;
        self.needs_rescan = false;
        self.selected_album_index = 0;
        self.album_list_offset = 0;

        match &result {
            Ok(_) => {
                let album_count = self.library.albums.len();
                let track_count: usize = self.library.albums.iter().map(|a| a.tracks.len()).sum();
                self.status_message = Some(format!(
                    "Scan complete: {} tracks in {} albums",
                    track_count, album_count
                ));
                log::info!(
                    "Scan complete: {} tracks in {} albums",
                    track_count,
                    album_count
                );
            }
            Err(e) => {
                self.status_message = Some(format!("Scan failed: {}", e));
                log::error!("Scan failed: {}", e);
            }
        }

        self.rebuild_artist_tree();

        // Start background waveform scan for new tracks
        if result.is_ok() {
            if let Err(e) = self.start_waveform_scan() {
                log::warn!("Failed to start waveform scan: {}", e);
            }
        }

        result
    }

    pub fn clean_library_database(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        self.maintenance_in_progress = true;
        self.maintenance_progress_checked = 0;
        self.maintenance_progress_total = 0;
        self.status_message = Some("Starting database maintenance...".to_string());

        // Create shared progress state
        let progress_checked = Arc::new(Mutex::new(0usize));
        let progress_total = Arc::new(Mutex::new(0usize));

        let progress_checked_clone = Arc::clone(&progress_checked);
        let progress_total_clone = Arc::clone(&progress_total);

        // Use progress callback to update shared progress
        let result = self
            .library
            .clean_database_with_progress(move |checked, total| {
                if let Ok(mut pc) = progress_checked_clone.lock() {
                    *pc = checked;
                }
                if let Ok(mut pt) = progress_total_clone.lock() {
                    *pt = total;
                }
            });

        // Update app state with final progress
        if let Ok(pc) = progress_checked.lock() {
            self.maintenance_progress_checked = *pc;
        }
        if let Ok(pt) = progress_total.lock() {
            self.maintenance_progress_total = *pt;
        }

        self.maintenance_in_progress = false;

        match &result {
            Ok(removed) => {
                if *removed > 0 {
                    self.status_message =
                        Some(format!("Cleaned {} missing tracks from database", removed));
                    log::info!("Database maintenance: removed {} missing tracks", removed);
                } else {
                    self.status_message =
                        Some("Database is clean - no missing tracks found".to_string());
                    log::info!("Database maintenance: no missing tracks found");
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Database maintenance failed: {}", e));
                log::error!("Database maintenance failed: {}", e);
            }
        }

        self.rebuild_artist_tree();

        result
    }

    /// Start background ReplayGain analysis for tracks without gain data
    pub fn start_replay_gain_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let msg = self.replay_gain_manager.start_scan()?;
        if self.replay_gain_manager.in_progress {
            self.status_message = Some(msg);
        }
        Ok(())
    }

    /// Check for ReplayGain scanner progress updates
    pub fn check_replay_gain_progress(&mut self) {
        if !self.replay_gain_manager.in_progress {
            return;
        }

        let just_completed = self.replay_gain_manager.update();

        if just_completed {
            self.status_message = Some(format!(
                "ReplayGain scan complete: {}/{} succeeded, {} failed",
                self.replay_gain_manager.succeeded,
                self.replay_gain_manager.total,
                self.replay_gain_manager.failed
            ));
        }
    }

    /// Start background waveform scanning for tracks without waveform data
    pub fn start_waveform_scan(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.waveform_manager.start_scan()
    }

    /// Check progress of waveform scan
    pub fn check_waveform_progress(&mut self) {
        if !self.waveform_manager.in_progress {
            return;
        }
        self.waveform_manager.update();
    }

    /// Save current app state to config file
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = sotf_audio_player::config::AppConfig {
            version: 1,
            output_device: self.current_output_device_name.clone(),
            queue: self
                .queue
                .iter()
                .map(|entry| (entry.item.album.artist(), entry.item.album.title.clone()))
                .collect(),
            queue_index: self.current_queue_index,
            track_index: self
                .current_queue_index
                .and_then(|idx| self.queue.get(idx))
                .map(|entry| entry.item.current_track_index)
                .unwrap_or(0),
            plugin_preset: self.last_loaded_preset.clone(),
        };

        sotf_audio_player::config::save_app_config(&config)?;
        log::info!("Saved app configuration");
        Ok(())
    }

    /// Load app state from config file and restore it
    pub fn load_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let config = sotf_audio_player::config::load_app_config()?;

        // Restore output device
        if let Some(device_name) = &config.output_device {
            self.current_output_device_name = Some(device_name.clone());
            // Find the device index
            if let Some(idx) = self
                .output_devices
                .iter()
                .position(|d| d.name == *device_name)
            {
                self.selected_output_device_index = idx;
            }
        }

        // Restore queue - need to find albums by artist/title
        for (artist, title) in config.queue {
            if let Some(album) = self
                .library
                .albums
                .iter()
                .find(|a| a.artist() == artist && a.title == title)
                .cloned()
            {
                self.queue.push(QueueEntry::new(QueueItem::new(album)));
            }
        }

        // Restore queue position
        if let Some(queue_idx) = config.queue_index
            && queue_idx < self.queue.len()
        {
            self.current_queue_index = Some(queue_idx);
            // Restore track position within album
            if let Some(entry) = self.queue.get_mut(queue_idx)
                && config.track_index < entry.item.album.tracks.len()
            {
                entry.item.current_track_index = config.track_index;
            }
        }

        // Restore plugin preset
        if let Some(preset_name) = &config.plugin_preset {
            // Use the plugin chain's own load method
            if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() {
                match self.plugin_chain.load_from_file(&presets_dir, preset_name) {
                    Ok(_) => {
                        // Update BinauralDecoder input channels after loading
                        self.plugin_chain.update_channel_dependent_plugins();

                        self.last_loaded_preset = Some(preset_name.clone());
                        self.request_plugin_update();
                        log::info!("Restored plugin preset: {}", preset_name);
                    }
                    Err(e) => {
                        log::warn!("Could not restore preset '{}': {}", preset_name, e);
                    }
                }
            }
        }

        log::info!(
            "Loaded app configuration: {} items in queue, device: {:?}, preset: {:?}",
            self.queue.len(),
            self.current_output_device_name,
            self.last_loaded_preset
        );
        Ok(())
    }

    /// Build the artist tree from the current album list
    pub fn rebuild_artist_tree(&mut self) {
        use std::collections::HashMap;

        let mut artist_map: HashMap<String, Vec<usize>> = HashMap::new();

        // Group albums by artist
        for (idx, album) in self.library.albums.iter().enumerate() {
            artist_map.entry(album.artist()).or_default().push(idx);
        }

        // Create artist nodes
        let mut artists: Vec<_> = artist_map.into_iter().collect();
        artists.sort_by(|a, b| a.0.cmp(&b.0));

        self.artist_tree = artists
            .into_iter()
            .map(|(artist, album_indices)| ArtistNode {
                artist,
                album_indices,
                expanded: false,
            })
            .collect();

        self.selected_tree_index = 0;
    }

    /// Toggle tree view mode
    pub fn toggle_library_view_mode(&mut self) {
        self.library_view_mode = match self.library_view_mode {
            LibraryViewMode::Flat => LibraryViewMode::TreeView,
            LibraryViewMode::TreeView => LibraryViewMode::Flat,
        };
        self.selected_tree_index = 0;
    }

    /// Set library sort order
    pub fn set_library_sort_order(&mut self, order: LibrarySortOrder) {
        self.library_sort_order = order;
        // Reset selection to top when changing sort order
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Mark cache as dirty
        self.request_filter_update();
        // Rebuild tree view if active (as sort order affects tree structure)
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Set channel filter
    pub fn set_channel_filter(&mut self, filter: ChannelFilter) {
        self.channel_filter = filter;
        // Reset selection to top when changing filter
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Mark cache as dirty
        self.request_filter_update();
        // Rebuild tree view if active
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Get unique channel counts present in the library
    pub fn get_unique_channel_counts(&self) -> Vec<u32> {
        use std::collections::HashSet;

        let mut channel_counts = HashSet::new();

        for album in &self.library.albums {
            if let Some(count) = album.uniform_channel_count() {
                channel_counts.insert(count);
            }
        }

        let mut counts: Vec<u32> = channel_counts.into_iter().collect();
        counts.sort();
        counts
    }

    /// Cycle to next channel filter
    pub fn cycle_channel_filter(&mut self) {
        // Get available channel counts in library (excluding mono and stereo since they have their own filters)
        let specific_counts: Vec<u32> = self
            .get_unique_channel_counts()
            .into_iter()
            .filter(|&count| count > 2)
            .collect();

        self.channel_filter = match self.channel_filter {
            ChannelFilter::All => ChannelFilter::Mono,
            ChannelFilter::Mono => ChannelFilter::Stereo,
            ChannelFilter::Stereo => ChannelFilter::Surround,
            ChannelFilter::Surround => ChannelFilter::Surround71,
            ChannelFilter::Surround71 => ChannelFilter::SurroundPlus,
            ChannelFilter::SurroundPlus => ChannelFilter::Mixed,
            ChannelFilter::Mixed => {
                // Cycle to first specific count if available, otherwise back to All
                if let Some(&first_count) = specific_counts.first() {
                    ChannelFilter::Specific(first_count)
                } else {
                    ChannelFilter::All
                }
            }
            ChannelFilter::Specific(current) => {
                // Find next specific count in the list
                if let Some(pos) = specific_counts.iter().position(|&c| c == current) {
                    if pos + 1 < specific_counts.len() {
                        ChannelFilter::Specific(specific_counts[pos + 1])
                    } else {
                        ChannelFilter::All
                    }
                } else {
                    ChannelFilter::All
                }
            }
        };
        // Reset selection
        self.selected_album_index = 0;
        self.selected_tree_index = 0;
        // Mark cache as dirty
        self.request_filter_update();
        // Rebuild tree view if active
        if self.library_view_mode == LibraryViewMode::TreeView {
            self.rebuild_artist_tree();
        }
    }

    /// Toggle expansion of the currently selected artist node
    pub fn toggle_artist_expansion(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        // Get the filtered tree items to find which artist we're on
        let tree_items = self.get_tree_items();
        if let Some(TreeItem::Artist { name, .. }) = tree_items.get(self.selected_tree_index) {
            // Find this artist in the tree and toggle expansion
            for artist_node in &mut self.artist_tree {
                if artist_node.artist == *name {
                    artist_node.expanded = !artist_node.expanded;
                    // Note: This doesn't change the set of albums, just visibility in tree
                    // so we don't necessarily need request_filter_update() here
                    // but we do need to rebuild the tree items display
                    return;
                }
            }
        }
    }

    /// Get the set of album indices that pass the current search and channel filters
    fn filtered_album_indices(&self) -> std::collections::HashSet<usize> {
        use sotf_audio_player::AlbumChannelType;
        use std::collections::HashSet;

        let mut indices: HashSet<usize> = if self.search_query.is_empty() {
            (0..self.library.albums.len()).collect()
        } else {
            // Get filtered albums and find their indices in the library
            let filtered = self.library.search_albums(&self.search_query);
            self.library
                .albums
                .iter()
                .enumerate()
                .filter(|(_, album)| filtered.iter().any(|a| std::ptr::eq(*a, *album)))
                .map(|(idx, _)| idx)
                .collect()
        };

        // Apply channel filter
        indices.retain(|&idx| {
            if let Some(album) = self.library.albums.get(idx) {
                match self.channel_filter {
                    ChannelFilter::All => true,
                    ChannelFilter::Mono => album.uniform_channel_count() == Some(1),
                    ChannelFilter::Stereo => album.uniform_channel_count() == Some(2),
                    ChannelFilter::Surround => {
                        matches!(album.uniform_channel_count(), Some(5) | Some(6))
                    }
                    ChannelFilter::Surround71 => album.uniform_channel_count() == Some(8),
                    ChannelFilter::SurroundPlus => {
                        album.uniform_channel_count().is_some_and(|ch| ch > 8)
                    }
                    ChannelFilter::Mixed => {
                        matches!(album.channel_type(), Some(AlbumChannelType::Mixed))
                    }
                    ChannelFilter::Specific(n) => album.uniform_channel_count() == Some(n),
                }
            } else {
                false
            }
        });

        indices
    }

    /// Get the flattened tree items for rendering (returns artist names or album indices)
    /// Respects search query and channel filter
    pub fn get_tree_items(&self) -> Vec<TreeItem> {
        let mut items = Vec::new();
        let filtered_indices = self.filtered_album_indices();

        for artist_node in &self.artist_tree {
            // Filter albums for this artist
            let visible_albums: Vec<usize> = artist_node
                .album_indices
                .iter()
                .copied()
                .filter(|idx| filtered_indices.contains(idx))
                .collect();

            // Skip artists with no visible albums
            if visible_albums.is_empty() {
                continue;
            }

            items.push(TreeItem::Artist {
                name: artist_node.artist.clone(),
                expanded: artist_node.expanded,
            });

            if artist_node.expanded {
                for album_idx in visible_albums {
                    items.push(TreeItem::Album { index: album_idx });
                }
            }
        }

        items
    }

    /// Select next item in tree view
    pub fn select_next_tree_item(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            self.selected_tree_index = (self.selected_tree_index + 1) % tree_items.len();
        }
    }

    /// Select previous item in tree view
    pub fn select_previous_tree_item(&mut self) {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return;
        }

        let tree_items = self.get_tree_items();
        if !tree_items.is_empty() {
            if self.selected_tree_index == 0 {
                self.selected_tree_index = tree_items.len() - 1;
            } else {
                self.selected_tree_index -= 1;
            }
        }
    }

    /// Add the selected item (artist or album) to queue from tree view
    pub fn add_tree_selection_to_queue(&mut self) -> Option<PathBuf> {
        if self.library_view_mode != LibraryViewMode::TreeView {
            return None;
        }

        let was_empty = self.queue.is_empty();
        let was_not_playing = !self.is_playing;
        let tree_items = self.get_tree_items();
        let filtered_indices = self.filtered_album_indices();

        if let Some(item) = tree_items.get(self.selected_tree_index) {
            match item {
                TreeItem::Artist { name, .. } => {
                    // Find this artist in the tree and add their filtered albums
                    for artist_node in &self.artist_tree {
                        if artist_node.artist == *name {
                            // Add only albums that pass the current filter
                            for &album_idx in &artist_node.album_indices {
                                if filtered_indices.contains(&album_idx) {
                                    if let Some(album) = self.library.albums.get(album_idx) {
                                        self.queue
                                            .push(QueueEntry::new(QueueItem::new(album.clone())));
                                    }
                                }
                            }
                            // Auto-play if queue was empty OR if nothing was playing
                            if was_empty || was_not_playing {
                                return self.start_queue();
                            }
                            return None;
                        }
                    }
                }
                TreeItem::Album { index } => {
                    // Add single album
                    if let Some(album) = self.library.albums.get(*index) {
                        self.queue
                            .push(QueueEntry::new(QueueItem::new(album.clone())));

                        // Auto-play if queue was empty OR if nothing was playing
                        if was_empty || was_not_playing {
                            return self.start_queue();
                        }
                    }
                }
            }
        }
        None
    }

    pub fn current_track_path(&self) -> Option<PathBuf> {
        self.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.current_track())
            .map(|track| track.path.clone())
    }

    /// Get the currently playing track info
    pub fn current_track(&self) -> Option<&Track> {
        self.current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.current_track())
    }

    pub fn next_track(&mut self) -> Option<PathBuf> {
        if let Some(idx) = self.current_queue_index {
            if let Some(entry) = self.queue.get_mut(idx) {
                if let Some(track) = entry.item.next_track() {
                    return Some(track.path.clone());
                }
            }

            // Album finished (or entry missing), remove it and move to next
            self.remove_from_queue(idx);
            return self.current_track_path();
        }
        None
    }

    pub fn previous_track(&mut self) -> Option<PathBuf> {
        if let Some(idx) = self.current_queue_index
            && let Some(entry) = self.queue.get_mut(idx)
        {
            if let Some(track) = entry.item.previous_track() {
                return Some(track.path.clone());
            } else {
                // Move to previous album in queue
                if idx > 0 {
                    self.current_queue_index = Some(idx - 1);
                    // Go to last track of previous album
                    if let Some(prev_entry) = self.queue.get_mut(idx - 1) {
                        prev_entry.item.current_track_index =
                            prev_entry.item.album.tracks.len().saturating_sub(1);
                    }
                    return self.current_track_path();
                }
            }
        }
        None
    }

    pub fn start_queue(&mut self) -> Option<PathBuf> {
        if !self.queue.is_empty() {
            self.current_queue_index = Some(0);
            self.queue[0].item.current_track_index = 0;
            self.is_playing = true;
            self.current_track_path()
        } else {
            None
        }
    }

    /// Jump to the selected album/track in queue and start playing
    pub fn jump_to_selected_album(&mut self) -> Option<PathBuf> {
        if self.selected_queue_index < self.queue.len() {
            self.current_queue_index = Some(self.selected_queue_index);
            let track_idx = self.selected_queue_track_index.unwrap_or(0);
            self.queue[self.selected_queue_index]
                .item
                .current_track_index = track_idx;
            self.is_playing = true;
            self.current_track_path()
        } else {
            None
        }
    }

    pub fn increase_volume(&mut self) {
        self.volume = (self.volume + 0.05).min(1.0);
    }

    pub fn decrease_volume(&mut self) {
        self.volume = (self.volume - 0.05).max(0.0);
    }

    // Plugin management

    /// Request a plugin update and reset retry state
    /// This should be called whenever the plugin chain is modified
    pub fn request_plugin_update(&mut self) {
        self.needs_plugin_update = true;
        self.plugin_update_retry_count = 0;
        self.plugin_update_in_progress = false;
    }

    pub fn add_plugin(&mut self, plugin_type: &PluginType) {
        let insert_idx = self.plugin_chain.user_plugin_insert_index();
        self.plugin_chain.insert_plugin(insert_idx, plugin_type);
        // Update BinauralDecoder input channels after adding
        self.plugin_chain.update_channel_dependent_plugins();
        self.request_plugin_update();
    }

    pub fn remove_plugin(&mut self, index: usize) {
        self.plugin_chain.remove_plugin(index);
        if self.selected_plugin_index >= self.plugin_chain.len() && self.selected_plugin_index > 0 {
            self.selected_plugin_index = self.plugin_chain.len() - 1;
        }
        // Update BinauralDecoder input channels after removal
        self.plugin_chain.update_channel_dependent_plugins();
        self.request_plugin_update();
    }

    pub fn toggle_plugin(&mut self, index: usize) {
        self.plugin_chain.toggle_plugin(index);
        // Update BinauralDecoder input channels after toggle
        self.plugin_chain.update_channel_dependent_plugins();
        self.request_plugin_update();
    }

    pub fn move_plugin_up(&mut self, index: usize) {
        if self.plugin_chain.can_move_plugin_up(index) {
            self.plugin_chain.move_plugin(index, index - 1);
            self.selected_plugin_index = index - 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_channel_dependent_plugins();
            self.request_plugin_update();
        }
    }

    pub fn move_plugin_down(&mut self, index: usize) {
        if self.plugin_chain.can_move_plugin_down(index) {
            self.plugin_chain.move_plugin(index, index + 1);
            self.selected_plugin_index = index + 1;
            // Update BinauralDecoder input channels after move
            self.plugin_chain.update_channel_dependent_plugins();
            self.request_plugin_update();
        }
    }

    pub fn select_next_plugin(&mut self) {
        if !self.plugin_chain.is_empty() {
            self.selected_plugin_index = (self.selected_plugin_index + 1) % self.plugin_chain.len();
        }
    }

    pub fn select_previous_plugin(&mut self) {
        if !self.plugin_chain.is_empty() {
            if self.selected_plugin_index == 0 {
                self.selected_plugin_index = self.plugin_chain.len() - 1;
            } else {
                self.selected_plugin_index -= 1;
            }
        }
    }

    // Plugin parameter editing
    pub fn enter_plugin_edit_mode(&mut self) {
        if self.selected_plugin_index < self.plugin_chain.len() {
            self.editing_plugin_index = Some(self.selected_plugin_index);
            self.plugin_param_selection = 0;
            self.input_mode = InputMode::EditPlugin;
        }
    }

    pub fn exit_plugin_edit_mode(&mut self) {
        self.editing_plugin_index = None;
        self.plugin_param_selection = 0;
        self.input_mode = InputMode::Normal;
    }

    pub fn get_editing_plugin(&self) -> Option<&sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_chain.get_plugin(idx))
    }

    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut sotf_audio_player::Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.plugin_chain.get_plugin_mut(idx))
    }

    pub fn select_next_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                self.plugin_param_selection = (self.plugin_param_selection + 1) % param_count;
            }
        }
    }

    pub fn select_previous_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                if self.plugin_param_selection == 0 {
                    self.plugin_param_selection = param_count - 1;
                } else {
                    self.plugin_param_selection -= 1;
                }
            }
        }
    }

    /// Adjust the currently selected parameter by the given delta
    /// Returns true if the parameter was adjusted successfully
    pub fn adjust_selected_param(&mut self, delta: f64) -> bool {
        let param_idx = self.plugin_param_selection;

        let success = if let Some(plugin) = self.get_editing_plugin_mut() {
            plugin.settings.adjust_param(param_idx, delta)
        } else {
            false
        };

        if success {
            // Always propagate channel counts — a parameter change (e.g., upmixer speaker config)
            // may change intermediate channel counts that downstream plugins depend on
            self.plugin_chain.update_channel_dependent_plugins();
        }

        success
    }

    // ========================================================================
    // Matrix Editor Methods
    // ========================================================================

    /// Get the dimensions of the currently editing Matrix plugin
    pub fn get_matrix_dimensions(&self) -> Option<(usize, usize)> {
        use sotf_audio_player::PluginSettings;
        if let Some(plugin) = self.get_editing_plugin() {
            if let PluginSettings::Matrix {
                input_channels,
                output_channels,
                ..
            } = &plugin.settings
            {
                return Some((*input_channels, *output_channels));
            }
        }
        None
    }

    /// Adjust the selected matrix header parameter (input channels, output channels, or preset)
    /// Returns true if adjustment was made
    pub fn adjust_matrix_header(&mut self, delta: i32) -> bool {
        use sotf_audio_player::{PluginSettings, apply_matrix_preset, resize_matrix};

        // Read selection before mutable borrow
        let header_selection = self.matrix_header_selection;

        // Track whether we need to clamp grid selection and the new dimensions
        let mut clamp_col_to: Option<usize> = None;
        let mut clamp_row_to: Option<usize> = None;

        let result = {
            let Some(plugin) = self.get_editing_plugin_mut() else {
                return false;
            };

            let PluginSettings::Matrix {
                input_channels,
                output_channels,
                matrix,
                ..
            } = &mut plugin.settings
            else {
                return false;
            };

            match header_selection {
                0 => {
                    // Input channels: 1-16
                    let old_in = *input_channels;
                    let new_in = (*input_channels as i32 + delta).clamp(1, 16) as usize;
                    if new_in != old_in {
                        resize_matrix(matrix, old_in, *output_channels, new_in, *output_channels);
                        *input_channels = new_in;
                        clamp_col_to = Some(new_in);
                        true
                    } else {
                        false
                    }
                }
                1 => {
                    // Output channels: 1-16
                    let old_out = *output_channels;
                    let new_out = (*output_channels as i32 + delta).clamp(1, 16) as usize;
                    if new_out != old_out {
                        resize_matrix(matrix, *input_channels, old_out, *input_channels, new_out);
                        *output_channels = new_out;
                        clamp_row_to = Some(new_out);
                        true
                    } else {
                        false
                    }
                }
                2 => {
                    // Preset: cycle through presets valid for current channel config
                    let in_ch = *input_channels;
                    let out_ch = *output_channels;
                    let presets = sotf_audio_player::available_matrix_presets(in_ch, out_ch);
                    let current = sotf_audio_player::detect_matrix_preset(in_ch, out_ch, matrix);
                    let current_idx = presets.iter().position(|&p| p == current).unwrap_or(0);
                    let new_idx = if delta > 0 {
                        (current_idx + 1) % presets.len()
                    } else {
                        (current_idx + presets.len() - 1) % presets.len()
                    };
                    apply_matrix_preset(in_ch, out_ch, matrix, presets[new_idx]);
                    true
                }
                _ => false,
            }
        }; // Mutable borrow ends here

        // Clamp grid selection after borrow is released
        if let Some(max_col) = clamp_col_to {
            if self.matrix_grid_col >= max_col {
                self.matrix_grid_col = max_col.saturating_sub(1);
            }
        }
        if let Some(max_row) = clamp_row_to {
            if self.matrix_grid_row >= max_row {
                self.matrix_grid_row = max_row.saturating_sub(1);
            }
        }

        result
    }

    /// Adjust the selected matrix cell gain by dB amount
    /// Returns true if adjustment was made
    pub fn adjust_matrix_cell(&mut self, delta_db: f32) -> bool {
        use sotf_audio_player::{PluginSettings, db_to_linear};

        // Read grid position before mutable borrow
        let grid_row = self.matrix_grid_row;
        let grid_col = self.matrix_grid_col;

        let Some(plugin) = self.get_editing_plugin_mut() else {
            return false;
        };

        let PluginSettings::Matrix {
            input_channels,
            matrix,
            ..
        } = &mut plugin.settings
        else {
            return false;
        };

        let idx = grid_row * *input_channels + grid_col;
        if idx >= matrix.len() {
            return false;
        }

        let current = matrix[idx];
        // Convert to dB, adjust, convert back
        let current_db = if current < 0.001 {
            -60.0 // Treat as -60 dB for adjustment
        } else {
            20.0 * current.log10()
        };
        let new_db = (current_db + delta_db).clamp(-60.0, 6.0);
        let new_linear = if new_db <= -60.0 {
            0.0 // Silence
        } else {
            db_to_linear(new_db)
        };
        matrix[idx] = new_linear;
        true
    }

    /// Set the selected matrix cell to a specific linear gain value
    /// Returns true if adjustment was made
    pub fn set_matrix_cell(&mut self, linear_gain: f32) -> bool {
        use sotf_audio_player::PluginSettings;

        // Read grid position before mutable borrow
        let grid_row = self.matrix_grid_row;
        let grid_col = self.matrix_grid_col;

        let Some(plugin) = self.get_editing_plugin_mut() else {
            return false;
        };

        let PluginSettings::Matrix {
            input_channels,
            matrix,
            ..
        } = &mut plugin.settings
        else {
            return false;
        };

        let idx = grid_row * *input_channels + grid_col;
        if idx >= matrix.len() {
            return false;
        }

        matrix[idx] = linear_gain.clamp(0.0, 2.0);
        true
    }

    // ========================================================================

    /// Save plugin chain to file
    pub fn save_plugin_chain(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.status_message = Some("Error: No filename specified".to_string());
            return;
        }

        // Check if file exists and show warning if overwriting
        let filename_with_ext = if self.plugin_file_input.ends_with(".json") {
            self.plugin_file_input.clone()
        } else {
            format!("{}.json", self.plugin_file_input)
        };

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() {
            let full_path = presets_dir.join(&filename_with_ext);
            if full_path.exists() {
                self.status_message = Some(format!(
                    "Warning: Overwriting existing preset: {}",
                    filename_with_ext
                ));
                log::warn!("Overwriting existing preset: {}", filename_with_ext);
            }
        }

        // Save using the plugin chain's own save method (handles path, validation, etc.)
        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.status_message = Some("Error: Could not find presets directory".to_string());
            return;
        };
        match self
            .plugin_chain
            .save_to_file(&presets_dir, &self.plugin_file_input)
        {
            Ok(_) => {
                self.status_message = Some(format!("Saved preset: {}", filename_with_ext));
                self.last_loaded_preset = Some(filename_with_ext);
                // Refresh presets list
                self.refresh_plugin_presets();
            }
            Err(e) => {
                self.status_message = Some(format!("Error saving: {}", e));
                log::error!("Failed to save plugin chain: {}", e);
            }
        }
    }

    /// Save plugin chain to selected preset file (overwrite confirmation shown in UI)
    pub fn save_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.status_message = Some("No presets available".to_string());
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            // Pass filename as-is; save_to_file handles .json extension correctly
            // Save using the plugin chain's own save method
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                self.status_message = Some("Error: Could not find presets directory".to_string());
                return;
            };
            match self
                .plugin_chain
                .save_to_file(&presets_dir, &preset_filename)
            {
                Ok(_) => {
                    self.status_message = Some(format!("Overwritten preset: {}", preset_filename));
                    self.last_loaded_preset = Some(preset_filename);
                    // Refresh presets list
                    self.refresh_plugin_presets();
                }
                Err(e) => {
                    self.status_message = Some(format!("Error saving: {}", e));
                    log::error!("Failed to save plugin chain: {}", e);
                }
            }
        }
    }

    /// Load plugin chain from file
    pub fn load_plugin_chain(&mut self) {
        if self.plugin_file_input.is_empty() {
            self.status_message = Some("Error: No filename specified".to_string());
            return;
        }

        // Load using the plugin chain's own load method (handles path, extension, etc.)
        let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
            self.status_message = Some("Error: Could not find presets directory".to_string());
            return;
        };
        match self
            .plugin_chain
            .load_from_file(&presets_dir, &self.plugin_file_input)
        {
            Ok(_) => {
                // Update BinauralDecoder input channels after loading
                self.plugin_chain.update_channel_dependent_plugins();

                // Get the final filename (with .json appended if needed)
                let filename = if self.plugin_file_input.ends_with(".json") {
                    self.plugin_file_input.clone()
                } else {
                    format!("{}.json", self.plugin_file_input)
                };

                self.status_message = Some(format!("Loaded preset: {}", filename));
                self.request_plugin_update();
                self.last_loaded_preset = Some(filename);
            }
            Err(e) => {
                self.status_message = Some(format!("Error loading: {}", e));
                log::error!("Failed to load plugin chain: {}", e);
            }
        }
    }

    /// Refresh the list of available plugin presets from the config directory
    pub fn refresh_plugin_presets(&mut self) {
        self.available_plugin_presets.clear();
        self.selected_preset_index = 0;

        if let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir()
            && let Ok(entries) = std::fs::read_dir(&presets_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && ext == "json"
                    && let Some(filename) = path.file_name()
                {
                    self.available_plugin_presets
                        .push(filename.to_string_lossy().to_string());
                }
            }
            // Sort presets alphabetically
            self.available_plugin_presets.sort();
        }

        log::info!(
            "Found {} plugin presets",
            self.available_plugin_presets.len()
        );
    }

    /// Select the next preset in the list
    pub fn select_next_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            self.selected_preset_index =
                (self.selected_preset_index + 1) % self.available_plugin_presets.len();
        }
    }

    /// Select the previous preset in the list
    pub fn select_previous_preset(&mut self) {
        if !self.available_plugin_presets.is_empty() {
            if self.selected_preset_index == 0 {
                self.selected_preset_index = self.available_plugin_presets.len() - 1;
            } else {
                self.selected_preset_index -= 1;
            }
        }
    }

    /// Load the currently selected preset
    pub fn load_selected_preset(&mut self) {
        if self.available_plugin_presets.is_empty() {
            self.status_message = Some("No presets available".to_string());
            log::warn!("No presets available to load");
            return;
        }

        if let Some(preset_filename) = self
            .available_plugin_presets
            .get(self.selected_preset_index)
            .cloned()
        {
            log::info!(
                "Loading preset: {} (index {})",
                preset_filename,
                self.selected_preset_index
            );
            // Use the plugin chain's own load method (handles path construction)
            let Some(presets_dir) = sotf_audio_player::config::get_plugin_presets_dir() else {
                self.status_message = Some("Error: Could not find presets directory".to_string());
                return;
            };
            match self
                .plugin_chain
                .load_from_file(&presets_dir, &preset_filename)
            {
                Ok(_) => {
                    // Update BinauralDecoder input channels after loading
                    self.plugin_chain.update_channel_dependent_plugins();

                    log::info!(
                        "Successfully loaded preset: {} ({} plugins)",
                        preset_filename,
                        self.plugin_chain.len()
                    );
                    self.status_message = Some(format!("Loaded preset: {}", preset_filename));
                    self.request_plugin_update();
                    self.last_loaded_preset = Some(preset_filename);
                }
                Err(e) => {
                    self.status_message = Some(format!("Error loading preset: {}", e));
                    log::error!("Failed to load preset {}: {}", preset_filename, e);
                }
            }
        } else {
            log::error!(
                "Failed to get preset at index {}",
                self.selected_preset_index
            );
        }
    }

    /// Generate autocomplete suggestions for the current directory input
    pub fn generate_autocomplete_suggestions(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

        let input = if self.directory_input.is_empty() {
            "./"
        } else {
            &self.directory_input
        };

        // Expand tilde to home directory
        let expanded_input = if input.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                input.replacen('~', &home, 1)
            } else {
                input.to_string()
            }
        } else {
            input.to_string()
        };

        let path = std::path::Path::new(&expanded_input);

        // Determine the directory to search and the prefix to match
        let (search_dir, prefix) = if path.is_dir() && expanded_input.ends_with('/') {
            // User typed a complete directory with trailing slash
            (path.to_path_buf(), String::new())
        } else if let Some(parent) = path.parent() {
            // User is typing a partial name
            let prefix = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), prefix)
        } else {
            // Fallback to current directory
            (std::path::PathBuf::from("."), expanded_input.clone())
        };

        // Read directory and find matching entries
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Skip hidden files unless prefix starts with '.'
                    if file_name.starts_with('.') && !prefix.starts_with('.') {
                        continue;
                    }

                    // Check if filename starts with prefix
                    if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut full_path = search_dir.join(&file_name);

                        // Add trailing slash for directories
                        if entry.path().is_dir() {
                            full_path = full_path.join("");
                        }

                        let suggestion = full_path.to_string_lossy().to_string();
                        self.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        // Sort suggestions
        self.autocomplete_suggestions.sort();
    }

    /// Apply the current autocomplete suggestion to the directory input
    pub fn apply_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.directory_input = suggestion.clone();
        }
    }

    /// Cycle to the next autocomplete suggestion
    pub fn next_autocomplete(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete();
        }
    }

    /// Clear autocomplete suggestions
    pub fn clear_autocomplete(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;
    }

    /// Generate autocomplete suggestions for saving presets (restricted to preset directory)
    /// This filters available presets by the current input and provides suggestions
    pub fn generate_autocomplete_suggestions_for_save_preset(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

        // Get the current input (without .json extension if present)
        let input = self
            .plugin_file_input
            .trim_end_matches(".json")
            .to_lowercase();

        // Filter available presets by prefix match
        for preset in &self.available_plugin_presets {
            let preset_without_ext = preset.trim_end_matches(".json");
            if preset_without_ext.to_lowercase().starts_with(&input) {
                // Add suggestion without .json extension (save_to_file will add it)
                self.autocomplete_suggestions
                    .push(preset_without_ext.to_string());
            }
        }

        // Sort suggestions alphabetically
        self.autocomplete_suggestions.sort();
    }

    /// Generate autocomplete suggestions for plugin file input
    pub fn generate_autocomplete_suggestions_for_plugin_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.plugin_file_input.clone());
    }

    /// Apply autocomplete to plugin file input
    pub fn apply_autocomplete_to_plugin_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.plugin_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for plugin file input
    pub fn next_autocomplete_for_plugin_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_plugin_file();
        }
    }

    /// Generate autocomplete suggestions for APO file input
    pub fn generate_autocomplete_suggestions_for_apo_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.apo_file_input.clone());
    }

    /// Apply autocomplete to APO file input
    pub fn apply_autocomplete_to_apo_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.apo_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for APO file input
    pub fn next_autocomplete_for_apo_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_apo_file();
        }
    }

    /// Generate autocomplete suggestions for SOFA file input
    pub fn generate_autocomplete_suggestions_for_sofa_file(&mut self) {
        self.generate_autocomplete_suggestions_for_input(&self.sofa_file_input.clone());
    }

    /// Apply autocomplete to SOFA file input
    pub fn apply_autocomplete_to_sofa_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            let suggestion = &self.autocomplete_suggestions[self.autocomplete_index];
            self.sofa_file_input = suggestion.clone();
        }
    }

    /// Cycle to next autocomplete for SOFA file input
    pub fn next_autocomplete_for_sofa_file(&mut self) {
        if !self.autocomplete_suggestions.is_empty() {
            self.autocomplete_index =
                (self.autocomplete_index + 1) % self.autocomplete_suggestions.len();
            self.apply_autocomplete_to_sofa_file();
        }
    }

    /// Generic autocomplete suggestions generator for any file input
    fn generate_autocomplete_suggestions_for_input(&mut self, input: &str) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_index = 0;

        let input = if input.is_empty() { "./" } else { input };

        // Expand tilde to home directory
        let expanded_input = if input.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                input.replacen('~', &home, 1)
            } else {
                input.to_string()
            }
        } else {
            input.to_string()
        };

        let path = std::path::Path::new(&expanded_input);

        // Determine the directory to search and the prefix to match
        let (search_dir, prefix) = if path.is_dir() && expanded_input.ends_with('/') {
            // User typed a complete directory with trailing slash
            (path.to_path_buf(), String::new())
        } else if let Some(parent) = path.parent() {
            // User is typing a partial name
            let prefix = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent.to_path_buf(), prefix)
        } else {
            // Fallback to current directory
            (std::path::PathBuf::from("."), expanded_input.clone())
        };

        // Read directory and find matching entries
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                if let Ok(file_name) = entry.file_name().into_string() {
                    // Skip hidden files unless prefix starts with '.'
                    if file_name.starts_with('.') && !prefix.starts_with('.') {
                        continue;
                    }

                    // Check if filename starts with prefix
                    if file_name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                        let mut full_path = search_dir.join(&file_name);

                        // Add trailing slash for directories
                        if entry.path().is_dir() {
                            full_path = full_path.join("");
                        }

                        let suggestion = full_path.to_string_lossy().to_string();
                        self.autocomplete_suggestions.push(suggestion);
                    }
                }
            }
        }

        // Sort suggestions
        self.autocomplete_suggestions.sort();
    }

    /// Load APO file and update the currently selected EQ plugin
    pub fn load_apo_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::{EQFilter, PluginSettings};
        use std::path::Path;

        let path = Path::new(&self.apo_file_input);

        // Load filters from APO file
        let filters = EQFilter::from_apo_file(path)?;

        // Update the currently selected plugin if it's an EQ
        if let Some(plugin) = self.plugin_chain.get_plugin_mut(self.selected_plugin_index) {
            if let PluginSettings::EQ { channels, .. } = &plugin.settings {
                let channels = *channels;
                plugin.settings = PluginSettings::EQ {
                    channels,
                    filters,
                    channel_filters: None,
                    per_channel_mode: false,
                };
                Ok(())
            } else {
                Err("Selected plugin is not an EQ".to_string())
            }
        } else {
            Err("No plugin selected".to_string())
        }
    }

    /// Update SOFA file path for the currently selected binaural decoder plugin
    pub fn load_sofa_file(&mut self) -> Result<(), String> {
        use sotf_audio_player::PluginSettings;

        // Update the currently selected plugin if it's a binaural decoder
        if let Some(plugin) = self.plugin_chain.get_plugin_mut(self.selected_plugin_index) {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = self.sofa_file_input.clone();
                Ok(())
            } else {
                Err("Selected plugin is not a Binaural Decoder".to_string())
            }
        } else {
            Err("No plugin selected".to_string())
        }
    }

    /// Find and load all image files in the currently playing album's directory
    pub fn load_album_images(&mut self) {
        self.album_images.clear();
        self.selected_image_index = 0;

        // Initialize image picker if not already done.
        // macOS Terminal.app doesn't support graphics protocols (Kitty/iTerm2)
        // and the stdio query leaks escape sequences onto the screen.
        // Use halfblocks directly for terminals that don't support graphics.
        if self.image_picker.is_none() {
            let use_halfblocks = std::env::var("TERM_PROGRAM")
                .map(|tp| tp == "Apple_Terminal")
                .unwrap_or(false);

            if use_halfblocks {
                log::info!("Terminal.app detected, using halfblocks for album art");
                self.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
            } else {
                match ratatui_image::picker::Picker::from_query_stdio() {
                    Ok(picker) => {
                        self.image_picker = Some(picker);
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to query terminal for font size: {}, using halfblocks fallback",
                            e
                        );
                        self.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
                    }
                }
            }
        }

        // Get the currently playing album
        if let Some(queue_index) = self.current_queue_index {
            if let Some(entry) = self.queue.get(queue_index) {
                if let Some(first_track) = entry.item.album.tracks.first() {
                    if let Some(parent_dir) = first_track.path.parent() {
                        // Find all image files in the directory
                        if let Ok(entries) = std::fs::read_dir(parent_dir) {
                            for entry in entries.flatten() {
                                if let Ok(path) = entry.path().canonicalize() {
                                    if let Some(ext) = path.extension() {
                                        let ext_lower = ext.to_string_lossy().to_lowercase();
                                        if matches!(
                                            ext_lower.as_str(),
                                            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
                                        ) {
                                            self.album_images.push(path);
                                        }
                                    }
                                }
                            }
                        }
                        // Sort images for consistent order
                        self.album_images.sort();
                    }
                }
            }
        }
    }

    /// Cycle to the next image in the album directory
    pub fn next_album_image(&mut self) {
        if !self.album_images.is_empty() {
            self.selected_image_index = (self.selected_image_index + 1) % self.album_images.len();
        }
    }

    /// Cycle to the previous image in the album directory
    pub fn prev_album_image(&mut self) {
        if !self.album_images.is_empty() {
            if self.selected_image_index == 0 {
                self.selected_image_index = self.album_images.len() - 1;
            } else {
                self.selected_image_index -= 1;
            }
        }
    }

    /// Get the currently selected album image path
    pub fn get_current_album_image(&self) -> Option<&PathBuf> {
        self.album_images.get(self.selected_image_index)
    }

    /// Build channel groups from current speaker configuration or channel count
    /// Uses caching to avoid rebuilding every frame
    pub fn update_level_meter_groups(&mut self) {
        let num_channels = self
            .loudness_info
            .as_ref()
            .map(|l| l.channel_peaks.len())
            .unwrap_or(0);

        if num_channels == 0 {
            return;
        }

        // Get current speaker config
        let current_speaker_config = self.plugin_chain.output_speaker_config().map(String::from);

        // Skip rebuilding if nothing has changed
        if num_channels == self.level_meter_last_channel_count
            && current_speaker_config == self.level_meter_last_speaker_config
            && !self.level_meter_groups.is_empty()
        {
            return;
        }

        // Update cache
        self.level_meter_last_channel_count = num_channels;
        self.level_meter_last_speaker_config = current_speaker_config.clone();

        self.level_meter_groups.clear();

        // Try to get meter groups from the speaker config (via upmixer plugin)
        // This handles collisions like 5.1.4 vs 7.1.2 (both 10 channels)
        let meter_groups: Option<&[MeterGroupSpec]> = current_speaker_config
            .as_deref()
            .and_then(get_meter_groups)
            .or_else(|| get_meter_groups_by_channels(num_channels));

        if let Some(groups) = meter_groups {
            // Convert static specs to runtime groups
            for group_spec in groups {
                self.level_meter_groups.push(ChannelGroup {
                    name: group_spec.name.to_string(),
                    channels: group_spec
                        .channels
                        .iter()
                        .map(|ch| ChannelInfo {
                            index: ch.index,
                            name: ch.label.to_string(),
                            display_name: ch
                                .display_chars
                                .iter()
                                .map(|s| (*s).to_string())
                                .collect(),
                        })
                        .collect(),
                    muted: false,
                    soloed: false,
                    dimmed: false,
                });
            }
        } else {
            // Fallback for unknown channel counts (mono, quad, or exotic configs)
            match num_channels {
                1 => {
                    // Mono
                    self.level_meter_groups.push(ChannelGroup {
                        name: "Mono".to_string(),
                        channels: vec![ChannelInfo {
                            index: 0,
                            name: "M".to_string(),
                            display_name: vec!["M".to_string()],
                        }],
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
                4 => {
                    // Quad (FL, FR, SL, SR) - not a standard speaker config
                    self.level_meter_groups.push(ChannelGroup {
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
                    });
                    self.level_meter_groups.push(ChannelGroup {
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
                    });
                }
                _ => {
                    // Generic fallback - treat all channels as one group
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
                    self.level_meter_groups.push(ChannelGroup {
                        name: "All Channels".to_string(),
                        channels,
                        muted: false,
                        soloed: false,
                        dimmed: false,
                    });
                }
            }
        }

        // Update ChannelMuteSolo plugin to have correct number of channels
        self.update_channel_mute_solo_plugin();
    }

    /// Clear all mutes, solos, and dims in level meter groups
    pub fn clear_level_meter_mutes_and_solos(&mut self) {
        for group in &mut self.level_meter_groups {
            group.muted = false;
            group.soloed = false;
            group.dimmed = false;
        }
        self.update_channel_mute_solo_plugin();
    }

    /// Toggle mute for the selected level meter group
    pub fn toggle_level_meter_mute(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.muted = !group.muted;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle solo for the selected level meter group
    pub fn toggle_level_meter_solo(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            let is_currently_soloed = group.soloed;

            // Solo behavior: only one group can be soloed at a time
            // When soloing, set soloed=true on selected group, soloed=false on all others
            // When un-soloing, set soloed=false on selected group
            for (idx, g) in self.level_meter_groups.iter_mut().enumerate() {
                if idx == self.selected_level_meter_group {
                    g.soloed = !is_currently_soloed;
                } else {
                    g.soloed = false;
                }
            }

            self.update_channel_mute_solo_plugin();
        }
    }

    /// Toggle dim for the selected level meter group
    pub fn toggle_level_meter_dim(&mut self) {
        if let Some(group) = self
            .level_meter_groups
            .get_mut(self.selected_level_meter_group)
        {
            group.dimmed = !group.dimmed;
            self.update_channel_mute_solo_plugin();
        }
    }

    /// Update the ChannelMuteSolo plugin based on current level meter group states
    fn update_channel_mute_solo_plugin(&mut self) {
        use sotf_audio_player::PluginSettings;
        use sotf_plugins::ChannelState;

        // Calculate total channel count
        let num_channels = self
            .level_meter_groups
            .iter()
            .map(|g| g.channels.len())
            .sum();

        if num_channels == 0 {
            return;
        }

        // Build per-channel states from groups
        let mut channel_states = vec![
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false
            };
            num_channels
        ];

        for group in &self.level_meter_groups {
            for channel_info in &group.channels {
                if channel_info.index < num_channels {
                    channel_states[channel_info.index] = ChannelState {
                        muted: group.muted,
                        soloed: group.soloed,
                        dimmed: group.dimmed,
                    };
                }
            }
        }

        // Determine if any channel is muted, soloed, or dimmed
        let enabled = channel_states
            .iter()
            .any(|s| s.muted || s.soloed || s.dimmed);

        // Find and update the ChannelMuteSolo plugin
        for i in 0..self.plugin_chain.len() {
            if let Some(plugin) = self.plugin_chain.get_plugin_mut(i) {
                if matches!(&plugin.settings, PluginSettings::ChannelMuteSolo { .. }) {
                    // Update settings in memory
                    plugin.settings = PluginSettings::ChannelMuteSolo {
                        enabled,
                        channel_states: channel_states.clone(),
                    };

                    // Queue zero-dropout parameter update
                    // Use get_engine_index() which handles monitoring plugin reordering
                    if let Some(engine_index) = self.plugin_chain.get_engine_index(i) {
                        // Send enabled and channel_states together as a JSON object
                        // The plugin's set_parameter for channel_states will auto-enable
                        // when any channel is muted/soloed/dimmed
                        let params = serde_json::json!({
                            "enabled": enabled,
                            "channel_states": channel_states,
                        });
                        if let Ok(json) = serde_json::to_string(&params) {
                            self.pending_param_update = Some(PendingParameterUpdate {
                                plugin_index: engine_index,
                                param_id: "full_state".to_string(),
                                value: json,
                            });
                        }
                    }
                    return;
                }
            }
        }
    }

    /// Navigate to next level meter group
    pub fn select_next_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            self.selected_level_meter_group =
                (self.selected_level_meter_group + 1) % self.level_meter_groups.len();
        }
    }

    /// Navigate to previous level meter group
    pub fn select_previous_level_meter_group(&mut self) {
        if !self.level_meter_groups.is_empty() {
            if self.selected_level_meter_group == 0 {
                self.selected_level_meter_group = self.level_meter_groups.len() - 1;
            } else {
                self.selected_level_meter_group -= 1;
            }
        }
    }

    /// Navigate between mute, solo, and dim controls
    pub fn select_next_level_meter_control(&mut self) {
        self.level_meter_control_selection = (self.level_meter_control_selection + 1) % 3;
    }

    /// Navigate between mute, solo, and dim controls (previous)
    pub fn select_previous_level_meter_control(&mut self) {
        self.level_meter_control_selection = if self.level_meter_control_selection == 0 {
            2
        } else {
            self.level_meter_control_selection - 1
        };
    }

    /// Start tracking a new track for play statistics
    pub fn start_track_tracking(&mut self, track_path: PathBuf) {
        self.current_track_path = Some(track_path);
        self.current_track_start_time = Some(std::time::Instant::now());
        self.current_track_already_recorded = false;
    }

    /// Check if current track has been played for 30+ seconds and record it
    pub fn check_and_record_play(&mut self) {
        if self.current_track_already_recorded {
            return;
        }

        if let (Some(path), Some(start_time)) =
            (&self.current_track_path, self.current_track_start_time)
        {
            let elapsed = start_time.elapsed().as_secs();
            if elapsed >= 30 {
                // Record the play in the database
                if let Some(db) = self.library.get_database() {
                    let duration = self.position_secs as u64;
                    if let Err(e) = db.record_play(path, duration) {
                        log::error!("Failed to record play: {}", e);
                    } else {
                        log::info!("Recorded play for {:?} ({}s)", path, duration);
                        self.current_track_already_recorded = true;
                    }
                }
            }
        }
    }

    /// Stop tracking the current track (called when track changes or stops)
    pub fn stop_track_tracking(&mut self) {
        self.current_track_path = None;
        self.current_track_start_time = None;
        self.current_track_already_recorded = false;
    }

    // ========================================================================
    // Favorites Methods
    // ========================================================================

    /// Toggle favorite on the currently selected album in library view
    pub fn toggle_selected_album_favorite(&mut self) {
        // Copy the index first to avoid borrow conflicts with filtered_albums()
        let idx = self.selected_album_index;
        let album_id = self.cached_filtered_albums.get(idx).and_then(|a| a.id);
        if let Some(album_id) = album_id {
            if let Some(db) = self.library.get_database() {
                match db.toggle_album_favorite(album_id) {
                    Ok(new_state) => {
                        // Update in-memory state
                        for a in &mut self.library.albums {
                            if a.id == Some(album_id) {
                                a.is_favorite = new_state;
                            }
                        }
                        // Invalidate filter cache since library data changed
                        self.request_filter_update();
                        log::info!(
                            "Toggled album favorite: id={} is_favorite={}",
                            album_id,
                            new_state
                        );
                    }
                    Err(e) => log::error!("Failed to toggle album favorite: {}", e),
                }
            }
        }
    }

    /// Toggle favorite on the current queue album
    pub fn toggle_current_queue_album_favorite(&mut self) {
        let album_id = self
            .current_queue_index
            .and_then(|idx| self.queue.get(idx))
            .and_then(|entry| entry.item.album.id);
        if let Some(album_id) = album_id {
            if let Some(db) = self.library.get_database() {
                match db.toggle_album_favorite(album_id) {
                    Ok(new_state) => {
                        // Update in queue
                        for qi in &mut self.queue {
                            if qi.item.album.id == Some(album_id) {
                                qi.item.album.is_favorite = new_state;
                            }
                        }
                        // Update in library
                        for a in &mut self.library.albums {
                            if a.id == Some(album_id) {
                                a.is_favorite = new_state;
                            }
                        }
                        log::info!(
                            "Toggled queue album favorite: id={} is_favorite={}",
                            album_id,
                            new_state
                        );
                    }
                    Err(e) => log::error!("Failed to toggle album favorite: {}", e),
                }
            }
        }
    }

    // ========================================================================
    // File Browser Methods
    // ========================================================================

    pub fn refresh_file_browser(&mut self) {
        self.file_browser_items.clear();
        self.selected_file_index = 0;

        // Add ".." entry to go up
        if let Some(parent) = self.current_browser_dir.parent() {
            self.file_browser_items.push(parent.to_path_buf());
        }

        if let Ok(entries) = std::fs::read_dir(&self.current_browser_dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.is_file() {
                    if let Some(ext) = &self.file_browser_extension {
                        if path
                            .extension()
                            .is_some_and(|e| e.to_string_lossy().to_lowercase() == *ext)
                        {
                            files.push(path);
                        }
                    } else {
                        files.push(path);
                    }
                }
            }

            dirs.sort();
            files.sort();

            self.file_browser_items.extend(dirs);
            self.file_browser_items.extend(files);
        }
    }

    pub fn navigate_file_browser(&mut self) -> Option<PathBuf> {
        if let Some(path) = self
            .file_browser_items
            .get(self.selected_file_index)
            .cloned()
        {
            if path.is_dir() {
                self.current_browser_dir = path;
                self.refresh_file_browser();
                None
            } else {
                Some(path)
            }
        } else {
            None
        }
    }

    pub fn select_next_file(&mut self) {
        if !self.file_browser_items.is_empty() {
            self.selected_file_index =
                (self.selected_file_index + 1) % self.file_browser_items.len();
        }
    }

    pub fn select_previous_file(&mut self) {
        if !self.file_browser_items.is_empty() {
            if self.selected_file_index == 0 {
                self.selected_file_index = self.file_browser_items.len() - 1;
            } else {
                self.selected_file_index -= 1;
            }
        }
    }
}

/// Helper function to cycle through path config presets for A/B Compare plugin
/// Returns JSON string for the selected path config
fn cycle_path_config(current: &str, forward: bool) -> String {
    // List of available path configs (None + common plugins)
    let presets = [
        (r#"{"type":"None"}"#, "None"),
        (
            r#"{"type":"Plugin","plugin_type":"EQ","parameters":{"filters":[]}}"#,
            "EQ",
        ),
        (
            r#"{"type":"Plugin","plugin_type":"gain","parameters":{"gain_db":0.0}}"#,
            "Gain",
        ),
        (
            r#"{"type":"Plugin","plugin_type":"compressor","parameters":{"threshold_db":-20.0,"ratio":4.0,"attack_ms":10.0,"release_ms":100.0,"knee_db":3.0,"makeup_gain_db":0.0,"mix":1.0}}"#,
            "Compressor",
        ),
        (
            r#"{"type":"Plugin","plugin_type":"limiter","parameters":{"threshold_db":-1.0,"release_ms":100.0,"lookahead_ms":5.0,"soft":false,"mix":1.0}}"#,
            "Limiter",
        ),
        (
            r#"{"type":"Plugin","plugin_type":"gate","parameters":{"threshold_db":-40.0,"ratio":10.0,"attack_ms":1.0,"hold_ms":50.0,"release_ms":100.0,"mix":1.0}}"#,
            "Gate",
        ),
        (
            r#"{"type":"Plugin","plugin_type":"expander","parameters":{"threshold_db":-40.0,"ratio":2.0,"attack_ms":5.0,"release_ms":50.0,"range_db":20.0,"knee_db":3.0,"hysteresis_db":2.0,"hold_ms":10.0,"mix":1.0}}"#,
            "Expander",
        ),
        (
            r#"{"type":"Plugin","plugin_type":"denoiser","parameters":{"reduction_db":12.0,"floor_db":-60.0,"smoothing":0.5,"attack_ms":5.0,"release_ms":50.0}}"#,
            "Denoiser",
        ),
        (
            r#"{"type":"Plugin","plugin_type":"loudness_compensation","parameters":{"low_freq":100.0,"low_gain":3.0,"high_freq":8000.0,"high_gain":2.0}}"#,
            "Loudness Comp",
        ),
    ];

    // Find current index
    let current_idx = presets
        .iter()
        .position(|(json, _)| *json == current)
        .unwrap_or(0);

    // Calculate new index
    let new_idx = if forward {
        (current_idx + 1) % presets.len()
    } else {
        (current_idx + presets.len() - 1) % presets.len()
    };

    presets[new_idx].0.to_string()
}

// Helper function to get parameter count for a plugin
fn get_param_count(settings: &sotf_audio_player::PluginSettings) -> usize {
    use sotf_audio_player::PluginSettings;
    match settings {
        PluginSettings::EQ { filters, .. } => filters.len() * 4, // freq, q, gain, type for each filter
        PluginSettings::Gain { .. } => 1,                        // gain_db
        PluginSettings::Upmixer { .. } => 35, // speaker_config, gains (4), lfe (2), stereo/spatial (3), subharm (2), hr (2), safety, decor mode, ext subharm (3), decor details (3), height details (3), surround/ambient (4), dialogue (3), bypass (3)
        PluginSettings::Compressor { .. } => 10, // threshold, ratio, attack, release, knee, makeup_gain, mix, auto_makeup, link_channels, sidechain_hpf_hz
        PluginSettings::Limiter { .. } => 5,     // threshold, release, lookahead, soft, mix
        PluginSettings::Gate { .. } => 8, // threshold, ratio, attack, hold, release, mix, link_channels, sidechain_hpf_hz
        PluginSettings::LoudnessCompensation { .. } => 7, // low_freq, low_gain, high_freq, high_gain, auto_gain_enabled, auto_gain_max_db, auto_gain_smoothing_ms
        PluginSettings::BinauralDecoder { .. } => 5, // sofa_file, input_channels, enable_optimization, externalization, near_field_strength
        PluginSettings::Convolution { .. } => 3,     // ir_file, mix, gain_db
        PluginSettings::LoudnessMonitor => 0,        // No parameters
        PluginSettings::SpectrumAnalyzer { .. } => 6, // num_bins, min_freq, max_freq, smoothing, tilt_correction, tilt_reference
        PluginSettings::ChannelMuteSolo { .. } => 1, // enabled toggle
        PluginSettings::Matrix { .. } => 2,          // input_channels, output_channels
        PluginSettings::Expander { .. } => 11, // threshold, ratio, attack, release, range, knee, hysteresis, hold, mix, link_channels, sidechain_hpf
        PluginSettings::MultibandCompressor { num_bands, .. } => 12 + num_bands * 8, // global + per-band (solo, bypass, threshold, ratio, attack, release, knee, makeup)
        PluginSettings::MultibandExpander { num_bands, .. } => 15 + num_bands * 10, // global + per-band (solo, bypass, threshold, ratio, attack, release, range, knee, hysteresis, hold)
        PluginSettings::XTC { .. } => 8, // distance, speaker_angle, head_radius, beta_base, beta_low_boost, beta_high_boost, head_shadow_cutoff, head_shadow_slope
        PluginSettings::Denoiser { .. } => 12, // reduction_db, floor_db, smoothing, attack_ms, release_ms, low_latency, polyphonic_detection, transparency, dd_enabled, dd_alpha, psychoacoustic_masking, use_captured_profile
        PluginSettings::Pnd { .. } => 3, // correction_strength, analysis_window_ms, drift_smoothing
        PluginSettings::ABCompare { .. } => 11, // mix, mix_mode, selected_path, bypass, auto_gain_enabled, loudness_type, max_auto_gain_db, gain_smoothing_ms, mix_transition_ms, path_a, path_b
        PluginSettings::FletcherMunson { .. } => 6, // reference_level, enabled, smoothing, auto_gain_enabled, max_auto_gain, auto_gain_smoothing
        PluginSettings::BandSplit { .. } => 2,      // frequency, crossover_type
        PluginSettings::BandMerge { .. } => 1,      // bands
        PluginSettings::Downmix { .. } => 7,
        PluginSettings::MonoToStereo { .. } => 6,
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new(Theme::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_audio_player::{Album, DirectoryInfo, Track};
    use sotf_audio_player::{PluginSettings, PluginType};
    use std::path::PathBuf;

    fn create_test_directory_info(path: &str) -> DirectoryInfo {
        DirectoryInfo {
            path: PathBuf::from(path),
            file_count: 10,
            album_count: 2,
            last_scanned: None,
            expanded: false,
            subdirectories: vec![],
        }
    }

    fn create_test_app_with_directories(num_dirs: usize) -> App {
        let mut app = App::new(Theme::default());
        for i in 0..num_dirs {
            app.library
                .directories
                .push(create_test_directory_info(&format!("/test/dir{}", i)));
        }
        app
    }

    #[test]
    fn test_select_next_directory() {
        let mut app = create_test_app_with_directories(3);

        assert_eq!(app.selected_directory_index, 0);

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 1);

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 2);

        // Should wrap around to 0
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_select_previous_directory() {
        let mut app = create_test_app_with_directories(3);

        assert_eq!(app.selected_directory_index, 0);

        // Should wrap around to last item
        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 2);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 1);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_page_down_directories() {
        let mut app = create_test_app_with_directories(30);

        assert_eq!(app.selected_directory_index, 0);

        // Page down by 20
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 20);

        // Page down by 20 again - should stop at max (29)
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 29);

        // Should stay at max
        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 29);
    }

    #[test]
    fn test_page_up_directories() {
        let mut app = create_test_app_with_directories(30);

        // Start at the end
        app.selected_directory_index = 29;

        // Page up by 20
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 9);

        // Page up by 20 again - should stop at 0
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);

        // Should stay at 0
        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_navigation_with_empty_directories() {
        let mut app = App::new(Theme::default());
        assert_eq!(app.selected_directory_index, 0);

        // Should not crash with empty directories
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0);

        app.select_previous_directory();
        assert_eq!(app.selected_directory_index, 0);

        app.page_down_directories(20);
        assert_eq!(app.selected_directory_index, 0);

        app.page_up_directories(20);
        assert_eq!(app.selected_directory_index, 0);
    }

    #[test]
    fn test_navigation_with_expanded_directories() {
        let mut app = create_test_app_with_directories(2);

        // Add subdirectories to first directory
        app.library.directories[0].subdirectories = vec![
            create_test_directory_info("/test/dir0/subdir1"),
            create_test_directory_info("/test/dir0/subdir2"),
        ];

        // Initially collapsed - tree has 2 items
        assert_eq!(app.get_directory_tree_items().len(), 2);
        assert_eq!(app.selected_directory_index, 0);

        // Expand first directory
        app.toggle_directory_expansion();

        // Now tree has 4 items: dir0, subdir1, subdir2, dir1
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 4);

        // Navigate through expanded tree
        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 1); // subdir1

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 2); // subdir2

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 3); // dir1

        app.select_next_directory();
        assert_eq!(app.selected_directory_index, 0); // wrap to dir0
    }

    #[test]
    fn test_get_directory_tree_items() {
        let mut app = create_test_app_with_directories(1);

        // Add subdirectories
        app.library.directories[0].subdirectories = vec![
            create_test_directory_info("/test/dir0/subdir1"),
            create_test_directory_info("/test/dir0/subdir2"),
        ];

        // Collapsed - should only show root
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 1);
        assert_eq!(tree_items[0].0, PathBuf::from("/test/dir0"));
        assert_eq!(tree_items[0].1, 0); // level
        assert_eq!(tree_items[0].2, false); // not expanded

        // Expand
        app.toggle_directory_expansion();

        // Should show root + 2 subdirectories
        let tree_items = app.get_directory_tree_items();
        assert_eq!(tree_items.len(), 3);
        assert_eq!(tree_items[0].0, PathBuf::from("/test/dir0"));
        assert_eq!(tree_items[0].1, 0); // level
        assert_eq!(tree_items[0].2, true); // expanded

        assert_eq!(tree_items[1].0, PathBuf::from("/test/dir0/subdir1"));
        assert_eq!(tree_items[1].1, 1); // level

        assert_eq!(tree_items[2].0, PathBuf::from("/test/dir0/subdir2"));
        assert_eq!(tree_items[2].1, 1); // level
    }

    fn create_test_album(artist: &str, title: &str, base_path: &str, track_count: usize) -> Album {
        let mut tracks = Vec::new();
        for i in 0..track_count {
            tracks.push(Track {
                path: PathBuf::from(format!("{}/track{}.flac", base_path, i)),
                title: None,
                artist: Some(artist.to_string()),
                track_number: Some(i as u32),
                duration_secs: None,
                channels: None,
                replay_gain: None,
                replay_peak: None,
                album_gain: None,
                album_peak: None,
                waveform: None,
                genre: None,
                composer: None,
                disc_number: None,
                conductor: None,
                performer: None,
                isrc: None,
                album_artist: None,
                ensemble: None,
                edition: None,
                is_favorite: false,
                play_count: 0,
                bit_depth: None,
                sample_rate: None,
            });
        }
        Album {
            id: None,
            title: title.to_string(),
            year: None,
            tracks,
            album_art_path: None,
            album_art_thumbnail: None,
            play_count: 0,
            edition: None,
            dynamic_range: None,
            is_favorite: false,
        }
    }

    #[test]
    fn test_next_track_removes_finished_album_and_advances() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);

        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.current_queue_index = Some(0);
        app.is_playing = true;

        let first_path = app.current_track_path().unwrap();
        assert!(first_path.to_string_lossy().contains("track0.flac"));

        let second_path = app.next_track().unwrap();
        assert!(second_path.to_string_lossy().contains("track1.flac"));
        assert_eq!(app.queue.len(), 2);
        assert_eq!(app.current_queue_index, Some(0));

        let third_path = app.next_track().unwrap();
        assert!(third_path.to_string_lossy().contains("album2/track0.flac"));
        assert_eq!(app.queue.len(), 1);
        assert_eq!(app.current_queue_index, Some(0));

        let fourth_path = app.next_track().unwrap();
        assert!(fourth_path.to_string_lossy().contains("album2/track1.flac"));

        let none = app.next_track();
        assert!(none.is_none());
        assert!(app.queue.is_empty());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);
    }

    #[test]
    fn test_adjust_eq_parameters() {
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::EQ);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert!(!filters.is_empty());

        let orig_freq = filters[0].frequency;
        let orig_q = filters[0].q;
        let orig_gain = filters[0].gain_db;
        let orig_type = filters[0].filter_type;

        // Frequency
        app.plugin_param_selection = 0;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].frequency, orig_freq);

        // Q
        app.plugin_param_selection = 1;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].q, orig_q);

        // Gain
        app.plugin_param_selection = 2;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].gain_db, orig_gain);

        // Type
        app.plugin_param_selection = 3;
        assert!(app.adjust_selected_param(1.0));
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let filters = match &plugin.settings {
            PluginSettings::EQ { filters, .. } => filters,
            _ => panic!("Expected EQ plugin"),
        };
        assert_ne!(filters[0].filter_type, orig_type);
    }

    #[test]
    fn test_adjust_upmixer_parameters() {
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Upmixer);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (
            orig_speaker_config,
            orig_front_direct,
            orig_front_ambient,
            orig_rear_ambient,
            orig_height_gain,
            orig_lfe_gain,
            orig_lfe_cutoff,
            orig_stereo_width,
            orig_center_spread,
            orig_bandpass,
            orig_enable_subharm,
            orig_subharm_gain,
            orig_enable_hr_direct,
            orig_hr_sharpen,
            orig_safety_cap_db,
        ) = match &plugin.settings {
            PluginSettings::Upmixer {
                speaker_config,
                gain_front_direct,
                gain_front_ambient,
                gain_rear_ambient,
                height_gain,
                lfe_gain,
                lfe_cutoff_hz,
                stereo_width,
                center_spread,
                bandpass_hz,
                enable_subharmonic_synth,
                subharmonic_gain,
                enable_hr_direct,
                hr_sharpen,
                safety_cap_db,
                ..
            } => (
                speaker_config.clone(),
                *gain_front_direct,
                *gain_front_ambient,
                *gain_rear_ambient,
                *height_gain,
                *lfe_gain,
                *lfe_cutoff_hz,
                *stereo_width,
                *center_spread,
                *bandpass_hz,
                *enable_subharmonic_synth,
                *subharmonic_gain,
                *enable_hr_direct,
                *hr_sharpen,
                *safety_cap_db,
            ),
            _ => panic!("Expected Upmixer plugin"),
        };

        // Indices match new get_params() order:
        // 0: speaker_config, 1-4: gains, 5: lfe_gain, 6: lfe_cutoff, 7: stereo_width,
        // 8: center_spread, 9: bandpass, 10: enable_subharm (toggle), 11: subharm_gain,
        // 12: enable_hr_direct (toggle), 13: hr_sharpen, 14: safety_cap
        for idx in 0..15 {
            app.plugin_param_selection = idx;
            // hr_sharpen (13) defaults to 1.0 and safety_cap_db (14) defaults to 3.0, both at max
            let delta = if idx == 13 || idx == 14 { -1.0 } else { 1.0 };
            assert!(app.adjust_selected_param(delta));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Upmixer {
            speaker_config,
            gain_front_direct,
            gain_front_ambient,
            gain_rear_ambient,
            height_gain,
            lfe_gain,
            lfe_cutoff_hz,
            stereo_width,
            center_spread,
            bandpass_hz,
            enable_subharmonic_synth,
            subharmonic_gain,
            enable_hr_direct,
            hr_sharpen,
            safety_cap_db,
            ..
        } = &plugin.settings
        {
            assert_ne!(*speaker_config, orig_speaker_config);
            assert_ne!(*gain_front_direct, orig_front_direct);
            assert_ne!(*gain_front_ambient, orig_front_ambient);
            assert_ne!(*gain_rear_ambient, orig_rear_ambient);
            assert_ne!(*height_gain, orig_height_gain);
            assert_ne!(*lfe_gain, orig_lfe_gain);
            assert_ne!(*lfe_cutoff_hz, orig_lfe_cutoff);
            assert_ne!(*stereo_width, orig_stereo_width);
            assert_ne!(*center_spread, orig_center_spread);
            assert_ne!(*bandpass_hz, orig_bandpass);
            assert_ne!(*enable_subharmonic_synth, orig_enable_subharm);
            assert_ne!(*subharmonic_gain, orig_subharm_gain);
            assert_ne!(*enable_hr_direct, orig_enable_hr_direct);
            assert_ne!(*hr_sharpen, orig_hr_sharpen);
            assert_ne!(*safety_cap_db, orig_safety_cap_db);
        } else {
            panic!("Expected Upmixer plugin");
        }
    }

    #[test]
    fn test_adjust_compressor_limiter_gate_loudness_parameters() {
        // Compressor
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Compressor);
        app.editing_plugin_index = Some(plugin_idx);

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (
            orig_thresh,
            orig_ratio,
            orig_attack,
            orig_release,
            orig_knee,
            orig_makeup,
            orig_mix,
            orig_auto_makeup,
            orig_link_channels,
            orig_sidechain_hpf,
        ) = match &plugin.settings {
            PluginSettings::Compressor {
                threshold_db,
                ratio,
                attack_ms,
                release_ms,
                knee_db,
                makeup_gain_db,
                mix,
                auto_makeup,
                link_channels,
                sidechain_hpf_hz,
            } => (
                *threshold_db,
                *ratio,
                *attack_ms,
                *release_ms,
                *knee_db,
                *makeup_gain_db,
                *mix,
                *auto_makeup,
                *link_channels,
                *sidechain_hpf_hz,
            ),
            _ => panic!("Expected Compressor plugin"),
        };

        for idx in 0..10 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Compressor {
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_db,
            makeup_gain_db,
            mix,
            auto_makeup,
            link_channels,
            sidechain_hpf_hz,
        } = &plugin.settings
        {
            assert_ne!(*threshold_db, orig_thresh);
            assert_ne!(*ratio, orig_ratio);
            assert_ne!(*attack_ms, orig_attack);
            assert_ne!(*release_ms, orig_release);
            assert_ne!(*knee_db, orig_knee);
            assert_ne!(*makeup_gain_db, orig_makeup);
            assert_ne!(*mix, orig_mix);
            assert_ne!(*auto_makeup, orig_auto_makeup);
            assert_ne!(*link_channels, orig_link_channels);
            assert_ne!(*sidechain_hpf_hz, orig_sidechain_hpf);
        }

        // Limiter (use -1.0 since mix starts at 1.0 which is max)
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Limiter);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_thresh, orig_rel, orig_look, orig_soft, orig_mix) = match &plugin.settings {
            PluginSettings::Limiter {
                threshold_db,
                release_ms,
                lookahead_ms,
                soft,
                mix,
            } => (*threshold_db, *release_ms, *lookahead_ms, *soft, *mix),
            _ => panic!("Expected Limiter plugin"),
        };
        for idx in 0..5 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(-1.0));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Limiter {
            threshold_db,
            release_ms,
            lookahead_ms,
            soft,
            mix,
        } = &plugin.settings
        {
            assert_ne!(*threshold_db, orig_thresh);
            assert_ne!(*release_ms, orig_rel);
            assert_ne!(*lookahead_ms, orig_look);
            assert_ne!(*soft, orig_soft);
            assert_ne!(*mix, orig_mix);
        }

        // Gate - test parameters individually since mix starts at max (1.0) and hpf at min (0.0)
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::Gate);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (
            orig_thresh,
            orig_ratio,
            orig_attack,
            orig_hold,
            orig_release,
            orig_mix,
            orig_link,
            orig_hpf,
        ) = match &plugin.settings {
            PluginSettings::Gate {
                threshold_db,
                ratio,
                attack_ms,
                hold_ms,
                release_ms,
                mix,
                link_channels,
                sidechain_hpf_hz,
            } => (
                *threshold_db,
                *ratio,
                *attack_ms,
                *hold_ms,
                *release_ms,
                *mix,
                *link_channels,
                *sidechain_hpf_hz,
            ),
            _ => panic!("Expected Gate plugin"),
        };
        // Adjust each parameter - mix (idx 5) decreases, hpf (idx 7) increases, others can go either way
        for idx in 0..8 {
            app.plugin_param_selection = idx;
            let delta = if idx == 5 { -1.0 } else { 1.0 }; // mix starts at max, decrease it
            assert!(app.adjust_selected_param(delta));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::Gate {
            threshold_db,
            ratio,
            attack_ms,
            hold_ms,
            release_ms,
            mix,
            link_channels,
            sidechain_hpf_hz,
        } = &plugin.settings
        {
            assert_ne!(*threshold_db, orig_thresh);
            assert_ne!(*ratio, orig_ratio);
            assert_ne!(*attack_ms, orig_attack);
            assert_ne!(*hold_ms, orig_hold);
            assert_ne!(*release_ms, orig_release);
            assert_ne!(*mix, orig_mix);
            assert_ne!(*link_channels, orig_link);
            assert_ne!(*sidechain_hpf_hz, orig_hpf);
        }

        // Loudness compensation
        let mut app = App::new(Theme::default());
        let plugin_idx = app
            .plugin_chain
            .add_plugin(&PluginType::LoudnessCompensation);
        app.editing_plugin_index = Some(plugin_idx);
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_low_freq, orig_low_gain, orig_high_freq, orig_high_gain) = match &plugin.settings
        {
            PluginSettings::LoudnessCompensation {
                low_freq,
                low_gain,
                high_freq,
                high_gain,
                ..
            } => (*low_freq, *low_gain, *high_freq, *high_gain),
            _ => panic!("Expected LoudnessCompensation plugin"),
        };
        for idx in 0..4 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }
        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::LoudnessCompensation {
            low_freq,
            low_gain,
            high_freq,
            high_gain,
            ..
        } = &plugin.settings
        {
            assert_ne!(*low_freq, orig_low_freq);
            assert_ne!(*low_gain, orig_low_gain);
            assert_ne!(*high_freq, orig_high_freq);
            assert_ne!(*high_gain, orig_high_gain);
        }
    }

    #[test]
    fn test_adjust_binaural_decoder_parameters_and_set_sofa() {
        let mut app = App::new(Theme::default());
        let plugin_idx = app.plugin_chain.add_plugin(&PluginType::BinauralDecoder);
        app.editing_plugin_index = Some(plugin_idx);
        app.selected_plugin_index = plugin_idx;

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        let (orig_sofa, orig_channels, orig_opt, orig_ext, orig_near) = match &plugin.settings {
            PluginSettings::BinauralDecoder {
                sofa_file,
                input_channels,
                enable_optimization,
                externalization,
                near_field_strength,
            } => (
                sofa_file.clone(),
                *input_channels,
                *enable_optimization,
                *externalization,
                *near_field_strength,
            ),
            _ => panic!("Expected BinauralDecoder plugin"),
        };

        // Adjust numeric / boolean parameters via adjust_selected_param
        for idx in 1..5 {
            app.plugin_param_selection = idx;
            assert!(app.adjust_selected_param(1.0));
        }

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::BinauralDecoder {
            sofa_file,
            input_channels,
            enable_optimization,
            externalization,
            near_field_strength,
        } = &plugin.settings
        {
            assert_eq!(*sofa_file, orig_sofa); // unchanged by adjust_selected_param
            assert_ne!(*input_channels, orig_channels);
            assert_ne!(*enable_optimization, orig_opt);
            assert_ne!(*externalization, orig_ext);
            assert_ne!(*near_field_strength, orig_near);
        } else {
            panic!("Expected BinauralDecoder plugin");
        }

        // Now set SOFA file via load_sofa_file path
        app.sofa_file_input = "/tmp/test.sofa".to_string();
        app.load_sofa_file().unwrap();

        let plugin = app.plugin_chain.get_plugin(plugin_idx).unwrap();
        if let PluginSettings::BinauralDecoder { sofa_file, .. } = &plugin.settings {
            assert_eq!(sofa_file, "/tmp/test.sofa");
        } else {
            panic!("Expected BinauralDecoder plugin");
        }
    }

    // ============================================================================
    // QueueItem Unit Tests
    // ============================================================================

    #[test]
    fn test_queue_item_new() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let queue_item = QueueItem::new(album);

        assert_eq!(queue_item.current_track_index, 0);
        assert_eq!(queue_item.album.title, "Album");
        assert_eq!(queue_item.album.tracks.len(), 3);
    }

    #[test]
    fn test_queue_item_current_track() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let queue_item = QueueItem::new(album);

        let track = queue_item.current_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track0.flac"));
    }

    #[test]
    fn test_queue_item_next_track() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let mut queue_item = QueueItem::new(album);

        assert_eq!(queue_item.current_track_index, 0);

        // Advance to next track
        let track = queue_item.next_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track1.flac"));
        assert_eq!(queue_item.current_track_index, 1);

        // Advance again
        let track = queue_item.next_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track2.flac"));
        assert_eq!(queue_item.current_track_index, 2);

        // No more tracks
        assert!(queue_item.next_track().is_none());
        assert_eq!(queue_item.current_track_index, 2); // Index unchanged
    }

    #[test]
    fn test_queue_item_previous_track() {
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        let mut queue_item = QueueItem::new(album);

        // Start at last track
        queue_item.current_track_index = 2;

        // Go back
        let track = queue_item.previous_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track1.flac"));
        assert_eq!(queue_item.current_track_index, 1);

        // Go back again
        let track = queue_item.previous_track().unwrap();
        assert!(track.path.to_string_lossy().contains("track0.flac"));
        assert_eq!(queue_item.current_track_index, 0);

        // Can't go back further
        assert!(queue_item.previous_track().is_none());
        assert_eq!(queue_item.current_track_index, 0); // Index unchanged
    }

    #[test]
    fn test_queue_item_empty_album() {
        let album = create_test_album("Artist", "Empty Album", "/music/empty", 0);
        let mut queue_item = QueueItem::new(album);

        assert!(queue_item.current_track().is_none());
        assert!(queue_item.next_track().is_none());
        assert!(queue_item.previous_track().is_none());
    }

    // ============================================================================
    // Volume Control Tests
    // ============================================================================

    #[test]
    fn test_increase_volume() {
        let mut app = App::new(Theme::default());
        app.volume = 0.5;

        app.increase_volume();
        assert!((app.volume - 0.55).abs() < 0.001);

        // Keep increasing
        for _ in 0..20 {
            app.increase_volume();
        }
        // Should clamp at 1.0
        assert!((app.volume - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_decrease_volume() {
        let mut app = App::new(Theme::default());
        app.volume = 0.5;

        app.decrease_volume();
        assert!((app.volume - 0.45).abs() < 0.001);

        // Keep decreasing
        for _ in 0..20 {
            app.decrease_volume();
        }
        // Should clamp at 0.0
        assert!((app.volume - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_volume_boundary_values() {
        let mut app = App::new(Theme::default());

        // Start at 0
        app.volume = 0.0;
        app.decrease_volume();
        assert_eq!(app.volume, 0.0);

        // Start at 1
        app.volume = 1.0;
        app.increase_volume();
        assert_eq!(app.volume, 1.0);
    }

    // ============================================================================
    // Queue Management Tests
    // ============================================================================

    #[test]
    fn test_clear_queue() {
        let mut app = App::new(Theme::default());

        // Add some items to queue
        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.current_queue_index = Some(1);
        app.is_playing = true;

        app.clear_queue();

        assert!(app.queue.is_empty());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);
    }

    #[test]
    fn test_remove_from_queue_first_item() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist", "Album3", "/music/album3", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
            QueueEntry::new(QueueItem::new(album3)),
        ];
        app.current_queue_index = Some(1);

        // Remove first item
        app.remove_from_queue(0);

        assert_eq!(app.queue.len(), 2);
        assert_eq!(app.queue[0].item.album.title, "Album2");
        assert_eq!(app.current_queue_index, Some(0)); // Adjusted
    }

    #[test]
    fn test_remove_from_queue_current_playing() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.current_queue_index = Some(0);
        app.is_playing = true;

        // Remove currently playing item
        app.remove_from_queue(0);

        assert_eq!(app.queue.len(), 1);
        assert_eq!(app.queue[0].item.album.title, "Album2");
        // Current queue index should remain at 0 (now pointing to Album2)
        assert_eq!(app.current_queue_index, Some(0));
    }

    #[test]
    fn test_remove_from_queue_last_item() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        app.queue = vec![QueueEntry::new(QueueItem::new(album1))];
        app.current_queue_index = Some(0);
        app.is_playing = true;

        // Remove last item
        app.remove_from_queue(0);

        assert!(app.queue.is_empty());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);
    }

    #[test]
    fn test_toggle_queue_item_expansion() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 1;

        // Toggle expansion
        app.toggle_queue_item_expansion();
        assert!(!app.queue[0].expanded);
        assert!(app.queue[1].expanded);

        // Toggle again
        app.toggle_queue_item_expansion();
        assert!(!app.queue[1].expanded);
    }

    #[test]
    fn test_select_next_queue_item() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 0;

        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 1);

        // Wrap around
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
    }

    #[test]
    fn test_select_previous_queue_item() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 0;

        // Wrap around to last
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 1);

        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
    }

    #[test]
    fn test_queue_navigation_empty() {
        let mut app = App::new(Theme::default());
        app.selected_queue_index = 0;

        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);

        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
    }

    #[test]
    fn test_queue_navigation_into_expanded_tracks() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 3);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.selected_queue_index = 0;
        app.queue[0].expanded = true;

        // Start on album header
        assert_eq!(app.selected_queue_track_index, None);

        // Down → first track
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(0));

        // Down → second track
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(1));

        // Down → third track
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(2));

        // Down → next album header
        app.select_next_queue_item();
        assert_eq!(app.selected_queue_index, 1);
        assert_eq!(app.selected_queue_track_index, None);
    }

    #[test]
    fn test_queue_navigation_previous_into_expanded_tracks() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue = vec![
            QueueEntry::new(QueueItem::new(album1)),
            QueueEntry::new(QueueItem::new(album2)),
        ];
        app.queue[0].expanded = true;
        app.selected_queue_index = 1;

        // Up from album2 header → last track of expanded album1
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(1));

        // Up → first track
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, Some(0));

        // Up → album1 header
        app.select_previous_queue_item();
        assert_eq!(app.selected_queue_index, 0);
        assert_eq!(app.selected_queue_track_index, None);
    }

    #[test]
    fn test_collapse_resets_track_selection() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 3);
        app.queue = vec![QueueEntry::new(QueueItem::new(album1))];
        app.queue[0].expanded = true;
        app.selected_queue_index = 0;
        app.selected_queue_track_index = Some(1);

        // Left on a track → moves to album header
        app.collapse_queue_item();
        assert!(app.queue[0].expanded); // still expanded
        assert_eq!(app.selected_queue_track_index, None);

        // Left on album header → collapses
        app.collapse_queue_item();
        assert!(!app.queue[0].expanded);
    }

    #[test]
    fn test_jump_to_selected_track() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 3);
        app.queue = vec![QueueEntry::new(QueueItem::new(album1))];
        app.selected_queue_index = 0;
        app.selected_queue_track_index = Some(2);

        app.jump_to_selected_album();
        assert_eq!(app.queue[0].item.current_track_index, 2);
    }

    // ============================================================================
    // Album Navigation Tests
    // ============================================================================

    fn create_test_app_with_albums(num_albums: usize) -> App {
        let mut app = App::new(Theme::default());
        for i in 0..num_albums {
            let album = create_test_album(
                &format!("Artist{}", i),
                &format!("Album{}", i),
                &format!("/music/album{}", i),
                3,
            );
            app.library.albums.push(album);
        }
        app
    }

    #[test]
    fn test_select_next_album() {
        let mut app = create_test_app_with_albums(5);
        app.selected_album_index = 0;

        app.select_next_album();
        assert_eq!(app.selected_album_index, 1);

        app.select_next_album();
        assert_eq!(app.selected_album_index, 2);
    }

    #[test]
    fn test_select_previous_album() {
        let mut app = create_test_app_with_albums(5);
        app.selected_album_index = 2;

        app.select_previous_album();
        assert_eq!(app.selected_album_index, 1);

        app.select_previous_album();
        assert_eq!(app.selected_album_index, 0);

        // Wraps around to last album
        app.select_previous_album();
        assert_eq!(app.selected_album_index, 4);
    }

    #[test]
    fn test_page_down_albums() {
        let mut app = create_test_app_with_albums(30);
        app.selected_album_index = 0;

        app.page_down_albums(10);
        assert_eq!(app.selected_album_index, 10);

        app.page_down_albums(10);
        assert_eq!(app.selected_album_index, 20);

        // Should stop at max (29)
        app.page_down_albums(20);
        assert_eq!(app.selected_album_index, 29);
    }

    #[test]
    fn test_page_up_albums() {
        let mut app = create_test_app_with_albums(30);
        app.selected_album_index = 25;

        app.page_up_albums(10);
        assert_eq!(app.selected_album_index, 15);

        app.page_up_albums(10);
        assert_eq!(app.selected_album_index, 5);

        // Should stop at 0
        app.page_up_albums(10);
        assert_eq!(app.selected_album_index, 0);
    }

    #[test]
    fn test_album_navigation_empty_library() {
        let mut app = App::new(Theme::default());
        app.selected_album_index = 0;

        app.select_next_album();
        assert_eq!(app.selected_album_index, 0);

        app.page_down_albums(10);
        assert_eq!(app.selected_album_index, 0);
    }

    // ============================================================================
    // Plugin Management Tests
    // ============================================================================

    #[test]
    fn test_add_plugin() {
        let mut app = App::new(Theme::default());
        // App starts with 2 default plugins: LoudnessMonitor and ChannelMuteSolo
        let initial_count = app.plugin_chain.len();
        assert!(initial_count >= 2, "App should start with default plugins");

        app.add_plugin(&PluginType::Gain);
        assert_eq!(app.plugin_chain.len(), initial_count + 1);
        assert!(app.needs_plugin_update);

        app.add_plugin(&PluginType::EQ);
        assert_eq!(app.plugin_chain.len(), initial_count + 2);
    }

    #[test]
    fn test_remove_plugin() {
        let mut app = App::new(Theme::default());
        let initial_count = app.plugin_chain.len();

        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);
        app.add_plugin(&PluginType::Limiter);

        assert_eq!(app.plugin_chain.len(), initial_count + 3);

        // Remove one of our added plugins (index after the defaults)
        app.remove_plugin(initial_count);
        assert_eq!(app.plugin_chain.len(), initial_count + 2);
        assert!(app.needs_plugin_update);
    }

    #[test]
    fn test_toggle_plugin() {
        let mut app = App::new(Theme::default());
        app.add_plugin(&PluginType::Gain);

        // Check initial state (enabled)
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert!(plugin.enabled);

        // Toggle off
        app.toggle_plugin(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert!(!plugin.enabled);

        // Toggle on
        app.toggle_plugin(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert!(plugin.enabled);
    }

    #[test]
    fn test_move_plugin_up() {
        let mut app = App::new(Theme::default());
        let base_idx = app.plugin_chain.user_plugin_insert_index();
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);
        app.add_plugin(&PluginType::Limiter);

        // Move limiter up (from base_idx + 2 to base_idx + 1)
        app.move_plugin_up(base_idx + 2);

        // Limiter should now be at base_idx + 1
        let plugin = app.plugin_chain.get_plugin(base_idx + 1).unwrap();
        assert!(matches!(plugin.plugin_type(), PluginType::Limiter));
    }

    #[test]
    fn test_move_plugin_down() {
        let mut app = App::new(Theme::default());
        let base_idx = app.plugin_chain.user_plugin_insert_index();
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);
        app.add_plugin(&PluginType::Limiter);

        // Move gain down (from base_idx to base_idx + 1)
        app.move_plugin_down(base_idx);

        // Gain should now be at base_idx + 1
        let plugin = app.plugin_chain.get_plugin(base_idx + 1).unwrap();
        assert!(matches!(plugin.plugin_type(), PluginType::Gain));
    }

    #[test]
    fn test_move_plugin_boundary() {
        let mut app = App::new(Theme::default());
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);

        // Try to move first plugin (index 0) up - should do nothing
        let first_plugin_type = app.plugin_chain.get_plugin(0).unwrap().plugin_type();
        app.move_plugin_up(0);
        let plugin = app.plugin_chain.get_plugin(0).unwrap();
        assert_eq!(plugin.plugin_type(), first_plugin_type);

        // Try to move last plugin down (should do nothing)
        let last_idx = app.plugin_chain.len() - 1;
        let last_plugin_type = app.plugin_chain.get_plugin(last_idx).unwrap().plugin_type();
        app.move_plugin_down(last_idx);
        let plugin = app.plugin_chain.get_plugin(last_idx).unwrap();
        assert_eq!(plugin.plugin_type(), last_plugin_type);
    }

    #[test]
    fn test_select_next_plugin() {
        let mut app = App::new(Theme::default());
        app.add_plugin(&PluginType::Gain);
        app.add_plugin(&PluginType::EQ);

        let total_plugins = app.plugin_chain.len();
        app.selected_plugin_index = 0;

        // Navigate through all plugins
        for i in 1..total_plugins {
            app.select_next_plugin();
            assert_eq!(app.selected_plugin_index, i);
        }

        // Wrap around to 0
        app.select_next_plugin();
        assert_eq!(app.selected_plugin_index, 0);
    }

    #[test]
    fn test_select_previous_plugin() {
        let mut app = App::new(Theme::default());
        app.add_plugin(&PluginType::Gain);

        let total_plugins = app.plugin_chain.len();
        app.selected_plugin_index = 0;

        // Wrap to last
        app.select_previous_plugin();
        assert_eq!(app.selected_plugin_index, total_plugins - 1);

        // Navigate back to 0
        for _ in 1..total_plugins {
            app.select_previous_plugin();
        }
        assert_eq!(app.selected_plugin_index, 0);
    }

    #[test]
    fn test_enter_exit_plugin_edit_mode() {
        let mut app = App::new(Theme::default());
        app.add_plugin(&PluginType::EQ);
        app.selected_plugin_index = 0;

        assert!(app.editing_plugin_index.is_none());

        app.enter_plugin_edit_mode();
        assert_eq!(app.editing_plugin_index, Some(0));
        assert_eq!(app.plugin_param_selection, 0);

        app.exit_plugin_edit_mode();
        assert!(app.editing_plugin_index.is_none());
    }

    // ============================================================================
    // Library View Mode Tests
    // ============================================================================

    #[test]
    fn test_toggle_library_view_mode() {
        let mut app = App::new(Theme::default());
        assert_eq!(app.library_view_mode, LibraryViewMode::Flat);

        app.toggle_library_view_mode();
        assert_eq!(app.library_view_mode, LibraryViewMode::TreeView);

        app.toggle_library_view_mode();
        assert_eq!(app.library_view_mode, LibraryViewMode::Flat);
    }

    #[test]
    fn test_set_library_sort_order() {
        let mut app = App::new(Theme::default());

        app.set_library_sort_order(LibrarySortOrder::Artist);
        assert_eq!(app.library_sort_order, LibrarySortOrder::Artist);

        app.set_library_sort_order(LibrarySortOrder::Album);
        assert_eq!(app.library_sort_order, LibrarySortOrder::Album);

        app.set_library_sort_order(LibrarySortOrder::Year);
        assert_eq!(app.library_sort_order, LibrarySortOrder::Year);
    }

    #[test]
    fn test_set_channel_filter() {
        let mut app = App::new(Theme::default());

        app.set_channel_filter(ChannelFilter::All);
        assert_eq!(app.channel_filter, ChannelFilter::All);

        app.set_channel_filter(ChannelFilter::Stereo);
        assert_eq!(app.channel_filter, ChannelFilter::Stereo);

        app.set_channel_filter(ChannelFilter::Surround);
        assert_eq!(app.channel_filter, ChannelFilter::Surround);
    }

    #[test]
    fn test_cycle_channel_filter() {
        let mut app = App::new(Theme::default());
        app.channel_filter = ChannelFilter::All;

        // Cycling depends on available channel counts, so test basic cycling
        // When library is empty, cycling should still work
        let initial = app.channel_filter;
        app.cycle_channel_filter();
        // After cycling, filter may or may not change depending on library
        // At minimum, it shouldn't panic
        let _ = app.channel_filter;

        // Reset
        app.channel_filter = initial;
    }

    // ============================================================================
    // Tree View Tests
    // ============================================================================

    #[test]
    fn test_rebuild_artist_tree() {
        let mut app = App::new(Theme::default());

        // Add albums with different artists
        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist A", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist B", "Album3", "/music/album3", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.library.albums.push(album3);

        app.rebuild_artist_tree();

        // Should have 2 artists
        assert_eq!(app.artist_tree.len(), 2);

        // Find Artist A node - should have 2 albums
        let artist_a = app
            .artist_tree
            .iter()
            .find(|n| n.artist == "Artist A")
            .unwrap();
        assert_eq!(artist_a.album_indices.len(), 2);

        // Find Artist B node - should have 1 album
        let artist_b = app
            .artist_tree
            .iter()
            .find(|n| n.artist == "Artist B")
            .unwrap();
        assert_eq!(artist_b.album_indices.len(), 1);
    }

    #[test]
    fn test_toggle_artist_expansion() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist B", "Album2", "/music/album2", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.rebuild_artist_tree();

        app.library_view_mode = LibraryViewMode::TreeView;
        app.selected_tree_index = 0;

        // Initially collapsed
        assert!(!app.artist_tree[0].expanded);

        // Toggle expansion
        app.toggle_artist_expansion();
        assert!(app.artist_tree[0].expanded);

        // Toggle again
        app.toggle_artist_expansion();
        assert!(!app.artist_tree[0].expanded);
    }

    #[test]
    fn test_get_tree_items_collapsed() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist A", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist B", "Album3", "/music/album3", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.library.albums.push(album3);
        app.rebuild_artist_tree();

        // All collapsed - should only show artists
        let items = app.get_tree_items();
        assert_eq!(items.len(), 2);

        // Both should be Artist items
        for item in &items {
            assert!(matches!(item, TreeItem::Artist { .. }));
        }
    }

    #[test]
    fn test_get_tree_items_expanded() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist A", "Album2", "/music/album2", 2);
        let album3 = create_test_album("Artist B", "Album3", "/music/album3", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.library.albums.push(album3);
        app.rebuild_artist_tree();

        // Expand first artist
        app.artist_tree[0].expanded = true;

        let items = app.get_tree_items();
        // Artist A (expanded) + 2 albums + Artist B (collapsed) = 4 items
        assert_eq!(items.len(), 4);

        // First should be Artist A (expanded)
        assert!(
            matches!(&items[0], TreeItem::Artist { name, expanded } if name == "Artist A" && *expanded)
        );

        // Next two should be albums
        assert!(matches!(&items[1], TreeItem::Album { .. }));
        assert!(matches!(&items[2], TreeItem::Album { .. }));

        // Last should be Artist B (collapsed)
        assert!(
            matches!(&items[3], TreeItem::Artist { name, expanded } if name == "Artist B" && !*expanded)
        );
    }

    #[test]
    fn test_select_next_tree_item() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist B", "Album2", "/music/album2", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.rebuild_artist_tree();

        app.library_view_mode = LibraryViewMode::TreeView;
        app.selected_tree_index = 0;

        app.select_next_tree_item();
        assert_eq!(app.selected_tree_index, 1);

        // Should wrap
        app.select_next_tree_item();
        assert_eq!(app.selected_tree_index, 0);
    }

    #[test]
    fn test_select_previous_tree_item() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist A", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist B", "Album2", "/music/album2", 2);
        app.library.albums.push(album1);
        app.library.albums.push(album2);
        app.rebuild_artist_tree();

        app.library_view_mode = LibraryViewMode::TreeView;
        app.selected_tree_index = 0;

        // Wrap to last
        app.select_previous_tree_item();
        assert_eq!(app.selected_tree_index, 1);

        app.select_previous_tree_item();
        assert_eq!(app.selected_tree_index, 0);
    }

    // ============================================================================
    // Output Device Tests
    // ============================================================================

    fn create_test_audio_device(name: &str, is_default: bool) -> AudioDevice {
        AudioDevice {
            device_id: Some(format!("test-device-{}", name)),
            name: name.to_string(),
            display_info: None,
            is_input: false,
            is_default,
            supported_configs: vec![],
            default_config: None,
            available_sample_rates: vec![44100, 48000, 96000],
        }
    }

    #[test]
    fn test_select_next_output_device() {
        let mut app = App::new(Theme::default());

        // Simulate having some devices
        app.output_devices = vec![
            create_test_audio_device("Device 1", true),
            create_test_audio_device("Device 2", false),
        ];
        app.selected_output_device_index = 0;

        app.select_next_output_device();
        assert_eq!(app.selected_output_device_index, 1);

        // Wrap
        app.select_next_output_device();
        assert_eq!(app.selected_output_device_index, 0);
    }

    #[test]
    fn test_select_previous_output_device() {
        let mut app = App::new(Theme::default());

        app.output_devices = vec![
            create_test_audio_device("Device 1", true),
            create_test_audio_device("Device 2", false),
        ];
        app.selected_output_device_index = 0;

        // Wrap to last
        app.select_previous_output_device();
        assert_eq!(app.selected_output_device_index, 1);

        app.select_previous_output_device();
        assert_eq!(app.selected_output_device_index, 0);
    }

    #[test]
    fn test_get_selected_output_device() {
        let mut app = App::new(Theme::default());

        // Empty devices
        assert!(app.get_selected_output_device().is_none());

        app.output_devices = vec![create_test_audio_device("Test Device", false)];
        app.selected_output_device_index = 0;

        let device = app.get_selected_output_device().unwrap();
        assert_eq!(device.name, "Test Device");
    }

    #[test]
    fn test_output_device_navigation_empty() {
        let mut app = App::new(Theme::default());
        app.selected_output_device_index = 0;

        // Should not panic with empty devices
        app.select_next_output_device();
        assert_eq!(app.selected_output_device_index, 0);

        app.select_previous_output_device();
        assert_eq!(app.selected_output_device_index, 0);
    }

    // ============================================================================
    // Screen and Mode Tests
    // ============================================================================

    #[test]
    fn test_screen_variants() {
        let mut app = App::new(Theme::default());

        app.current_screen = Screen::Library;
        assert_eq!(app.current_screen, Screen::Library);

        app.current_screen = Screen::DirectoryManager;
        assert_eq!(app.current_screen, Screen::DirectoryManager);

        app.current_screen = Screen::Queue;
        assert_eq!(app.current_screen, Screen::Queue);

        app.current_screen = Screen::Plugins;
        assert_eq!(app.current_screen, Screen::Plugins);

        app.current_screen = Screen::Devices;
        assert_eq!(app.current_screen, Screen::Devices);
    }

    #[test]
    fn test_input_mode_variants() {
        let mut app = App::new(Theme::default());

        app.input_mode = InputMode::Normal;
        assert_eq!(app.input_mode, InputMode::Normal);

        app.input_mode = InputMode::Search;
        assert_eq!(app.input_mode, InputMode::Search);

        app.input_mode = InputMode::AddDirectory;
        assert_eq!(app.input_mode, InputMode::AddDirectory);

        app.input_mode = InputMode::EditPlugin;
        assert_eq!(app.input_mode, InputMode::EditPlugin);

        app.input_mode = InputMode::ShowHelp;
        assert_eq!(app.input_mode, InputMode::ShowHelp);

        app.input_mode = InputMode::ShowError;
        assert_eq!(app.input_mode, InputMode::ShowError);
    }

    // ============================================================================
    // Playback State Tests
    // ============================================================================

    #[test]
    fn test_start_queue() {
        let mut app = App::new(Theme::default());

        // Empty queue
        assert!(app.start_queue().is_none());
        assert!(app.current_queue_index.is_none());
        assert!(!app.is_playing);

        // Add items to queue
        let album = create_test_album("Artist", "Album", "/music/album", 3);
        app.queue.push(QueueEntry::new(QueueItem::new(album)));

        let path = app.start_queue();
        assert!(path.is_some());
        assert_eq!(app.current_queue_index, Some(0));
        assert!(app.is_playing);
    }

    #[test]
    fn test_previous_track_within_album() {
        let mut app = App::new(Theme::default());

        let album = create_test_album("Artist", "Album", "/music/album", 3);
        app.queue.push(QueueEntry::new(QueueItem::new(album)));
        app.current_queue_index = Some(0);
        app.is_playing = true;

        // Move to track 2
        app.queue[0].item.current_track_index = 2;

        // Go back
        let path = app.previous_track();
        assert!(path.is_some());
        assert!(path.unwrap().to_string_lossy().contains("track1.flac"));
    }

    #[test]
    fn test_previous_track_to_previous_album() {
        let mut app = App::new(Theme::default());

        let album1 = create_test_album("Artist", "Album1", "/music/album1", 2);
        let album2 = create_test_album("Artist", "Album2", "/music/album2", 2);
        app.queue.push(QueueEntry::new(QueueItem::new(album1)));
        app.queue.push(QueueEntry::new(QueueItem::new(album2)));
        app.current_queue_index = Some(1);
        app.is_playing = true;

        // At first track of second album
        app.queue[1].item.current_track_index = 0;

        // Go back should go to last track of first album
        let path = app.previous_track();
        assert!(path.is_some());
        assert!(
            path.unwrap()
                .to_string_lossy()
                .contains("album1/track1.flac")
        );
        assert_eq!(app.current_queue_index, Some(0));
    }
}
