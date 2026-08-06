/// Selects which EQ filter collection an editing operation mutates.
///
/// This is an editor concern only: parameter indices and serialized plugin
/// settings remain unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqEditTarget {
    Global,
    Channel(usize),
}

/// Effect returned by plugin mutations, telling the UI what kind of engine update is needed.
#[derive(Debug, Clone)]
pub enum PluginUpdateEffect {
    /// No update needed (e.g., invalid operation)
    None,
    /// Single parameter change — use `set_plugin_parameter()` for zero-dropout update
    Parameter {
        plugin_index: usize,
        param_index: usize,
    },
    /// Parameter change addressed by graph node ID (works for non-linear graphs).
    ParameterByNodeId {
        node_id: crate::plugin_graph::GraphNodeId,
        param_index: usize,
    },
    /// Structural change (add/remove/reorder/toggle) — full chain rebuild
    Structural,
}
