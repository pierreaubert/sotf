use sotf_audio_player::PluginType;

/// Type of item being dragged from palette
#[derive(Clone)]
pub enum PaletteItemType {
    Plugin(PluginType),
}
