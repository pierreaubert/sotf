use sotf_audio::engine::PluginGraphConfig;

/// Outcome of [`apply_room_eq_rack_to_chain`] — useful for log lines or
/// status messages in the UI.
#[derive(Debug, Clone, Copy)]
pub struct RackApplyOutcome {
    /// Number of output channels the EQ plugins were configured for.
    pub num_channels: usize,
    /// Total number of main-room-correction filters (sum across channels).
    pub total_filters: usize,
    /// Total number of broadband pre-correction filters (sum across channels).
    pub total_broadband: usize,
}

/// Outcome of [`apply_room_eq_graph_to_chain`].
///
/// The `config` field is the engine-bound [`PluginGraphConfig`] the caller
/// must pass to `Player::update_plugin_graph(config)`. The UI graph is
/// already mutated in-place on the `PluginGraph` reference passed in.
#[derive(Debug, Clone)]
pub struct GraphApplyOutcome {
    pub config: PluginGraphConfig,
    pub num_nodes: usize,
    pub num_edges: usize,
}

/// Dispatcher entry point: picks the rack or graph apply path based on the
/// optimizer's output shape, so callers don't have to duplicate the
/// `is_rack_compatible()` branch.
///
/// Returns a [`RoomEqApplyOutcome`] tag carrying the path-specific outcome.
/// Caller flushes via `update_plugins` (rack) or `update_plugin_graph` (graph).
#[derive(Debug, Clone)]
pub enum RoomEqApplyOutcome {
    Rack(RackApplyOutcome),
    Graph(GraphApplyOutcome),
}
