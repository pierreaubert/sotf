use super::default::default_input_channels;
use super::default::default_output_channels;
use serde::Deserialize;
use serde_json::Value;
use sotf_audio::PluginConfig;

#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
pub(super) enum Command {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "get_snapshot", alias = "snapshot")]
    GetSnapshot,
    #[serde(rename = "dump_state")]
    DumpState,
    #[serde(rename = "load")]
    Load { path: String },
    #[serde(rename = "play")]
    Play,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "seek")]
    Seek { position: f64 },
    #[serde(rename = "set_volume")]
    SetVolume { volume: f32 },
    #[serde(rename = "list_devices")]
    ListDevices,
    #[serde(rename = "set_device")]
    SetDevice { device: String },
    #[serde(rename = "load_plugins")]
    LoadPlugins {
        plugins: Vec<PluginConfig>,
        #[serde(default = "default_input_channels")]
        input_channels: usize,
        #[serde(default = "default_output_channels")]
        output_channels: usize,
    },
    #[serde(rename = "load_plugin_artifact")]
    LoadPluginArtifact {
        artifact: Value,
        #[serde(default)]
        base_generation: Option<u64>,
    },
    #[serde(rename = "reorder_graph")]
    ReorderGraph {
        order: Vec<usize>,
        #[serde(default)]
        base_generation: Option<u64>,
    },
    #[serde(rename = "set_input_channels")]
    SetInputChannels { channels: usize },
    #[serde(rename = "set_output_channels")]
    SetOutputChannels { channels: usize },
    #[serde(rename = "set_pipeline_channels")]
    SetPipelineChannels {
        #[serde(default)]
        input_channels: Option<usize>,
        #[serde(default)]
        output_channels: Option<usize>,
    },
    #[serde(rename = "get_loudness")]
    GetLoudness,
    #[serde(rename = "get_metering")]
    GetMetering,
    // Plugin management commands
    #[serde(rename = "get_plugins")]
    GetPlugins,
    #[serde(rename = "get_available_plugins")]
    GetAvailablePlugins,
    #[serde(rename = "add_plugin")]
    AddPlugin {
        plugin: PluginConfig,
        #[serde(default)]
        index: Option<usize>,
    },
    #[serde(rename = "remove_plugin")]
    RemovePlugin { index: usize },
    #[serde(rename = "update_plugin")]
    UpdatePlugin {
        index: usize,
        parameters: serde_json::Value,
    },
    #[serde(rename = "reorder_plugins")]
    ReorderPlugins { order: Vec<usize> },
    #[serde(rename = "set_rack_plugin_state")]
    SetRackPluginState {
        index: usize,
        #[serde(default)]
        input_channels: Option<usize>,
        #[serde(default)]
        bypassed: Option<bool>,
        #[serde(default)]
        base_generation: Option<u64>,
    },
    // Driver status (replaces hal_status, kept as alias)
    #[serde(rename = "driver_status", alias = "hal_status")]
    DriverStatus,
    #[serde(rename = "shutdown")]
    Shutdown,
    // Encryption commands
    #[serde(rename = "set_encryption")]
    SetEncryption { enabled: bool },
    #[serde(rename = "encryption_status")]
    EncryptionStatus,
    #[serde(rename = "rotate_encryption_key")]
    RotateEncryptionKey,
    // Driver config commands (replaces hal_config, kept as aliases)
    #[serde(rename = "set_sample_rate")]
    SetSampleRate { rate: u32 },
    #[serde(rename = "set_buffer_frames")]
    SetBufferFrames { frames: u32 },
    #[serde(rename = "get_driver_config", alias = "get_hal_config")]
    GetDriverConfig,
}

impl Command {
    /// Return the wire name (`#[serde(rename = ...)]`) for this command.
    ///
    /// Used to gate which commands a given peer UID may invoke (see
    /// `security::peer_allows_command`). Keep in sync with the `serde`
    /// attributes on each variant.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Command::Status => "status",
            Command::GetSnapshot => "get_snapshot",
            Command::DumpState => "dump_state",
            Command::Load { .. } => "load",
            Command::Play => "play",
            Command::Pause => "pause",
            Command::Stop => "stop",
            Command::Seek { .. } => "seek",
            Command::SetVolume { .. } => "set_volume",
            Command::ListDevices => "list_devices",
            Command::SetDevice { .. } => "set_device",
            Command::LoadPlugins { .. } => "load_plugins",
            Command::LoadPluginArtifact { .. } => "load_plugin_artifact",
            Command::ReorderGraph { .. } => "reorder_graph",
            Command::SetInputChannels { .. } => "set_input_channels",
            Command::SetOutputChannels { .. } => "set_output_channels",
            Command::SetPipelineChannels { .. } => "set_pipeline_channels",
            Command::GetLoudness => "get_loudness",
            Command::GetMetering => "get_metering",
            Command::GetPlugins => "get_plugins",
            Command::GetAvailablePlugins => "get_available_plugins",
            Command::AddPlugin { .. } => "add_plugin",
            Command::RemovePlugin { .. } => "remove_plugin",
            Command::UpdatePlugin { .. } => "update_plugin",
            Command::ReorderPlugins { .. } => "reorder_plugins",
            Command::SetRackPluginState { .. } => "set_rack_plugin_state",
            Command::DriverStatus => "driver_status",
            Command::Shutdown => "shutdown",
            Command::SetEncryption { .. } => "set_encryption",
            Command::EncryptionStatus => "encryption_status",
            Command::RotateEncryptionKey => "rotate_encryption_key",
            Command::SetSampleRate { .. } => "set_sample_rate",
            Command::SetBufferFrames { .. } => "set_buffer_frames",
            Command::GetDriverConfig => "get_driver_config",
        }
    }
}
