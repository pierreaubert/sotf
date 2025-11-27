//! Normalized state snapshot for cross-app equivalence testing.
//!
//! This module defines a common state representation that can be extracted
//! from both TUI and GPUI apps to verify behavioral equivalence.

/// Normalized screen identifier (app-agnostic)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenId {
    Library,
    DirectoryManager,
    Queue,
    Plugins,
    Devices,
    Spectrum, // GPUI-only, maps to Library for TUI
    Settings, // GPUI-only, maps to Library for TUI
}

/// Normalized input mode identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputModeId {
    Normal,
    Search,
    AddDirectory,
    EditPlugin,
    SavePlugins,
    LoadPlugins,
    LoadApoFile,
    LoadSofaFile,
    Help,
}

/// Normalized library sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrderId {
    Artist,
    Album,
    Title,
    Year,
    Popularity,
}

/// Normalized channel filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelFilterId {
    All,
    Mono,
    Stereo,
    Multichannel,
    Mixed,
    Specific(u32),
}

/// Normalized library view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewModeId {
    Flat,
    TreeView,
    Grid,
}

/// Normalized plugin type identifier
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginTypeId {
    Gain,
    EQ,
    Upmixer,
    BinauralDecoder,
    LoudnessMonitor,
    SpectrumAnalyzer,
    Compressor,
    Gate,
    Limiter,
    LoudnessCompensation,
    Convolution,
    ChannelMuteSolo,
    Unknown(String),
}

/// Normalized plugin state for comparison
#[derive(Debug, Clone, PartialEq)]
pub struct PluginSnapshot {
    pub plugin_type: PluginTypeId,
    pub enabled: bool,
    pub param_count: usize,
}

/// Normalized state snapshot that can be compared across apps.
///
/// This struct captures the essential state that should be equivalent
/// between TUI and GPUI implementations after the same operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparableState {
    // Navigation
    pub current_screen: ScreenId,
    pub input_mode: InputModeId,

    // Library state
    pub selected_album_index: usize,
    pub library_album_count: usize,
    pub search_query: String,
    pub library_sort_order: SortOrderId,
    pub channel_filter: ChannelFilterId,
    pub library_view_mode: ViewModeId,

    // Queue state
    pub queue_length: usize,
    pub current_queue_index: Option<usize>,
    pub selected_queue_index: usize,

    // Playback state (not real-time, snapshot)
    pub is_playing: bool,
    pub volume: f32,

    // Plugin state
    pub plugin_chain_length: usize,
    pub selected_plugin_index: usize,
    pub editing_plugin_index: Option<usize>,
    pub plugins: Vec<PluginSnapshot>,

    // Directory manager state
    pub directory_count: usize,
    pub selected_directory_index: usize,

    // Device state
    pub device_count: usize,
    pub selected_device_index: usize,
}

impl ComparableState {
    /// Create an empty/default state for testing
    pub fn default_test_state() -> Self {
        Self {
            current_screen: ScreenId::Library,
            input_mode: InputModeId::Normal,
            selected_album_index: 0,
            library_album_count: 0,
            search_query: String::new(),
            library_sort_order: SortOrderId::Artist,
            channel_filter: ChannelFilterId::All,
            library_view_mode: ViewModeId::TreeView,
            queue_length: 0,
            current_queue_index: None,
            selected_queue_index: 0,
            is_playing: false,
            volume: 0.1,
            plugin_chain_length: 0,
            selected_plugin_index: 0,
            editing_plugin_index: None,
            plugins: Vec::new(),
            directory_count: 0,
            selected_directory_index: 0,
            device_count: 0,
            selected_device_index: 0,
        }
    }
}

/// Difference between two states
#[derive(Debug, Clone)]
pub struct StateDiff {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for StateDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: expected {:?}, got {:?}",
            self.field, self.expected, self.actual
        )
    }
}

/// Compare two states and return list of differences
pub fn compare_states(expected: &ComparableState, actual: &ComparableState) -> Vec<StateDiff> {
    let mut diffs = Vec::new();

    macro_rules! check_field {
        ($field:ident) => {
            if expected.$field != actual.$field {
                diffs.push(StateDiff {
                    field: stringify!($field).to_string(),
                    expected: format!("{:?}", expected.$field),
                    actual: format!("{:?}", actual.$field),
                });
            }
        };
    }

    check_field!(current_screen);
    check_field!(input_mode);
    check_field!(selected_album_index);
    check_field!(library_album_count);
    check_field!(search_query);
    check_field!(library_sort_order);
    check_field!(channel_filter);
    check_field!(library_view_mode);
    check_field!(queue_length);
    check_field!(current_queue_index);
    check_field!(selected_queue_index);
    check_field!(is_playing);
    check_field!(plugin_chain_length);
    check_field!(selected_plugin_index);
    check_field!(editing_plugin_index);
    check_field!(directory_count);
    check_field!(selected_directory_index);
    check_field!(device_count);
    check_field!(selected_device_index);

    // Compare volume with tolerance (floating point)
    if (expected.volume - actual.volume).abs() > 0.001 {
        diffs.push(StateDiff {
            field: "volume".to_string(),
            expected: format!("{:.3}", expected.volume),
            actual: format!("{:.3}", actual.volume),
        });
    }

    // Compare plugins
    if expected.plugins.len() != actual.plugins.len() {
        diffs.push(StateDiff {
            field: "plugins.len".to_string(),
            expected: format!("{}", expected.plugins.len()),
            actual: format!("{}", actual.plugins.len()),
        });
    } else {
        for (i, (exp_plugin, act_plugin)) in
            expected.plugins.iter().zip(actual.plugins.iter()).enumerate()
        {
            if exp_plugin != act_plugin {
                diffs.push(StateDiff {
                    field: format!("plugins[{}]", i),
                    expected: format!("{:?}", exp_plugin),
                    actual: format!("{:?}", act_plugin),
                });
            }
        }
    }

    diffs
}
