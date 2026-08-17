/// State for rendering the Matrix plugin
pub struct MatrixRenderState<'a> {
    pub plugin_instance_id: usize,
    pub input_channels: usize,
    pub output_channels: usize,
    /// Actual plugin viewport width in pixels.
    pub available_width: f32,
    /// Combined responsive and user font scale used by rem-based controls.
    pub layout_scale: f32,
    pub matrix: &'a [f32],
    pub channel_states: &'a [sotf_plugins::ChannelState],
    pub speaker_config: Option<String>,
    pub is_editing: bool,
    pub selected_param: usize,
    /// Currently selected cell (input_idx, output_idx) for editing
    pub selected_cell: Option<(usize, usize)>,
}

#[derive(Clone, Copy)]
pub(super) enum MsdAction {
    Mute,
    Solo,
    Dim,
}
