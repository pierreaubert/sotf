pub use super::super::plugin_param_map::param_index_to_engine_param;
use super::adjust::adjust_plugin_param;
use super::misc::get_param_count;
use super::set::set_eq_param_value_for_target;
use super::set::set_plugin_param_value;
use super::types::EqEditTarget;
use super::types::PluginUpdateEffect;
use crate::plugin_graph::PluginGraph;
use crate::{BiquadFilterType, ChannelConflict, EQFilter, Plugin, PluginSettings, PluginType};
use std::path::Path;

fn eq_filters_mut(
    settings: &mut PluginSettings,
    target: EqEditTarget,
) -> Option<&mut Vec<EQFilter>> {
    let PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = settings
    else {
        return None;
    };
    match target {
        EqEditTarget::Global => Some(filters),
        EqEditTarget::Channel(channel) => channel_filters.as_mut()?.get_mut(channel),
    }
}

fn eq_band_mut(
    settings: &mut PluginSettings,
    target: EqEditTarget,
    band_idx: usize,
) -> Option<&mut EQFilter> {
    eq_filters_mut(settings, target)?.get_mut(band_idx)
}

fn default_eq_param_value(param_idx: usize) -> Option<f64> {
    let defaults = PluginSettings::default_for(&PluginType::EQ).ok()?;
    let PluginSettings::EQ { filters, .. } = defaults else {
        return None;
    };
    let filter = filters.get(param_idx / 4).or_else(|| filters.first())?;
    match param_idx % 4 {
        0 => Some(filter.frequency),
        1 => Some(filter.q),
        2 => Some(filter.gain_db),
        3 => Some(0.0), // The default EQ filter type is Peak, index zero.
        _ => None,
    }
}

fn add_eq_band_to(settings: &mut PluginSettings, target: EqEditTarget) -> Result<(), String> {
    let filters = eq_filters_mut(settings, target)
        .ok_or_else(|| "Selected EQ edit target is unavailable".to_string())?;
    filters.push(EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0));
    Ok(())
}

fn remove_eq_band_from(
    settings: &mut PluginSettings,
    target: EqEditTarget,
    band_idx: usize,
) -> Result<(), String> {
    let filters = eq_filters_mut(settings, target)
        .ok_or_else(|| "Selected EQ edit target is unavailable".to_string())?;
    if band_idx >= filters.len() {
        return Err("Invalid band index".to_string());
    }
    filters.remove(band_idx);
    Ok(())
}

fn copy_eq_global_to_channel_in_settings(
    settings: &mut PluginSettings,
    channel: usize,
) -> Result<(), String> {
    let PluginSettings::EQ {
        filters,
        channel_filters,
        ..
    } = settings
    else {
        return Err("Selected plugin is not an EQ".to_string());
    };
    let target = channel_filters
        .as_mut()
        .and_then(|channels| channels.get_mut(channel))
        .ok_or_else(|| "Selected EQ channel is unavailable".to_string())?;
    *target = filters.clone();
    Ok(())
}

fn copy_eq_channel_to_all_in_settings(
    settings: &mut PluginSettings,
    channel: usize,
) -> Result<(), String> {
    let PluginSettings::EQ {
        channel_filters, ..
    } = settings
    else {
        return Err("Selected plugin is not an EQ".to_string());
    };
    let channels = channel_filters
        .as_mut()
        .ok_or_else(|| "Per-channel EQ is not enabled".to_string())?;
    let source = channels
        .get(channel)
        .cloned()
        .ok_or_else(|| "Selected EQ channel is unavailable".to_string())?;
    for filters in channels {
        *filters = source.clone();
    }
    Ok(())
}

fn cycle_eq_topology_in_settings(
    settings: &mut PluginSettings,
    target: EqEditTarget,
    band_idx: usize,
) -> PluginUpdateEffect {
    use sotf_audio::plugins::eq::EqFilterTopology;

    let Some(filter) = eq_band_mut(settings, target, band_idx) else {
        return PluginUpdateEffect::None;
    };
    filter.topology = match filter.topology {
        EqFilterTopology::Biquad => EqFilterTopology::WarpedBiquad,
        EqFilterTopology::WarpedBiquad => EqFilterTopology::KautzFilter,
        EqFilterTopology::KautzFilter => EqFilterTopology::Biquad,
    };
    PluginUpdateEffect::Structural
}

fn cycle_eq_lambda_in_settings(
    settings: &mut PluginSettings,
    target: EqEditTarget,
    band_idx: usize,
) -> PluginUpdateEffect {
    use sotf_audio::plugins::eq::EqFilterTopology;
    const PRESETS: &[f64] = &[0.4, 0.6, 0.8];

    let Some(filter) = eq_band_mut(settings, target, band_idx) else {
        return PluginUpdateEffect::None;
    };
    if !matches!(filter.topology, EqFilterTopology::WarpedBiquad) {
        return PluginUpdateEffect::None;
    }
    filter.lambda = match filter.lambda {
        None => Some(PRESETS[0]),
        Some(value) => PRESETS
            .iter()
            .copied()
            .find(|preset| *preset > value + 1e-9),
    };
    PluginUpdateEffect::Structural
}

fn add_eq_kautz_section_in_settings(
    settings: &mut PluginSettings,
    target: EqEditTarget,
    band_idx: usize,
    pole_freq: f64,
    q: f64,
    gain: f64,
) -> PluginUpdateEffect {
    use sotf_audio::plugins::eq::{EqFilterTopology, KautzSectionConfig};
    let Some(filter) = eq_band_mut(settings, target, band_idx) else {
        return PluginUpdateEffect::None;
    };
    if !matches!(filter.topology, EqFilterTopology::KautzFilter) {
        return PluginUpdateEffect::None;
    }
    filter
        .kautz_sections
        .push(KautzSectionConfig { pole_freq, q, gain });
    PluginUpdateEffect::Structural
}

fn pop_eq_kautz_section_in_settings(
    settings: &mut PluginSettings,
    target: EqEditTarget,
    band_idx: usize,
) -> PluginUpdateEffect {
    use sotf_audio::plugins::eq::EqFilterTopology;
    let Some(filter) = eq_band_mut(settings, target, band_idx) else {
        return PluginUpdateEffect::None;
    };
    if matches!(filter.topology, EqFilterTopology::KautzFilter)
        && filter.kautz_sections.pop().is_some()
    {
        PluginUpdateEffect::Structural
    } else {
        PluginUpdateEffect::None
    }
}

/// Plugin controller owning shared state for plugin editing.
///
/// Uses `PluginGraph` as the sole source of truth for plugin topology.
#[derive(Debug, Clone)]
pub struct PluginController {
    pub graph: PluginGraph,
    pub editing_plugin_index: Option<usize>,
    pub plugin_param_selection: usize,
    pub selected_plugin_index: usize,
    pub selected_eq_band: usize,
    pub selected_eq_channel: usize,
    pub available_presets: Vec<String>,
    pub selected_preset_index: usize,
    pub last_loaded_preset: Option<String>,
}

impl Default for PluginController {
    fn default() -> Self {
        Self {
            graph: PluginGraph::with_default_rack(),
            editing_plugin_index: None,
            plugin_param_selection: 0,
            selected_plugin_index: 0,
            selected_eq_band: 0,
            selected_eq_channel: 0,
            available_presets: Vec::new(),
            selected_preset_index: 0,
            last_loaded_preset: None,
        }
    }
}

impl PluginController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the plugin graph is a simple linear chain (rack view compatible).
    pub fn is_linear(&self) -> bool {
        self.graph.is_linear()
    }

    // ========================================================================
    // Chain management
    // ========================================================================

    /// Add a plugin to the chain. Returns `Structural` effect.
    pub fn add_plugin(&mut self, plugin_type: &PluginType) -> PluginUpdateEffect {
        let insert_idx = self.graph.user_plugin_insert_index();
        match self.graph.insert_plugin(insert_idx, plugin_type) {
            Ok(_) => {
                self.selected_plugin_index = insert_idx;
                PluginUpdateEffect::Structural
            }
            Err(err) => {
                log::warn!(
                    "[PluginController] Failed to insert {:?} at {}: {}",
                    plugin_type,
                    insert_idx,
                    err
                );
                PluginUpdateEffect::None
            }
        }
    }

    /// Add a plugin whose complete settings are already known. This preserves
    /// external descriptors and returns insertion failures to the UI.
    pub fn add_plugin_settings(
        &mut self,
        settings: PluginSettings,
    ) -> Result<PluginUpdateEffect, String> {
        let insert_idx = self.graph.user_plugin_insert_index();
        self.graph.insert_plugin_settings(insert_idx, settings)?;
        self.selected_plugin_index = insert_idx;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Toggle a plugin's enabled state. A rejected channel contract leaves the
    /// graph unchanged and is returned to the UI.
    pub fn toggle_plugin(&mut self, index: usize) -> Result<PluginUpdateEffect, String> {
        self.graph.toggle_plugin_by_index(index)?;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Move a plugin up in the chain. Returns `Structural` if moved, `None` otherwise.
    pub fn move_plugin_up(&mut self, index: usize) -> Result<PluginUpdateEffect, String> {
        if self.graph.can_move_up_by_index(index) {
            self.graph.move_plugin(index, index - 1)?;
            self.selected_plugin_index = index - 1;
            Ok(PluginUpdateEffect::Structural)
        } else {
            Ok(PluginUpdateEffect::None)
        }
    }

    /// Move a plugin down in the chain. Returns `Structural` if moved, `None` otherwise.
    pub fn move_plugin_down(&mut self, index: usize) -> Result<PluginUpdateEffect, String> {
        if self.graph.can_move_down_by_index(index) {
            self.graph.move_plugin(index, index + 1)?;
            self.selected_plugin_index = index + 1;
            Ok(PluginUpdateEffect::Structural)
        } else {
            Ok(PluginUpdateEffect::None)
        }
    }

    /// Remove a plugin from the chain. Returns `Structural` if removed, `None` otherwise.
    pub fn remove_plugin(&mut self, index: usize) -> PluginUpdateEffect {
        if self.graph.remove_plugin_by_index(index).is_ok() {
            if self.selected_plugin_index >= self.graph.len() && self.selected_plugin_index > 0 {
                self.selected_plugin_index = self.graph.len() - 1;
            }
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Select the next plugin in the chain.
    pub fn select_next_plugin(&mut self) {
        if !self.graph.is_empty() {
            self.selected_plugin_index = (self.selected_plugin_index + 1) % self.graph.len();
        }
    }

    /// Select the previous plugin in the chain.
    pub fn select_previous_plugin(&mut self) {
        if !self.graph.is_empty() {
            if self.selected_plugin_index == 0 {
                self.selected_plugin_index = self.graph.len() - 1;
            } else {
                self.selected_plugin_index -= 1;
            }
        }
    }

    // ========================================================================
    // Plugin access
    // ========================================================================

    /// Get the currently editing plugin (immutable).
    pub fn get_editing_plugin(&self) -> Option<&Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.graph.get_plugin(idx))
    }

    /// Get the currently editing plugin (mutable).
    pub fn get_editing_plugin_mut(&mut self) -> Option<&mut Plugin> {
        self.editing_plugin_index
            .and_then(|idx| self.graph.get_plugin_mut(idx))
    }

    /// Whether the chain has an enabled spectrum analyzer.
    pub fn has_enabled_spectrum_analyzer(&self) -> bool {
        self.graph.has_enabled_spectrum_analyzer()
    }

    // ========================================================================
    // Parameter navigation
    // ========================================================================

    /// Select the next parameter.
    pub fn select_next_param(&mut self) {
        if let Some(plugin) = self.get_editing_plugin() {
            let param_count = get_param_count(&plugin.settings);
            if param_count > 0 {
                self.plugin_param_selection = (self.plugin_param_selection + 1) % param_count;
            }
        }
    }

    /// Select the previous parameter.
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

    // ========================================================================
    // Parameter editing
    // ========================================================================

    /// Adjust the currently selected parameter by the given delta.
    /// Returns `(adjusted, effect)`.
    pub fn adjust_selected_param(&mut self, delta: f64) -> (bool, PluginUpdateEffect) {
        let param_idx = self.plugin_param_selection;
        let mut channel_count_changed = false;

        let result = if let Some(plugin) = self.get_editing_plugin_mut() {
            adjust_plugin_param(
                &mut plugin.settings,
                param_idx,
                delta,
                &mut channel_count_changed,
            )
        } else {
            false
        };

        if result && channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        if result {
            let effect = self.determine_update_effect(
                self.editing_plugin_index,
                param_idx,
                channel_count_changed,
            );
            (true, effect)
        } else {
            (false, PluginUpdateEffect::None)
        }
    }

    /// Set a specific parameter value for a plugin.
    pub fn set_plugin_param(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
        value: f64,
    ) -> PluginUpdateEffect {
        let mut channel_count_changed = false;
        let mut update_needed = false;

        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            update_needed = set_plugin_param_value(
                &mut plugin.settings,
                param_idx,
                value,
                &mut channel_count_changed,
            );
        }

        if channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        if update_needed {
            self.determine_update_effect(Some(plugin_idx), param_idx, channel_count_changed)
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Set a dynamic EQ-band parameter on the explicitly selected filter set.
    pub fn set_eq_param(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        param_idx: usize,
        value: f64,
    ) -> PluginUpdateEffect {
        let updated = self.graph.get_plugin_mut(plugin_idx).is_some_and(|plugin| {
            set_eq_param_value_for_target(&mut plugin.settings, target, param_idx, value)
        });
        if updated {
            self.determine_update_effect(Some(plugin_idx), param_idx, false)
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Node-ID variant of [`Self::set_eq_param`] for graph editing.
    pub fn set_eq_param_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        param_idx: usize,
        value: f64,
    ) -> PluginUpdateEffect {
        let updated = self.graph.nodes.get_mut(&node_id).is_some_and(|node| {
            set_eq_param_value_for_target(&mut node.plugin.settings, target, param_idx, value)
        });
        if updated {
            self.determine_update_effect_by_node_id(node_id, param_idx, false)
        } else {
            PluginUpdateEffect::None
        }
    }

    pub fn reset_eq_param(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        param_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(value) = default_eq_param_value(param_idx) else {
            return PluginUpdateEffect::None;
        };
        self.set_eq_param(plugin_idx, target, param_idx, value)
    }

    pub fn reset_eq_param_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        param_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(value) = default_eq_param_value(param_idx) else {
            return PluginUpdateEffect::None;
        };
        self.set_eq_param_by_node_id(node_id, target, param_idx, value)
    }

    /// Cycle the topology of an EQ filter band through Biquad → Warped → Kautz.
    ///
    /// Returns `Structural` so the engine rebuilds the EQ instance with the new
    /// runtime topology (warped biquads need their own coefficient bank and
    /// Kautz needs a parallel modal filter — the existing parameter-update
    /// path can't reconfigure that in place).
    pub fn cycle_eq_filter_topology(
        &mut self,
        plugin_idx: usize,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        use sotf_audio::plugins::eq::EqFilterTopology;

        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        let sotf_audio::plugins::PluginSettings::EQ {
            filters,
            channel_filters,
            ..
        } = &mut plugin.settings
        else {
            return PluginUpdateEffect::None;
        };

        let cycle = |t: EqFilterTopology| -> EqFilterTopology {
            match t {
                EqFilterTopology::Biquad => EqFilterTopology::WarpedBiquad,
                EqFilterTopology::WarpedBiquad => EqFilterTopology::KautzFilter,
                EqFilterTopology::KautzFilter => EqFilterTopology::Biquad,
            }
        };

        // Compute the next topology ONCE from the global band (or the first
        // per-channel copy if the global slot is missing) so every replica
        // converges on the same value — otherwise repeated calls would
        // diverge per-channel state.
        let Some(current) = filters.get(band_idx).map(|f| f.topology).or_else(|| {
            channel_filters
                .as_ref()
                .and_then(|cf| cf.first())
                .and_then(|ch| ch.get(band_idx).map(|f| f.topology))
        }) else {
            return PluginUpdateEffect::None;
        };
        let next_topology = cycle(current);

        let mut mutated = false;
        if let Some(f) = filters.get_mut(band_idx) {
            f.topology = next_topology;
            mutated = true;
        }
        if let Some(channel_filters) = channel_filters.as_mut() {
            for per_channel in channel_filters.iter_mut() {
                if let Some(f) = per_channel.get_mut(band_idx) {
                    f.topology = next_topology;
                    mutated = true;
                }
            }
        }

        if mutated {
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Cycle topology on only the filter set being edited.
    pub fn cycle_eq_filter_topology_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        cycle_eq_topology_in_settings(&mut plugin.settings, target, band_idx)
    }

    pub fn cycle_eq_filter_topology_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(node) = self.graph.nodes.get_mut(&node_id) else {
            return PluginUpdateEffect::None;
        };
        cycle_eq_topology_in_settings(&mut node.plugin.settings, target, band_idx)
    }

    /// Cycle the lambda (warping coefficient) for a warped-biquad EQ band.
    ///
    /// Cycles through `None` (auto-Bark for the active sample rate) → 0.4 →
    /// 0.6 → 0.8 → back to `None`. No-op when the band isn't a warped biquad.
    pub fn cycle_eq_filter_lambda(
        &mut self,
        plugin_idx: usize,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        use sotf_audio::plugins::eq::EqFilterTopology;

        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        let sotf_audio::plugins::PluginSettings::EQ {
            filters,
            channel_filters,
            ..
        } = &mut plugin.settings
        else {
            return PluginUpdateEffect::None;
        };

        // Cycle: None → 0.4 → 0.6 → 0.8 → None. Built on top of "find the
        // smallest preset strictly above current" so a hand-edited JSON
        // value (e.g. 0.5 or 0.55) snaps onto the next preset rather than
        // skipping a stop. Values past the last preset wrap back to None
        // (auto-Bark).
        const PRESETS: &[f64] = &[0.4, 0.6, 0.8];
        let next = |current: Option<f64>| -> Option<f64> {
            match current {
                None => Some(PRESETS[0]),
                Some(v) => PRESETS.iter().copied().find(|&p| p > v + 1e-9),
            }
        };

        // Same rule as topology cycling: compute the next lambda once.
        let current_lambda = filters
            .get(band_idx)
            .filter(|f| matches!(f.topology, EqFilterTopology::WarpedBiquad))
            .map(|f| f.lambda)
            .or_else(|| {
                channel_filters
                    .as_ref()
                    .and_then(|cf| cf.first())
                    .and_then(|ch| ch.get(band_idx))
                    .filter(|f| matches!(f.topology, EqFilterTopology::WarpedBiquad))
                    .map(|f| f.lambda)
            });
        let Some(current_lambda) = current_lambda else {
            return PluginUpdateEffect::None;
        };
        let next_lambda = next(current_lambda);

        let mut mutated = false;
        if let Some(f) = filters.get_mut(band_idx)
            && matches!(f.topology, EqFilterTopology::WarpedBiquad)
        {
            f.lambda = next_lambda;
            mutated = true;
        }
        if let Some(channel_filters) = channel_filters.as_mut() {
            for per_channel in channel_filters.iter_mut() {
                if let Some(f) = per_channel.get_mut(band_idx)
                    && matches!(f.topology, EqFilterTopology::WarpedBiquad)
                {
                    f.lambda = next_lambda;
                    mutated = true;
                }
            }
        }

        if mutated {
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    pub fn cycle_eq_filter_lambda_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        cycle_eq_lambda_in_settings(&mut plugin.settings, target, band_idx)
    }

    pub fn cycle_eq_filter_lambda_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(node) = self.graph.nodes.get_mut(&node_id) else {
            return PluginUpdateEffect::None;
        };
        cycle_eq_lambda_in_settings(&mut node.plugin.settings, target, band_idx)
    }

    /// Append a Kautz pole section to an EQ band. Only takes effect when the
    /// band's topology is `KautzFilter`; otherwise returns `None`.
    pub fn add_eq_kautz_section(
        &mut self,
        plugin_idx: usize,
        band_idx: usize,
        pole_freq: f64,
        q: f64,
        gain: f64,
    ) -> PluginUpdateEffect {
        use sotf_audio::plugins::eq::{EqFilterTopology, KautzSectionConfig};

        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        let sotf_audio::plugins::PluginSettings::EQ {
            filters,
            channel_filters,
            ..
        } = &mut plugin.settings
        else {
            return PluginUpdateEffect::None;
        };

        let section = KautzSectionConfig { pole_freq, q, gain };

        let mut mutated = false;
        if let Some(f) = filters.get_mut(band_idx)
            && matches!(f.topology, EqFilterTopology::KautzFilter)
        {
            f.kautz_sections.push(section.clone());
            mutated = true;
        }
        if let Some(channel_filters) = channel_filters.as_mut() {
            for per_channel in channel_filters.iter_mut() {
                if let Some(f) = per_channel.get_mut(band_idx)
                    && matches!(f.topology, EqFilterTopology::KautzFilter)
                {
                    f.kautz_sections.push(section.clone());
                    mutated = true;
                }
            }
        }

        if mutated {
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_eq_kautz_section_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        band_idx: usize,
        pole_freq: f64,
        q: f64,
        gain: f64,
    ) -> PluginUpdateEffect {
        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        add_eq_kautz_section_in_settings(&mut plugin.settings, target, band_idx, pole_freq, q, gain)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_eq_kautz_section_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        band_idx: usize,
        pole_freq: f64,
        q: f64,
        gain: f64,
    ) -> PluginUpdateEffect {
        let Some(node) = self.graph.nodes.get_mut(&node_id) else {
            return PluginUpdateEffect::None;
        };
        add_eq_kautz_section_in_settings(
            &mut node.plugin.settings,
            target,
            band_idx,
            pole_freq,
            q,
            gain,
        )
    }

    /// Remove the last Kautz pole section from an EQ band. No-op when the
    /// band's topology isn't `KautzFilter` or the section list is empty.
    pub fn pop_eq_kautz_section(
        &mut self,
        plugin_idx: usize,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        use sotf_audio::plugins::eq::EqFilterTopology;

        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        let sotf_audio::plugins::PluginSettings::EQ {
            filters,
            channel_filters,
            ..
        } = &mut plugin.settings
        else {
            return PluginUpdateEffect::None;
        };

        let mut mutated = false;
        if let Some(f) = filters.get_mut(band_idx)
            && matches!(f.topology, EqFilterTopology::KautzFilter)
            && f.kautz_sections.pop().is_some()
        {
            mutated = true;
        }
        if let Some(channel_filters) = channel_filters.as_mut() {
            for per_channel in channel_filters.iter_mut() {
                if let Some(f) = per_channel.get_mut(band_idx)
                    && matches!(f.topology, EqFilterTopology::KautzFilter)
                    && f.kautz_sections.pop().is_some()
                {
                    mutated = true;
                }
            }
        }

        if mutated {
            PluginUpdateEffect::Structural
        } else {
            PluginUpdateEffect::None
        }
    }

    pub fn pop_eq_kautz_section_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) else {
            return PluginUpdateEffect::None;
        };
        pop_eq_kautz_section_in_settings(&mut plugin.settings, target, band_idx)
    }

    pub fn pop_eq_kautz_section_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        band_idx: usize,
    ) -> PluginUpdateEffect {
        let Some(node) = self.graph.nodes.get_mut(&node_id) else {
            return PluginUpdateEffect::None;
        };
        pop_eq_kautz_section_in_settings(&mut node.plugin.settings, target, band_idx)
    }

    /// Set a parameter value for a plugin identified by its graph node ID.
    ///
    /// Works for both linear and non-linear graph topologies, unlike
    /// `set_plugin_param` which requires a linear index.
    pub fn set_plugin_param_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        param_idx: usize,
        value: f64,
    ) -> PluginUpdateEffect {
        let mut channel_count_changed = false;
        let mut update_needed = false;

        if let Some(node) = self.graph.nodes.get_mut(&node_id) {
            update_needed = set_plugin_param_value(
                &mut node.plugin.settings,
                param_idx,
                value,
                &mut channel_count_changed,
            );
        }

        if channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        if update_needed {
            self.determine_update_effect_by_node_id(node_id, param_idx, channel_count_changed)
        } else {
            PluginUpdateEffect::None
        }
    }

    /// Set a string parameter value for a plugin by node ID.
    pub fn set_plugin_param_string_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        param_idx: usize,
        value: String,
    ) -> Result<PluginUpdateEffect, String> {
        let mut update_needed = false;

        if let Some(node) = self.graph.nodes.get_mut(&node_id) {
            match &mut node.plugin.settings {
                PluginSettings::ABCompare {
                    path_a_config,
                    path_b_config,
                    ..
                } => match param_idx {
                    9 => {
                        *path_a_config = value;
                        update_needed = true;
                    }
                    10 => {
                        *path_b_config = value;
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Convolution { ir_file, .. } if param_idx == 0 => {
                    if !value.is_empty() {
                        crate::security::validate_plugin_ir_file_path(Path::new(&value))
                            .map_err(|e| e.to_string())?;
                    }
                    *ir_file = value;
                    update_needed = true;
                }
                PluginSettings::XTC { room_ir_file, .. } if param_idx == 16 => {
                    if !value.is_empty() {
                        crate::security::validate_plugin_ir_file_path(Path::new(&value))
                            .map_err(|e| e.to_string())?;
                    }
                    *room_ir_file = if value.is_empty() { None } else { Some(value) };
                    update_needed = true;
                }
                PluginSettings::BinauralDecoder { sofa_file, .. } if param_idx == 0 => {
                    if !value.is_empty() {
                        crate::security::validate_plugin_sofa_file_path(Path::new(&value))
                            .map_err(|e| e.to_string())?;
                    }
                    *sofa_file = value;
                    update_needed = true;
                }
                _ => {}
            }
        }

        if update_needed {
            Ok(PluginUpdateEffect::Structural)
        } else {
            Ok(PluginUpdateEffect::None)
        }
    }

    /// Set a string parameter value for a plugin (e.g., file paths).
    ///
    /// File path parameters (IR file, SOFA file) are validated against
    /// path traversal before being accepted. AB Compare config strings
    /// are JSON and not validated as paths.
    pub fn set_plugin_param_string(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
        value: String,
    ) -> Result<PluginUpdateEffect, String> {
        let mut update_needed = false;

        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            match &mut plugin.settings {
                // AB Compare: path_a_config / path_b_config are JSON config
                // strings, not file paths — no path validation needed.
                PluginSettings::ABCompare {
                    path_a_config,
                    path_b_config,
                    ..
                } => match param_idx {
                    9 => {
                        *path_a_config = value;
                        update_needed = true;
                    }
                    10 => {
                        *path_b_config = value;
                        update_needed = true;
                    }
                    _ => {}
                },
                PluginSettings::Convolution { ir_file, .. } if param_idx == 0 => {
                    if !value.is_empty() {
                        crate::security::validate_plugin_ir_file_path(Path::new(&value))
                            .map_err(|e| e.to_string())?;
                    }
                    *ir_file = value;
                    update_needed = true;
                }
                PluginSettings::XTC { room_ir_file, .. } if param_idx == 16 => {
                    if !value.is_empty() {
                        crate::security::validate_plugin_ir_file_path(Path::new(&value))
                            .map_err(|e| e.to_string())?;
                    }
                    *room_ir_file = if value.is_empty() { None } else { Some(value) };
                    update_needed = true;
                }
                PluginSettings::BinauralDecoder { sofa_file, .. } if param_idx == 0 => {
                    if !value.is_empty() {
                        crate::security::validate_plugin_sofa_file_path(Path::new(&value))
                            .map_err(|e| e.to_string())?;
                    }
                    *sofa_file = value;
                    update_needed = true;
                }
                _ => {}
            }
        }

        if update_needed {
            Ok(PluginUpdateEffect::Structural)
        } else {
            Ok(PluginUpdateEffect::None)
        }
    }

    /// Set spectrum analyzer tilt correction mode.
    pub fn set_spectrum_tilt_correction(
        &mut self,
        plugin_idx: usize,
        tilt: sotf_plugins::SpectralTiltCorrection,
    ) -> PluginUpdateEffect {
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            if let PluginSettings::SpectrumAnalyzer {
                tilt_correction, ..
            } = &mut plugin.settings
            {
                *tilt_correction = tilt;
                return PluginUpdateEffect::Structural;
            }
        }
        PluginUpdateEffect::None
    }

    /// Set spectrum analyzer tilt reference frequency.
    pub fn set_spectrum_tilt_reference(
        &mut self,
        plugin_idx: usize,
        reference: sotf_plugins::TiltReferenceFreq,
    ) -> PluginUpdateEffect {
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            if let PluginSettings::SpectrumAnalyzer { tilt_reference, .. } = &mut plugin.settings {
                *tilt_reference = reference;
                return PluginUpdateEffect::Structural;
            }
        }
        PluginUpdateEffect::None
    }

    /// Reset a specific parameter to its default value, addressed by node ID.
    pub fn reset_plugin_param_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        param_idx: usize,
    ) -> PluginUpdateEffect {
        let plugin_type = if let Some(node) = self.graph.nodes.get(&node_id) {
            node.plugin.plugin_type()
        } else {
            return PluginUpdateEffect::None;
        };

        let Ok(default_settings) = PluginSettings::default_for(&plugin_type) else {
            return PluginUpdateEffect::None;
        };
        let default_value = match default_settings.param_value(param_idx) {
            Some(v) => v,
            None => return PluginUpdateEffect::None,
        };

        let mut channel_count_changed = false;

        if let Some(node) = self.graph.nodes.get_mut(&node_id) {
            node.plugin
                .settings
                .set_param_value(param_idx, default_value);

            match &mut node.plugin.settings {
                PluginSettings::Upmixer { .. } if param_idx == 0 => {
                    channel_count_changed = true;
                }
                PluginSettings::MultibandCompressor {
                    num_bands, bands, ..
                } if param_idx == 0 => {
                    bands.resize_with(*num_bands, Default::default);
                    for (i, band) in bands.iter_mut().enumerate() {
                        band.active = match *num_bands {
                            4 | 5 => i < 3,
                            _ => true,
                        };
                    }
                    channel_count_changed = true;
                }
                PluginSettings::MultibandExpander {
                    num_bands, bands, ..
                } if param_idx == 0 => {
                    bands.resize_with(*num_bands, Default::default);
                    for (i, band) in bands.iter_mut().enumerate() {
                        band.active = match *num_bands {
                            4 | 5 => i < 3,
                            _ => true,
                        };
                    }
                    channel_count_changed = true;
                }
                _ => {}
            }
        }

        if channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        self.determine_update_effect_by_node_id(node_id, param_idx, channel_count_changed)
    }

    /// Reset a specific parameter to its default value.
    pub fn reset_plugin_param(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
    ) -> PluginUpdateEffect {
        let plugin_type = if let Some(plugin) = self.graph.get_plugin(plugin_idx) {
            plugin.plugin_type()
        } else {
            return PluginUpdateEffect::None;
        };

        let Ok(default_settings) = PluginSettings::default_for(&plugin_type) else {
            return PluginUpdateEffect::None;
        };
        let default_value = match default_settings.param_value(param_idx) {
            Some(v) => v,
            None => return PluginUpdateEffect::None,
        };

        let mut channel_count_changed = false;

        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            plugin.settings.set_param_value(param_idx, default_value);

            match &mut plugin.settings {
                PluginSettings::Upmixer { .. } if param_idx == 0 => {
                    channel_count_changed = true;
                }
                PluginSettings::MultibandCompressor {
                    num_bands, bands, ..
                } if param_idx == 0 => {
                    bands.resize_with(*num_bands, Default::default);
                    // Default active states: 4 bands => band 4 passive, 5 bands => bands 4,5 passive
                    for (i, band) in bands.iter_mut().enumerate() {
                        band.active = match *num_bands {
                            4 => i < 3,
                            5 => i < 3,
                            _ => true,
                        };
                    }
                    channel_count_changed = true;
                }
                PluginSettings::MultibandExpander {
                    num_bands, bands, ..
                } if param_idx == 0 => {
                    bands.resize_with(*num_bands, Default::default);
                    for (i, band) in bands.iter_mut().enumerate() {
                        band.active = match *num_bands {
                            4 => i < 3,
                            5 => i < 3,
                            _ => true,
                        };
                    }
                    channel_count_changed = true;
                }
                _ => {}
            }
        }

        if channel_count_changed {
            self.graph.update_channel_dependent_plugins();
        }

        self.determine_update_effect(Some(plugin_idx), param_idx, channel_count_changed)
    }

    // ========================================================================
    // EQ operations
    // ========================================================================

    /// Load EQ filters from an APO file path. Works for both EQ and LinearPhaseEq.
    pub fn load_apo_filters(&mut self, path: &Path) -> Result<PluginUpdateEffect, String> {
        crate::security::validate_plugin_apo_file_path(path).map_err(|e| e.to_string())?;
        let new_filters = EQFilter::from_apo_file(path)?;
        let plugin = self
            .get_editing_plugin_mut()
            .ok_or_else(|| "No plugin being edited".to_string())?;
        let filters = plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        *filters = new_filters;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Update SOFA file path for the currently editing binaural decoder plugin.
    pub fn load_sofa_path(&mut self, sofa_path: String) -> Result<PluginUpdateEffect, String> {
        if let Some(plugin) = self.get_editing_plugin() {
            if !matches!(plugin.settings, PluginSettings::BinauralDecoder { .. }) {
                return Err("Selected plugin is not a Binaural Decoder".to_string());
            }
        } else {
            return Err("No plugin being edited".to_string());
        }

        if !sofa_path.is_empty() {
            crate::security::validate_plugin_sofa_file_path(Path::new(&sofa_path))
                .map_err(|e| e.to_string())?;
        }

        if let Some(plugin) = self.get_editing_plugin_mut() {
            if let PluginSettings::BinauralDecoder {
                ref mut sofa_file, ..
            } = plugin.settings
            {
                *sofa_file = sofa_path;
                Ok(PluginUpdateEffect::Structural)
            } else {
                Err("Selected plugin is not a Binaural Decoder".to_string())
            }
        } else {
            Err("No plugin being edited".to_string())
        }
    }

    /// Add a new EQ band to the currently editing EQ or LinearPhaseEq plugin.
    pub fn add_eq_band_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin_mut(plugin_idx)
            .ok_or_else(|| "Plugin not found".to_string())?;
        add_eq_band_to(&mut plugin.settings, target)?;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn add_eq_band_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        add_eq_band_to(&mut node.plugin.settings, target)?;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn add_eq_band(&mut self) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .get_editing_plugin_mut()
            .ok_or_else(|| "No plugin being edited".to_string())?;
        let filters = plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        filters.push(EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0));
        Ok(PluginUpdateEffect::Structural)
    }

    /// Add a new EQ band to a plugin addressed by graph node ID.
    /// Required when editing a node inside a non-linear graph (graph view),
    /// where `editing_plugin_index` is not set.
    pub fn add_eq_band_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        let filters = node
            .plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        filters.push(EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 0.0));
        Ok(PluginUpdateEffect::Structural)
    }

    /// Remove an EQ band from the currently editing EQ or LinearPhaseEq plugin.
    pub fn remove_eq_band_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin_mut(plugin_idx)
            .ok_or_else(|| "Plugin not found".to_string())?;
        remove_eq_band_from(&mut plugin.settings, target, band_idx)?;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn remove_eq_band_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        remove_eq_band_from(&mut node.plugin.settings, target, band_idx)?;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn remove_eq_band(&mut self, band_idx: usize) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .get_editing_plugin_mut()
            .ok_or_else(|| "No plugin being edited".to_string())?;
        let filters = plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        if band_idx >= filters.len() {
            return Err("Invalid band index".to_string());
        }
        filters.remove(band_idx);
        Ok(PluginUpdateEffect::Structural)
    }

    /// Remove an EQ band from a plugin addressed by graph node ID.
    pub fn remove_eq_band_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        let filters = node
            .plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        if band_idx >= filters.len() {
            return Err("Invalid band index".to_string());
        }
        filters.remove(band_idx);
        Ok(PluginUpdateEffect::Structural)
    }

    /// Toggle mute state for an EQ or LinearPhaseEq band.
    pub fn toggle_eq_band_mute_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin_mut(plugin_idx)
            .ok_or_else(|| "Plugin not found".to_string())?;
        let band = eq_band_mut(&mut plugin.settings, target, band_idx)
            .ok_or_else(|| "Invalid band index or EQ edit target".to_string())?;
        band.muted = !band.muted;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn toggle_eq_band_mute_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        let band = eq_band_mut(&mut node.plugin.settings, target, band_idx)
            .ok_or_else(|| "Invalid band index or EQ edit target".to_string())?;
        band.muted = !band.muted;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn toggle_eq_band_mute(&mut self, band_idx: usize) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .get_editing_plugin_mut()
            .ok_or_else(|| "No plugin being edited".to_string())?;
        let filters = plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        if band_idx >= filters.len() {
            return Err("Invalid band index".to_string());
        }
        filters[band_idx].muted = !filters[band_idx].muted;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Toggle mute for an EQ band on a plugin addressed by graph node ID.
    pub fn toggle_eq_band_mute_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        let filters = node
            .plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        if band_idx >= filters.len() {
            return Err("Invalid band index".to_string());
        }
        filters[band_idx].muted = !filters[band_idx].muted;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Toggle solo state for an EQ or LinearPhaseEq band.
    pub fn toggle_eq_band_solo_for_target(
        &mut self,
        plugin_idx: usize,
        target: EqEditTarget,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin_mut(plugin_idx)
            .ok_or_else(|| "Plugin not found".to_string())?;
        let band = eq_band_mut(&mut plugin.settings, target, band_idx)
            .ok_or_else(|| "Invalid band index or EQ edit target".to_string())?;
        band.solo = !band.solo;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn toggle_eq_band_solo_for_target_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        target: EqEditTarget,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        let band = eq_band_mut(&mut node.plugin.settings, target, band_idx)
            .ok_or_else(|| "Invalid band index or EQ edit target".to_string())?;
        band.solo = !band.solo;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn toggle_eq_band_solo(&mut self, band_idx: usize) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .get_editing_plugin_mut()
            .ok_or_else(|| "No plugin being edited".to_string())?;
        let filters = plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        if band_idx >= filters.len() {
            return Err("Invalid band index".to_string());
        }
        filters[band_idx].solo = !filters[band_idx].solo;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Toggle solo for an EQ band on a plugin addressed by graph node ID.
    pub fn toggle_eq_band_solo_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        band_idx: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        let filters = node
            .plugin
            .settings
            .eq_global_filters_mut()
            .ok_or_else(|| "Selected plugin is not an EQ".to_string())?;
        if band_idx >= filters.len() {
            return Err("Invalid band index".to_string());
        }
        filters[band_idx].solo = !filters[band_idx].solo;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Set the EQ plugin to per-channel mode or global mode.
    pub fn set_eq_per_channel_mode(
        &mut self,
        plugin_idx: usize,
        per_channel: bool,
    ) -> PluginUpdateEffect {
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            if let PluginSettings::EQ {
                channels,
                filters,
                channel_filters,
                per_channel_mode,
                ..
            } = &mut plugin.settings
            {
                if per_channel && channel_filters.is_none() {
                    let num_channels = *channels;
                    let mut ch_filters = Vec::with_capacity(num_channels);
                    for _ in 0..num_channels {
                        ch_filters.push(filters.clone());
                    }
                    *channel_filters = Some(ch_filters);
                }

                *per_channel_mode = per_channel;
                return PluginUpdateEffect::Structural;
            }
        }
        PluginUpdateEffect::None
    }

    /// Replace one channel's bands with the global EQ bands.
    pub fn copy_eq_global_to_channel(
        &mut self,
        plugin_idx: usize,
        channel: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin_mut(plugin_idx)
            .ok_or_else(|| "Plugin not found".to_string())?;
        copy_eq_global_to_channel_in_settings(&mut plugin.settings, channel)?;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn copy_eq_global_to_channel_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        channel: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        copy_eq_global_to_channel_in_settings(&mut node.plugin.settings, channel)?;
        Ok(PluginUpdateEffect::Structural)
    }

    /// Replace every channel's bands with the selected channel's bands.
    pub fn copy_eq_channel_to_all(
        &mut self,
        plugin_idx: usize,
        channel: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin_mut(plugin_idx)
            .ok_or_else(|| "Plugin not found".to_string())?;
        copy_eq_channel_to_all_in_settings(&mut plugin.settings, channel)?;
        Ok(PluginUpdateEffect::Structural)
    }

    pub fn copy_eq_channel_to_all_by_node_id(
        &mut self,
        node_id: crate::plugin_graph::GraphNodeId,
        channel: usize,
    ) -> Result<PluginUpdateEffect, String> {
        let node = self
            .graph
            .nodes
            .get_mut(&node_id)
            .ok_or_else(|| "Plugin node not found".to_string())?;
        copy_eq_channel_to_all_in_settings(&mut node.plugin.settings, channel)?;
        Ok(PluginUpdateEffect::Structural)
    }

    // ========================================================================
    // Presets
    // ========================================================================

    /// Refresh the list of available plugin presets from the config directory.
    pub fn refresh_presets(&mut self) {
        self.available_presets.clear();
        self.selected_preset_index = 0;

        if let Some(presets_dir) = crate::config::get_plugin_presets_dir()
            && let Ok(entries) = std::fs::read_dir(&presets_dir)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && ext == "json"
                    && let Some(filename) = path.file_name()
                {
                    self.available_presets
                        .push(filename.to_string_lossy().to_string());
                }
            }
            self.available_presets.sort();
        }

        log::info!("Found {} plugin presets", self.available_presets.len());
    }

    /// Save the plugin chain to a file. Returns the filename used.
    pub fn save_to_file(&mut self, presets_dir: &Path, filename: &str) -> Result<String, String> {
        let filename_with_ext = if filename.ends_with(".json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };

        self.graph
            .save_to_file(presets_dir, filename)
            .map_err(|e| format!("Error saving: {}", e))?;

        self.last_loaded_preset = Some(filename_with_ext.clone());
        self.refresh_presets();
        Ok(filename_with_ext)
    }

    /// Load a plugin chain from a file. Returns `Structural` effect on success.
    pub fn load_from_file(
        &mut self,
        presets_dir: &Path,
        filename: &str,
    ) -> Result<(PluginUpdateEffect, String, Vec<String>), String> {
        let warnings = self
            .graph
            .load_from_file(presets_dir, filename)
            .map_err(|e| format!("Error loading: {}", e))?;

        self.graph.update_channel_dependent_plugins();

        let filename_with_ext = if filename.ends_with(".json") {
            filename.to_string()
        } else {
            format!("{}.json", filename)
        };
        self.last_loaded_preset = Some(filename_with_ext.clone());

        Ok((PluginUpdateEffect::Structural, filename_with_ext, warnings))
    }

    /// Save the plugin chain to the selected preset file. Returns the filename used.
    pub fn save_selected_preset(&mut self, presets_dir: &Path) -> Result<String, String> {
        if self.available_presets.is_empty() {
            return Err("No presets available".to_string());
        }

        let preset_filename = self
            .available_presets
            .get(self.selected_preset_index)
            .cloned()
            .ok_or_else(|| "Invalid preset index".to_string())?;

        self.graph
            .save_to_file(presets_dir, &preset_filename)
            .map_err(|e| format!("Error saving: {}", e))?;

        self.last_loaded_preset = Some(preset_filename.clone());
        self.refresh_presets();
        Ok(preset_filename)
    }

    /// Load the selected preset. Returns `Structural` effect, preset filename, plugin count,
    /// and any warnings about skipped plugins.
    pub fn load_selected_preset(
        &mut self,
        presets_dir: &Path,
    ) -> Result<(PluginUpdateEffect, String, usize, Vec<String>), String> {
        if self.available_presets.is_empty() {
            return Err("No presets available".to_string());
        }

        let preset_filename = self
            .available_presets
            .get(self.selected_preset_index)
            .cloned()
            .ok_or_else(|| "Invalid preset index".to_string())?;

        let warnings = self
            .graph
            .load_from_file(presets_dir, &preset_filename)
            .map_err(|e| format!("Error loading preset: {}", e))?;

        self.graph.update_channel_dependent_plugins();
        self.last_loaded_preset = Some(preset_filename.clone());
        let plugin_count = self.graph.len();

        Ok((
            PluginUpdateEffect::Structural,
            preset_filename,
            plugin_count,
            warnings,
        ))
    }

    /// Select the next preset in the list.
    pub fn select_next_preset(&mut self) {
        if !self.available_presets.is_empty() {
            self.selected_preset_index =
                (self.selected_preset_index + 1) % self.available_presets.len();
        }
    }

    /// Select the previous preset in the list.
    pub fn select_previous_preset(&mut self) {
        if !self.available_presets.is_empty() {
            if self.selected_preset_index == 0 {
                self.selected_preset_index = self.available_presets.len() - 1;
            } else {
                self.selected_preset_index -= 1;
            }
        }
    }

    // ========================================================================
    // Per-Plugin Presets
    // ========================================================================

    /// Get the per-plugin presets directory for a given plugin type.
    /// Path: `{config_dir}/plugins/{plugin_type_name}/`
    pub(super) fn plugin_preset_dir(plugin_type: &PluginType) -> Option<std::path::PathBuf> {
        let config_dir = crate::config::get_app_config_dir()?;
        let dir = config_dir
            .join("plugins")
            .join(plugin_type.name().to_lowercase().replace(' ', "_"));
        Some(dir)
    }

    /// List available presets for a specific plugin type.
    /// Returns sorted list of preset names (without .json extension).
    pub fn list_plugin_presets(plugin_type: &PluginType) -> Vec<String> {
        let Some(dir) = Self::plugin_preset_dir(plugin_type) else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        names
    }

    /// Save a single plugin's settings as a preset.
    /// Path: `{config_dir}/plugins/{plugin_type_name}/{preset_name}.json`
    pub fn save_plugin_preset(
        &self,
        plugin_idx: usize,
        preset_name: &str,
    ) -> Result<String, String> {
        let plugin = self
            .graph
            .get_plugin(plugin_idx)
            .ok_or_else(|| format!("Plugin index {} out of range", plugin_idx))?;

        let dir = Self::plugin_preset_dir(&plugin.plugin_type())
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Could not create preset directory: {}", e))?;

        let filename = format!("{}.json", preset_name);
        let full_path = dir.join(&filename);

        let json = serde_json::to_string_pretty(&plugin.settings)
            .map_err(|e| format!("Serialization error: {}", e))?;
        std::fs::write(&full_path, json)
            .map_err(|e| format!("Could not write preset file: {}", e))?;

        log::info!(
            "Saved plugin preset '{}' to {}",
            preset_name,
            full_path.display()
        );
        Ok(filename)
    }

    /// Load a preset into a specific plugin slot.
    /// Returns `Structural` effect since settings are fully replaced.
    pub fn load_plugin_preset(
        &mut self,
        plugin_idx: usize,
        preset_name: &str,
    ) -> Result<PluginUpdateEffect, String> {
        let plugin = self
            .graph
            .get_plugin(plugin_idx)
            .ok_or_else(|| format!("Plugin index {} out of range", plugin_idx))?;

        let dir = Self::plugin_preset_dir(&plugin.plugin_type())
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        let filename = if preset_name.ends_with(".json") {
            preset_name.to_string()
        } else {
            format!("{}.json", preset_name)
        };
        let full_path = dir.join(&filename);

        let json = std::fs::read_to_string(&full_path)
            .map_err(|e| format!("Could not read preset file: {}", e))?;

        let settings: PluginSettings =
            serde_json::from_str(&json).map_err(|e| format!("Invalid preset file: {}", e))?;

        // Verify the preset is for the same plugin type by creating a temp plugin
        // and comparing types
        let temp = Plugin {
            id: 0,
            enabled: true,
            settings: settings.clone(),
            permanent: false,
            suspended: false,
            name: None,
        };
        let loaded_type = temp.plugin_type();
        let current_type = plugin.plugin_type();
        if loaded_type != current_type {
            return Err(format!(
                "Preset is for {} but plugin is {}",
                loaded_type.name(),
                current_type.name()
            ));
        }

        // Apply the settings
        if let Some(plugin) = self.graph.get_plugin_mut(plugin_idx) {
            plugin.settings = settings;
        }
        self.graph.update_channel_dependent_plugins();

        log::info!(
            "Loaded plugin preset '{}' into slot {}",
            preset_name,
            plugin_idx
        );
        Ok(PluginUpdateEffect::Structural)
    }

    /// Delete a preset for a specific plugin type.
    pub fn delete_plugin_preset(plugin_type: &PluginType, preset_name: &str) -> Result<(), String> {
        let dir = Self::plugin_preset_dir(plugin_type)
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        let filename = if preset_name.ends_with(".json") {
            preset_name.to_string()
        } else {
            format!("{}.json", preset_name)
        };
        let full_path = dir.join(&filename);

        std::fs::remove_file(&full_path).map_err(|e| format!("Could not delete preset: {}", e))?;

        log::info!("Deleted plugin preset '{}'", preset_name);
        Ok(())
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Determine the update effect based on the plugin and parameter.
    pub(super) fn determine_update_effect(
        &self,
        plugin_idx: Option<usize>,
        param_idx: usize,
        channel_count_changed: bool,
    ) -> PluginUpdateEffect {
        if channel_count_changed {
            return PluginUpdateEffect::Structural;
        }

        if let Some(idx) = plugin_idx {
            if let Some(plugin) = self.graph.get_plugin(idx) {
                if param_index_to_engine_param(&plugin.settings, param_idx).is_some() {
                    return PluginUpdateEffect::Parameter {
                        plugin_index: idx,
                        param_index: param_idx,
                    };
                }
            }
        }

        PluginUpdateEffect::Structural
    }

    /// Determine the update effect for a node-ID-addressed parameter change.
    pub(super) fn determine_update_effect_by_node_id(
        &self,
        node_id: crate::plugin_graph::GraphNodeId,
        param_idx: usize,
        channel_count_changed: bool,
    ) -> PluginUpdateEffect {
        if channel_count_changed {
            return PluginUpdateEffect::Structural;
        }
        if let Some(node) = self.graph.nodes.get(&node_id) {
            if param_index_to_engine_param(&node.plugin.settings, param_idx).is_some() {
                return PluginUpdateEffect::ParameterByNodeId {
                    node_id,
                    param_index: param_idx,
                };
            }
        }
        PluginUpdateEffect::Structural
    }

    // -- Channel conflict detection & suspension --

    pub fn find_channel_conflicts(&self, input_channels: usize) -> Vec<ChannelConflict> {
        self.graph.find_channel_conflicts(input_channels)
    }

    /// Find and suspend all incompatible plugins, then update channel-dependent plugins.
    pub fn suspend_incompatible(&mut self, input_channels: usize) {
        let conflicts = self.graph.find_channel_conflicts(input_channels);
        let indices: Vec<usize> = conflicts.iter().map(|c| c.index).collect();
        self.graph.suspend_plugins(&indices);
        self.graph.update_channel_dependent_plugins();
    }

    /// Clear all suspensions and update channel-dependent plugins.
    pub fn clear_suspensions(&mut self) {
        self.graph.clear_suspensions();
        self.graph.update_channel_dependent_plugins();
    }

    pub fn has_suspensions(&self) -> bool {
        self.graph.has_suspensions()
    }
}
