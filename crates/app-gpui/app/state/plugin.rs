//! Plugin state management.
//!
//! Thin wrapper around `PluginController` from sotf-player, adding GPUI-specific
//! state (graph view, workflow canvas, pending update tracking, etc.).

use std::ops::{Deref, DerefMut};

use crate::app::types::PluginUpdateType;
use crate::components::plugins::theme::RackThemeState;
use gpui::Entity;
use gpui_ui_kit::workflow::{NodeId, WorkflowCanvas};
use sotf_audio_player::{
    AbTestController, AbTestError, ConnectionDrag, EarTrainingCourse, EarTrainingProgress,
    EqTrainingConfig, EqTrainingSession, GraphNodeId, GraphSelection, NodeDrag, Plugin,
    PluginController, PluginSettings, PluginUpdateEffect, TrialAnswer, TrialCue, TrialMode,
};
use sotf_audio_player_midi::MidiMappingEngine;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PluginState {
    ctrl: PluginController,

    /// GPUI-specific: tracks whether chain has been modified since last save
    /// and what kind of plugin update is pending.
    pub update_state: PluginUpdateState,

    /// GPUI-specific: selected cell in the routing matrix UI.
    /// Selected matrix route: (stable plugin instance ID, input, output).
    pub matrix_selected_cell: Option<(usize, usize, usize)>,

    /// GPUI-specific: graph/workflow view state.
    pub graph_state: PluginGraphState,

    /// GPUI-specific: A/B Compare plugin transient state.
    pub ab_compare_state: AbCompareState,

    /// Embedded, reproducible chain-level listening-test workflow.
    pub listening_test_state: ListeningTestState,

    /// GPUI-specific: plugin UI view mode and overlay/picker open flags.
    pub plugin_ui_state: PluginUiState,

    /// GPUI-specific: per-plugin preset picker state and destructive-action
    /// confirmations.
    pub preset_state: PluginPresetState,

    /// GPUI-specific: chain-level bypass / auto-gain / solo state.
    pub chain_state: PluginChainState,

    /// MIDI controller → plugin parameter mapping engine
    pub midi_mapping: MidiMappingEngine,

    /// Rack-level plugin theme + per-plugin overrides (UI-only, persisted to
    /// gpui state file separately from engine config).
    pub rack_theme_state: RackThemeState,

    /// Plugins discovered by scanning external plugin directories.
    pub scanned_external_plugins: Vec<sotf_plugins::PluginDescriptor>,

    /// Discovery and sandbox-permission state for the external-plugin settings surface.
    pub external_plugin_ui: ExternalPluginUiState,
}

#[derive(Debug, Clone, Default)]
pub struct ExternalPluginUiState {
    pub scan_in_progress: bool,
    pub scan_completed: bool,
    pub scan_error: Option<String>,
    pub runtime_update_in_progress: bool,
    pub runtime_error: Option<String>,
    pub runtime_summary: Option<ExternalPluginRuntimeSummary>,
    /// Structured plugin-host build diagnostics. They remain visible until a
    /// clean host rebuild clears the authoritative engine field.
    pub build_diagnostics: Vec<sotf_audio::engine::PluginBuildDiagnostic>,
    /// Latest structured status for every isolated external-plugin worker.
    pub worker_statuses: Vec<sotf_audio::engine::IsolatedExternalPluginWorkerStatus>,
    /// Persistent insertion/load validation errors keyed by descriptor id/path.
    pub load_errors: HashMap<String, String>,
    /// Runtime worker errors keyed by stable player plugin instance id.
    pub worker_errors: HashMap<usize, String>,
    /// Number of external-plugin scan results currently exposed by the settings list.
    pub visible_scan_results: usize,
}

pub const EXTERNAL_PLUGIN_SCAN_PAGE_SIZE: usize = 100;

impl ExternalPluginUiState {
    pub fn reset_scan_result_pagination(&mut self) {
        self.visible_scan_results = EXTERNAL_PLUGIN_SCAN_PAGE_SIZE;
    }

    pub fn visible_scan_result_count(&self, total: usize) -> usize {
        self.visible_scan_results
            .max(EXTERNAL_PLUGIN_SCAN_PAGE_SIZE)
            .min(total)
    }

    pub fn show_more_scan_results(&mut self, total: usize) -> usize {
        self.visible_scan_results = self
            .visible_scan_result_count(total)
            .saturating_add(EXTERNAL_PLUGIN_SCAN_PAGE_SIZE)
            .min(total);
        self.visible_scan_results
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalPluginWorkerHealth {
    Healthy,
    Degraded,
    Failed,
}

pub fn external_plugin_worker_health(
    status: &sotf_audio::engine::IsolatedExternalPluginWorkerStatus,
) -> ExternalPluginWorkerHealth {
    use sotf_audio::engine::{
        IsolatedExternalPluginSandboxStatus, IsolatedExternalPluginWorkerEvent,
    };

    if status.error.is_some()
        || matches!(
            status.event,
            Some(
                IsolatedExternalPluginWorkerEvent::Exited { .. }
                    | IsolatedExternalPluginWorkerEvent::NotRunning
            )
        )
    {
        return ExternalPluginWorkerHealth::Failed;
    }

    if matches!(
        status.event,
        Some(
            IsolatedExternalPluginWorkerEvent::AlreadyRunning
                | IsolatedExternalPluginWorkerEvent::Started { .. }
        )
    ) && status.sandbox_status == IsolatedExternalPluginSandboxStatus::Enforced
        && status.sandbox_reason.is_none()
    {
        ExternalPluginWorkerHealth::Healthy
    } else {
        ExternalPluginWorkerHealth::Degraded
    }
}

pub fn external_plugin_error_key(plugin: &sotf_plugins::PluginDescriptor) -> String {
    format!("{}:{}", plugin.id, plugin.path.display())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExternalPluginRuntimeSummary {
    pub runtime_external_access_disabled: bool,
    pub persistent_grant_count: usize,
    pub media_read_path_count: usize,
    pub protected_import_path_count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExternalPluginScanCounts {
    pub total: usize,
    pub loadable: usize,
    pub discovered: usize,
    pub unsupported: usize,
}

impl ExternalPluginScanCounts {
    pub fn from_plugins(plugins: &[sotf_plugins::PluginDescriptor]) -> Self {
        let mut counts = Self {
            total: plugins.len(),
            ..Self::default()
        };
        for plugin in plugins {
            match plugin.scan_status {
                sotf_plugins::PluginScanStatus::Loadable => counts.loadable += 1,
                sotf_plugins::PluginScanStatus::Discovered => counts.discovered += 1,
                sotf_plugins::PluginScanStatus::UnsupportedByBuild => counts.unsupported += 1,
            }
        }
        counts
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
impl From<sotf_audio_player::config::PluginSandboxRuntimeStatus> for ExternalPluginRuntimeSummary {
    fn from(status: sotf_audio_player::config::PluginSandboxRuntimeStatus) -> Self {
        Self {
            runtime_external_access_disabled: status.runtime_external_access_disabled,
            persistent_grant_count: status.persistent_grant_count,
            media_read_path_count: status.media_read_paths.len(),
            protected_import_path_count: status.protected_import_paths.len(),
        }
    }
}

/// Tracks whether the plugin chain has been modified since the last save and
/// what kind of engine update is queued.
#[derive(Debug, Clone, Default)]
pub struct PluginUpdateState {
    /// Whether the chain has been modified since last save.
    pub plugin_graph_modified: bool,
    /// Pending plugin update to apply on the next tick.
    pub pending_plugin_update: Option<PluginUpdateType>,
    /// The actor accepted a structural update; the Player result is still pending.
    pub pending_ack: Option<PluginUpdateAcknowledgement>,
    pub acknowledged_processing_graph: Option<serde_json::Value>,
}

/// Attribution for the graph actually sent to the actor.
#[derive(Debug, Clone)]
pub struct PluginUpdateAcknowledgement {
    pub receipt: crate::app::player_handle::PlayerCommandReceipt,
    pub submitted_graph: Option<serde_json::Value>,
    pub submitted_processing_graph: Option<serde_json::Value>,
    pub correction_revisions: [Option<u64>; 3],
    pub routing: bool,
    pub audition: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RoutingConnectionForm {
    pub endpoints: [Option<(GraphNodeId, usize)>; 2],
    pub open: Option<usize>,
    pub highlighted: [Option<usize>; 2],
}

/// GPUI-specific state for the graph / workflow view.
#[derive(Debug, Clone, Default)]
pub struct PluginGraphState {
    /// Routing edits never replace the playback controller until Apply.
    pub draft: Option<PluginController>,
    pub draft_base: Option<serde_json::Value>,
    pub draft_solo: Option<(GraphNodeId, HashMap<GraphNodeId, bool>)>,
    pub draft_generation: u64,
    pub applying_original: Option<sotf_audio_player::PluginGraph>,
    pub header_details_open: bool,
    pub connection_list_open: bool,
    pub connection_form: RoutingConnectionForm,
    pub palette_open: Option<bool>,
    pub keyboard_help_open: Option<bool>,
    pub graph_selection: GraphSelection,
    pub graph_connection_drag: Option<ConnectionDrag>,
    pub graph_node_drag: Option<NodeDrag>,
    pub keyboard_palette_index: usize,
    pub keyboard_connect_source: Option<(GraphNodeId, usize)>,
    pub keyboard_target_port: usize,
    pub workflow_canvas: Option<Entity<WorkflowCanvas>>,
    pub workflow_node_mapping: Option<WorkflowNodeMapping>,
    pub editing_plugin_node: Option<gpui_ui_kit::workflow::NodeId>,
    /// The `GraphNodeId` (UUID) of the plugin being edited in the graph modal.
    /// When set, `set_plugin_param` redirects to the node-ID-based path so
    /// parameter edits work even in non-linear graphs.
    pub editing_graph_node_uuid: Option<sotf_audio_player::GraphNodeId>,
    /// Serialized settings captured when the graph modal opens. This enables
    /// a truthful discard path while parameter changes remain live-previewed.
    pub editing_original_settings_json: Option<String>,
    /// Enabled state captured when the graph modal opens. Bypass is edited by
    /// the shared plugin shell, so it participates in dirty/discard semantics.
    pub editing_original_enabled: Option<bool>,
    /// Whether the close control is showing the dirty-edit confirmation.
    pub confirm_close_dirty: bool,
    /// Whether generic setup/output controls are expanded in the graph node
    /// editor. This is separate from the rack configuration overlay.
    pub graph_config_open: bool,
}

impl PluginGraphState {
    pub fn settings_are_dirty(
        &self,
        current_settings: Option<&PluginSettings>,
        current_enabled: Option<bool>,
    ) -> bool {
        let settings_changed = current_settings
            .and_then(|settings| serde_json::to_string(settings).ok())
            .zip(self.editing_original_settings_json.as_deref())
            .is_some_and(|(current, original)| current != original);
        let enabled_changed = current_enabled
            .zip(self.editing_original_enabled)
            .is_some_and(|(current, original)| current != original);
        settings_changed || enabled_changed
    }

    pub fn original_settings(&self) -> Option<PluginSettings> {
        self.editing_original_settings_json
            .as_deref()
            .and_then(|settings| serde_json::from_str(settings).ok())
    }

    /// Restore every modal-owned live-preview field captured at open time.
    pub fn restore_original(&self, plugin: &mut Plugin) {
        if let Some(settings) = self.original_settings() {
            plugin.settings = settings;
        }
        if let Some(enabled) = self.editing_original_enabled {
            plugin.enabled = enabled;
        }
    }

    pub fn clear_editing_context(&mut self) {
        self.editing_plugin_node = None;
        self.editing_graph_node_uuid = None;
        self.editing_original_settings_json = None;
        self.editing_original_enabled = None;
        self.confirm_close_dirty = false;
        self.graph_config_open = false;
    }
}

/// GPUI-specific transient state for the A/B Compare plugin UI.
#[derive(Debug, Clone, Default)]
pub struct AbCompareState {
    /// Dropdown open states for AB Compare plugin
    pub ab_compare_dropdowns: ABCompareDropdowns,
    /// File paths for AB Compare loaded configs (for display)
    pub ab_compare_file_a: Option<String>,
    pub ab_compare_file_b: Option<String>,
    /// Parsed sub-rack contents for A/B paths (kept in sync with engine JSON)
    pub ab_path_a: Vec<sotf_audio_player::controllers::ab_compare_path::PluginInRack>,
    pub ab_path_b: Vec<sotf_audio_player::controllers::ab_compare_path::PluginInRack>,
    /// Selected sub-plugin index within each A/B path
    pub ab_path_a_selected: Option<usize>,
    pub ab_path_b_selected: Option<usize>,
    /// Which path's "add plugin" menu is currently open
    pub ab_add_menu_target: Option<ABPathTarget>,
}

#[derive(Debug, Clone)]
pub struct ListeningTestState {
    pub surface: EarTrainingSurface,
    pub eq_config: EqTrainingConfig,
    pub eq_session: Option<EqTrainingSession>,
    pub eq_selected_band: usize,
    pub eq_filtered: bool,
    pub eq_progress: EarTrainingProgress,
    pub eq_active_course: Option<EarTrainingCourse>,
    pub eq_adaptive: bool,
    pub eq_audition_node_id: Option<GraphNodeId>,
    pub eq_sources: Vec<std::path::PathBuf>,
    pub eq_source_index: usize,
    pub eq_loop_enabled: bool,
    pub eq_loop_range: Option<(f64, f64)>,
    pub path_a: Option<sotf_audio_player::controllers::ab_compare_path::PathConfig>,
    pub path_b: Option<sotf_audio_player::controllers::ab_compare_path::PathConfig>,
    pub path_a_label: String,
    pub path_b_label: String,
    pub ab_test: AbTestController,
    pub trial_mode: sotf_audio_player::controllers::ab_test_session::TrialMode,
    pub level_match_config: sotf_audio_player::controllers::ab_test_session::LevelMatchConfig,
    pub segment_start_ms: u64,
    /// Optional self-reported confidence (0-100) for the pending trial.
    /// `None` until the listener supplies a value; a default number must
    /// never masquerade as a reported judgment.
    pub confidence: Option<u8>,
    pub notes: String,
    pub status: String,
    pub path_a_canvas: Option<Entity<WorkflowCanvas>>,
    pub path_b_canvas: Option<Entity<WorkflowCanvas>>,
    pub editing_path_target: Option<ABPathTarget>,
    pub editing_path_node_id: Option<String>,
    pub editing_path_parameters: String,
    pub graph_add_menu_target: Option<ABPathTarget>,
}

impl Default for ListeningTestState {
    fn default() -> Self {
        let eq_progress = sotf_audio_player::config::get_ear_training_progress_path()
            .and_then(|path| EarTrainingProgress::load(&path).ok())
            .unwrap_or_default();
        Self {
            surface: EarTrainingSurface::EqBands,
            eq_config: EqTrainingConfig::default(),
            eq_session: None,
            eq_selected_band: 0,
            eq_filtered: false,
            eq_progress,
            eq_active_course: None,
            eq_adaptive: false,
            eq_audition_node_id: None,
            eq_sources: Vec::new(),
            eq_source_index: 0,
            eq_loop_enabled: false,
            eq_loop_range: None,
            path_a: None,
            path_b: None,
            path_a_label: "Path A".into(),
            path_b_label: "Path B".into(),
            ab_test: AbTestController::default(),
            trial_mode: sotf_audio_player::controllers::ab_test_session::TrialMode::Abx,
            level_match_config:
                sotf_audio_player::controllers::ab_test_session::LevelMatchConfig::default(),
            segment_start_ms: 0,
            confidence: None,
            notes: String::new(),
            status: String::new(),
            path_a_canvas: None,
            path_b_canvas: None,
            editing_path_target: None,
            editing_path_node_id: None,
            editing_path_parameters: String::new(),
            graph_add_menu_target: None,
        }
    }
}

impl ListeningTestState {
    /// UI setup identity for accepting a background comparison measurement.
    /// Serialization covers nested rack/graph parameters without requiring
    /// presentation code to duplicate the plugin configuration schema.
    pub fn preparation_snapshot(
        &self,
        media_path: Option<std::path::PathBuf>,
    ) -> Option<(std::path::PathBuf, serde_json::Value)> {
        let media_path = media_path?;
        serde_json::to_value((
            self.path_a.as_ref()?,
            self.path_b.as_ref()?,
            &self.path_a_label,
            &self.path_b_label,
            self.segment_start_ms,
            self.level_match_config,
        ))
        .ok()
        .map(|setup| (media_path, setup))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EarTrainingSurface {
    #[default]
    EqBands,
    Courses,
    Progress,
    BlindComparison,
}

/// GPUI-specific state for the plugin UI view mode and open pickers/overlays.
/// Transient EQ graph edit that has not yet been sent to the audio engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqDragPreview {
    pub plugin_idx: usize,
    pub band_idx: usize,
    pub frequency: f64,
    pub gain_db: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoudnessControlUiRequest {
    pub runtime_instance_id: u64,
    pub request_id: u64,
    pub engine_index: usize,
    pub operation: &'static str,
    pub command: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PluginUiState {
    /// Monotonic transient command IDs for this UI process; never serialized as a preset.
    pub next_loudness_control_request_id: u64,
    /// Latest control request/error by stable graph plugin ID.
    pub loudness_control_requests: HashMap<usize, LoudnessControlUiRequest>,
    /// Last measured rack content-box width, independent of sidebar and panel ratios.
    pub rack_width: Option<f32>,
    pub rack_editor_width: Option<f32>,
    pub listening_width: Option<f32>,
    /// Comparison workspace presentation; blind assignments remain in the controller.
    pub listening_workspace: ListeningWorkspaceState,
    /// Expanded generated-editor sections, keyed by stable plugin instance ID.
    pub plugin_sections: HashMap<usize, Vec<String>>,
    /// Open Matrix route selector: plugin instance and whether it selects output.
    pub matrix_route_menu: Option<(usize, bool)>,
    /// Which plugin UI view mode to show
    pub plugin_ui_view: PluginUiView,
    /// Whether the controller picker dropdown is open
    pub controller_picker_open: bool,
    /// Whether the rack-level plugin configuration popover is open.
    pub rack_config_overlay_open: bool,
    /// Whether the plugin skin picker dropdown is open.
    pub rack_skin_picker_open: bool,
    /// Compact EQ: whether the global config panel is expanded.
    pub eq_compact_config_open: bool,
    /// Compact EQ: whether the graph is visible in inspector mode.
    pub eq_compact_graph_visible: bool,
    /// FIR EQ point currently being previewed. Committed on pointer release.
    pub eq_drag_preview: Option<EqDragPreview>,
}

impl PluginUiState {
    pub fn preview_eq_drag(&mut self, preview: EqDragPreview) {
        self.eq_drag_preview = Some(preview);
    }

    pub fn take_eq_drag_preview_for(&mut self, plugin_idx: usize) -> Option<EqDragPreview> {
        self.eq_drag_preview
            .filter(|preview| preview.plugin_idx == plugin_idx)?;
        self.eq_drag_preview.take()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PracticeWorkspaceState {
    pub frequency_details_open: bool,
    pub paused: bool,
    pub confirm_end: bool,
    pub resume_playback: bool,
}

impl PracticeWorkspaceState {
    pub fn interaction_locked(&self) -> bool {
        self.paused || self.confirm_end
    }
}

#[derive(Debug, Clone)]
pub struct ListeningWorkspaceState {
    pub source_request: Option<std::sync::Arc<()>>,
    pub preparation_request: Option<std::sync::Arc<()>>,
    pub practice: PracticeWorkspaceState,
    pub results_open: bool,
    pub confirm_end: bool,
    pub metadata_open: bool,
    pub trials_open: bool,
    pub selected_answer: Option<TrialAnswer>,
    pub auditioned: bool,
    pub planned_trials: usize,
    pub paused: bool,
}

impl Default for ListeningWorkspaceState {
    fn default() -> Self {
        Self {
            source_request: None,
            preparation_request: None,
            practice: PracticeWorkspaceState::default(),
            results_open: false,
            confirm_end: false,
            metadata_open: false,
            trials_open: false,
            selected_answer: None,
            auditioned: false,
            planned_trials: 12,
            paused: false,
        }
    }
}

impl ListeningWorkspaceState {
    /// Abandon the result without allowing a late worker to replace a newer request.
    pub fn cancel_preparation(&mut self) {
        self.preparation_request = None;
    }

    pub fn begin_preparation(&mut self) -> std::sync::Arc<()> {
        let request = std::sync::Arc::new(());
        self.preparation_request = Some(request.clone());
        request
    }

    /// Only the newest worker may consume the pending request and publish state.
    pub fn finish_preparation(&mut self, request: &std::sync::Arc<()>) -> bool {
        if !self
            .preparation_request
            .as_ref()
            .is_some_and(|current| std::sync::Arc::ptr_eq(current, request))
        {
            return false;
        }
        self.preparation_request = None;
        true
    }

    pub fn interaction_locked(&self) -> bool {
        self.paused || self.confirm_end
    }

    /// A pending blind trial always takes precedence over a stale results view.
    pub fn phase(&self, pending: Option<TrialMode>, completed: usize) -> ListeningWorkspacePhase {
        if pending.is_some() {
            ListeningWorkspacePhase::Listen
        } else if self.results_open {
            ListeningWorkspacePhase::Results
        } else if completed > 0 {
            ListeningWorkspacePhase::Listen
        } else {
            ListeningWorkspacePhase::Setup
        }
    }

    pub fn can_submit(&self, mode: TrialMode) -> bool {
        self.auditioned
            && !self.interaction_locked()
            && matches!(
                (mode, self.selected_answer),
                (TrialMode::Abx, Some(TrialAnswer::A | TrialAnswer::B))
                    | (
                        TrialMode::BlindAb,
                        Some(TrialAnswer::First | TrialAnswer::Second)
                    )
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListeningWorkspacePhase {
    Setup,
    Listen,
    Results,
}

/// GPUI-specific state for the per-plugin preset picker and destructive-action
/// confirmations.
#[derive(Debug, Clone, Default)]
pub struct PluginPresetState {
    /// Per-plugin preset picker state
    pub plugin_preset_open: Option<usize>, // Some(plugin_idx) when open
    pub plugin_preset_list: Vec<String>, // Available presets for the open plugin
    pub plugin_preset_save_mode: bool,   // True when in save mode (text input)
    pub plugin_preset_input: String,     // Save preset name input

    /// Pending confirmation for destructive actions
    pub confirm_remove_plugin: Option<usize>, // Some(plugin_idx) awaiting confirmation
    pub confirm_delete_preset: Option<(usize, String)>, // Some((plugin_idx, preset_name)) awaiting confirmation
    /// Two-click confirmation for replacing every per-channel EQ curve.
    pub confirm_eq_copy_to_all: Option<(usize, usize)>, // (plugin_idx, source_channel)
    /// Two-click confirmation for replacing the selected channel from global EQ.
    pub confirm_eq_copy_from_all: Option<(usize, usize)>, // (plugin_idx, target_channel)
}

/// GPUI-specific state for chain-level bypass, auto-gain and solo.
#[derive(Debug, Clone, Default)]
pub struct PluginChainState {
    /// When true, all plugins are bypassed (audio passes through unchanged)
    pub chain_bypass: bool,
    /// Chain-level auto-gain toggle
    pub chain_autogain: bool,
    /// Last UI timer frame that pushed a chain auto-gain correction.
    pub chain_autogain_last_frame: u64,
    /// Index of the currently soloed plugin (None = no solo active)
    pub soloed_plugin_index: Option<usize>,
    /// Saved enabled states before solo was activated (to restore on un-solo)
    pub pre_solo_enabled_states: Vec<bool>,
}

impl Deref for PluginState {
    type Target = PluginController;
    fn deref(&self) -> &Self::Target {
        &self.ctrl
    }
}

impl DerefMut for PluginState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctrl
    }
}

impl PluginState {
    /// Resolve custom-editor instance IDs in the same controller as the editor.
    /// A draft-only node must never fall back to an active playback node.
    pub fn editor_settings_mut_by_instance_id(
        &mut self,
        instance_id: usize,
    ) -> Option<&mut PluginSettings> {
        let graph = if self.graph_state.editing_graph_node_uuid.is_some() {
            &mut self.routing_controller_mut().graph
        } else {
            &mut self.ctrl.graph
        };
        graph
            .nodes
            .values_mut()
            .find(|node| node.plugin.id == instance_id)
            .map(|node| &mut node.plugin.settings)
    }

    pub fn editor_plugin_mut(&mut self, index: usize) -> Option<&mut Plugin> {
        if let Some(id) = self.graph_state.editing_graph_node_uuid {
            self.routing_controller_mut()
                .graph
                .nodes
                .get_mut(&id)
                .map(|node| &mut node.plugin)
        } else {
            self.ctrl.graph.get_plugin_mut(index)
        }
    }

    pub fn record_editor_effect(&mut self, effect: PluginUpdateEffect) {
        if self.graph_state.editing_graph_node_uuid.is_some() && self.graph_state.draft.is_some() {
            return;
        }
        self.record_ab_test_effect(effect);
    }
    pub fn begin_routing_draft(&mut self) {
        if self.graph_state.draft.is_none() {
            self.graph_state.draft_generation = self.graph_state.draft_generation.wrapping_add(1);
            self.graph_state.draft_base = serde_json::to_value(&self.ctrl.graph).ok();
            self.graph_state.draft = Some(self.ctrl.clone());
        }
    }

    pub fn routing_controller(&self) -> &PluginController {
        self.graph_state.draft.as_ref().unwrap_or(&self.ctrl)
    }

    pub fn routing_controller_mut(&mut self) -> &mut PluginController {
        self.begin_routing_draft();
        // The fallback keeps this accessor panic-free even if initialization changes.
        self.graph_state.draft.as_mut().unwrap_or(&mut self.ctrl)
    }

    pub fn discard_routing_draft(&mut self) {
        self.graph_state.draft = None;
        self.graph_state.draft_base = None;
        self.graph_state.draft_solo = None;
        self.graph_state.workflow_canvas = None;
        self.graph_state.clear_editing_context();
        self.graph_state.graph_selection.clear();
        self.graph_state.keyboard_connect_source = None;
    }

    pub fn routing_draft_is_dirty(&self) -> bool {
        self.graph_state.draft.as_ref().is_some_and(|draft| {
            let mut current = serde_json::to_value(&draft.graph).ok();
            let mut base = self.graph_state.draft_base.clone();
            // Adding then removing a node advances the allocator, but does not
            // change the routing the user can apply. Keep the full base snapshot
            // for the separate concurrent-edit guard in apply_routing_draft.
            for snapshot in [&mut current, &mut base] {
                if let Some(serde_json::Value::Object(graph)) = snapshot {
                    graph.remove("next_plugin_id");
                }
            }
            current != base
        })
    }

    pub fn apply_routing_draft(&mut self) -> Result<(), String> {
        if !self.routing_draft_is_dirty() {
            return Ok(());
        }
        if self.graph_state.applying_original.is_some() {
            return Err("A routing update is already pending.".into());
        }
        let Some(draft) = self.graph_state.draft.as_ref() else {
            return Ok(());
        };
        if self.graph_state.draft_base != serde_json::to_value(&self.ctrl.graph).ok() {
            return Err(
                "The active processing chain changed. Discard the routing draft and try again."
                    .into(),
            );
        }
        draft.graph.validate_routing()?;
        self.graph_state.applying_original = Some(self.ctrl.graph.clone());
        self.ctrl.graph = draft.graph.clone();
        self.discard_routing_draft();
        self.update_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        self.update_state.plugin_graph_modified = true;
        Ok(())
    }

    pub fn finish_routing_apply(&mut self, succeeded: bool) {
        if let Some(original) = self.graph_state.applying_original.take()
            && !succeeded
        {
            // Editing remains available while the submitted graph is awaiting
            // acknowledgement. Preserve those newer edits when submission fails.
            let candidate = self
                .graph_state
                .draft
                .take()
                .unwrap_or_else(|| self.ctrl.clone());
            self.ctrl.graph = original;
            self.graph_state.draft_base = serde_json::to_value(&self.ctrl.graph).ok();
            self.graph_state.draft = Some(candidate);
            self.graph_state.workflow_canvas = None;
        }
    }

    pub fn toggle_routing_solo(&mut self, node_id: GraphNodeId) {
        let previous = self.graph_state.draft_solo.take();
        if let Some((_, saved)) = &previous {
            for (id, enabled) in saved {
                if let Some(node) = self.routing_controller_mut().graph.nodes.get_mut(id) {
                    node.plugin.enabled = *enabled;
                }
            }
        }
        if previous.as_ref().is_some_and(|(id, _)| *id == node_id) {
            return;
        }
        let graph = &mut self.routing_controller_mut().graph;
        let saved = graph
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.plugin.enabled))
            .collect();
        for (id, node) in &mut graph.nodes {
            node.plugin.enabled = *id == node_id;
        }
        self.graph_state.draft_solo = Some((node_id, saved));
    }

    fn record_ab_test_effect(&mut self, effect: PluginUpdateEffect) {
        // A cue can arrive before the tick publishes the runtime graph. Never
        // replace that rebuild with a parameter command for a not-yet-live slot,
        // or erase teardown work when leaving an already inactive runtime.
        if matches!(effect, PluginUpdateEffect::None)
            || matches!(
                self.update_state.pending_plugin_update,
                Some(PluginUpdateType::Structural)
            )
        {
            return;
        }
        self.update_state.pending_plugin_update = match effect {
            PluginUpdateEffect::None => None,
            PluginUpdateEffect::Structural => Some(PluginUpdateType::Structural),
            PluginUpdateEffect::Parameter {
                plugin_index,
                param_index,
            } => Some(PluginUpdateType::Parameter {
                plugin_index,
                param_index,
            }),
            PluginUpdateEffect::ParameterByNodeId {
                node_id,
                param_index,
            } => Some(PluginUpdateType::ParameterByNodeId {
                node_id,
                param_index,
            }),
        };
    }

    pub fn start_ab_test_trial(&mut self, mode: TrialMode) -> Result<u32, AbTestError> {
        if !self.listening_test_state.ab_test.view().runtime_active {
            let mut controller = std::mem::take(&mut self.listening_test_state.ab_test);
            let effect = controller.enter_runtime(&mut self.ctrl.graph);
            self.listening_test_state.ab_test = controller;
            self.record_ab_test_effect(effect?);
        }
        self.listening_test_state.ab_test.start_trial(mode)
    }

    pub fn activate_ab_test_cue(&mut self, cue: TrialCue) -> Result<(), AbTestError> {
        let mut controller = std::mem::take(&mut self.listening_test_state.ab_test);
        let effect = controller.activate_cue(&mut self.ctrl.graph, cue);
        self.listening_test_state.ab_test = controller;
        self.record_ab_test_effect(effect?);
        Ok(())
    }

    pub fn commit_ab_test_answer(
        &mut self,
        answer: TrialAnswer,
        confidence: Option<u8>,
        notes: Option<String>,
    ) -> Result<(), AbTestError> {
        self.listening_test_state
            .ab_test
            .commit_trial(answer, confidence, notes)
    }

    pub fn leave_ab_test_runtime(&mut self) -> Result<(), AbTestError> {
        let mut controller = std::mem::take(&mut self.listening_test_state.ab_test);
        let effect = controller.leave_runtime(&mut self.ctrl.graph);
        self.listening_test_state.ab_test = controller;
        self.record_ab_test_effect(effect?);
        Ok(())
    }

    /// Whether the rack view can represent the current plugin graph.
    /// Returns false when the topology is non-linear (parallel branches,
    /// merges) — in that case the user must use the graph view.
    pub fn is_rack_available(&self) -> bool {
        self.ctrl.is_linear()
    }

    pub fn has_external_plugins(&self) -> bool {
        self.ctrl
            .graph
            .nodes
            .values()
            .any(|node| matches!(node.plugin.settings, PluginSettings::External { .. }))
    }

    /// Mirror authoritative engine diagnostics into persistent UI state and
    /// associate worker failures with the exact descriptor that owns the
    /// corresponding engine slot.
    pub fn sync_external_plugin_engine_diagnostics(
        &mut self,
        build_diagnostics: Vec<sotf_audio::engine::PluginBuildDiagnostic>,
        worker_statuses: Vec<sotf_audio::engine::IsolatedExternalPluginWorkerStatus>,
    ) {
        let instance_errors: Vec<(usize, Option<String>, bool)> = worker_statuses
            .iter()
            .filter_map(|status| {
                let instance_id = status.plugin_instance_id.or_else(|| {
                    self.ctrl.graph.nodes.values().find_map(|node| {
                        (self.ctrl.graph.get_engine_index(node.id) == Some(status.plugin_index))
                            .then_some(node.plugin.id)
                    })
                })?;
                Some((
                    instance_id,
                    status.error.clone(),
                    external_plugin_worker_health(status) == ExternalPluginWorkerHealth::Healthy,
                ))
            })
            .collect();

        let ui = &mut self.external_plugin_ui;
        ui.build_diagnostics = build_diagnostics;
        ui.worker_statuses = worker_statuses;
        for (instance_id, error, explicitly_healthy) in instance_errors {
            match error {
                Some(error) => {
                    ui.worker_errors.insert(instance_id, error);
                }
                None if explicitly_healthy => {
                    ui.worker_errors.remove(&instance_id);
                }
                None => {}
            }
        }
    }

    /// Return the build diagnostic owned by one external-plugin instance.
    /// Stable instance identity wins; engine chain/node indices are a fallback
    /// for older configurations that do not carry the instance parameter.
    pub fn external_plugin_build_diagnostic(
        &self,
        plugin_instance_id: Option<usize>,
        engine_index: Option<usize>,
    ) -> Option<&sotf_audio::engine::PluginBuildDiagnostic> {
        use sotf_audio::engine::PluginBuildTarget;

        self.external_plugin_ui
            .build_diagnostics
            .iter()
            .find(|diagnostic| {
                if let Some(diagnostic_instance_id) = diagnostic.plugin_instance_id {
                    return Some(diagnostic_instance_id) == plugin_instance_id;
                }

                let Some(engine_index) = engine_index else {
                    return false;
                };
                match diagnostic.target {
                    PluginBuildTarget::ChainPlugin { plugin_index } => plugin_index == engine_index,
                    PluginBuildTarget::GraphNode { node_id } => node_id == engine_index,
                    PluginBuildTarget::GraphEdge { from_node, to_node } => {
                        from_node == engine_index || to_node == engine_index
                    }
                    PluginBuildTarget::Host => false,
                }
            })
    }
}

/// Dropdown open states for AB Compare plugin UI
#[derive(Debug, Clone, Copy, Default)]
pub struct ABCompareDropdowns {
    pub path_a_open: bool,
    pub path_b_open: bool,
}

/// Which A/B path the add menu targets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ABPathTarget {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PluginViewMode {
    #[default]
    Rack,
    Graph,
}

/// Which view to show for a plugin's UI
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PluginUiView {
    /// Graphical plugin-specific UI (EQ graph, compressor curves, etc.)
    #[default]
    UI,
    /// Hardware controller view with MIDI mappings for a specific controller layout
    Controller(String),
    /// Simple text-based parameter table
    Simple,
}

impl PluginUiView {
    pub fn is_simple(&self) -> bool {
        matches!(self, Self::Simple)
    }

    pub fn is_controller(&self) -> bool {
        matches!(self, Self::Controller(_))
    }

    pub fn is_ui(&self) -> bool {
        matches!(self, Self::UI)
    }
}

/// Returns the list of available controller layout names
pub fn available_controllers() -> Vec<(&'static str, &'static str)> {
    vec![("xone_k2", "Xone:K2"), ("lcxl", "Launch Control XL")]
}

#[derive(Clone, Default, Debug)]
pub struct WorkflowNodeMapping {
    pub node_to_plugin: HashMap<NodeId, usize>,
    pub plugin_to_node: HashMap<usize, NodeId>,
    pub input_node_id: Option<NodeId>,
    pub output_node_id: Option<NodeId>,
}

impl Default for PluginState {
    fn default() -> Self {
        Self {
            ctrl: PluginController::new(),
            update_state: PluginUpdateState::default(),
            matrix_selected_cell: None,
            graph_state: PluginGraphState::default(),
            ab_compare_state: AbCompareState::default(),
            listening_test_state: ListeningTestState::default(),
            plugin_ui_state: PluginUiState::default(),
            preset_state: PluginPresetState::default(),
            chain_state: PluginChainState::default(),
            midi_mapping: MidiMappingEngine::new(),
            rack_theme_state: RackThemeState::default(),
            scanned_external_plugins: Vec::new(),
            external_plugin_ui: ExternalPluginUiState::default(),
        }
    }
}

impl PluginState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear pending destructive-action confirmations.
    /// Called on any structural change (toggle, reorder, screen switch)
    /// to prevent stale confirmation state from persisting.
    pub fn clear_confirmations(&mut self) {
        self.preset_state.confirm_remove_plugin = None;
        self.preset_state.confirm_delete_preset = None;
        self.preset_state.confirm_eq_copy_to_all = None;
        self.preset_state.confirm_eq_copy_from_all = None;
    }

    /// Sync the parsed A/B path state from the engine-side JSON config strings.
    /// Call this when selecting an AB Compare plugin or after loading a preset.
    pub fn sync_ab_path_state(&mut self, path_a_json: &str, path_b_json: &str) {
        use sotf_audio_player::controllers::ab_compare_path::parse_path_config;
        self.ab_compare_state.ab_path_a = parse_path_config(path_a_json);
        self.ab_compare_state.ab_path_b = parse_path_config(path_b_json);
        // Clamp selections to valid range
        if let Some(sel) = self.ab_compare_state.ab_path_a_selected
            && sel >= self.ab_compare_state.ab_path_a.len()
        {
            self.ab_compare_state.ab_path_a_selected = None;
        }
        if let Some(sel) = self.ab_compare_state.ab_path_b_selected
            && sel >= self.ab_compare_state.ab_path_b.len()
        {
            self.ab_compare_state.ab_path_b_selected = None;
        }
    }

    /// Read an A/B path from the current edit target, falling back to the
    /// linear rack index outside the graph modal.
    pub fn ab_compare_path_plugins(
        &self,
        plugin_idx: usize,
        param_idx: usize,
    ) -> Vec<sotf_audio_player::controllers::ab_compare_path::PluginInRack> {
        use sotf_audio_player::controllers::ab_compare_path::parse_path_config;
        let settings = if let Some(node_id) = self.graph_state.editing_graph_node_uuid {
            self.graph
                .nodes
                .get(&node_id)
                .map(|node| &node.plugin.settings)
        } else {
            self.graph
                .get_plugin(plugin_idx)
                .map(|plugin| &plugin.settings)
        };
        let Some(PluginSettings::ABCompare {
            path_a_config,
            path_b_config,
            ..
        }) = settings
        else {
            return Vec::new();
        };
        let path_a_idx = sotf_plugins::param_specs::index_of(
            sotf_plugins::param_specs::ab_compare::PARAMS,
            "path_a_config",
        );
        if param_idx == path_a_idx {
            parse_path_config(path_a_config)
        } else {
            parse_path_config(path_b_config)
        }
    }

    /// Store an A/B config and source path against the current edit target.
    pub fn set_ab_compare_config_file_for_edit_target(
        &mut self,
        plugin_idx: usize,
        param_idx: usize,
        config: String,
        source_path: String,
    ) -> PluginUpdateEffect {
        let effect = if let Some(node_id) = self.graph_state.editing_graph_node_uuid {
            self.set_ab_compare_config_file_by_node_id(node_id, param_idx, config, source_path)
        } else {
            self.set_ab_compare_config_file(plugin_idx, param_idx, config, source_path)
        };
        if !matches!(effect, PluginUpdateEffect::None) {
            self.update_state.pending_plugin_update = Some(PluginUpdateType::Structural);
        }
        effect
    }

    /// Clear all A/B path state (called when an AB Compare plugin is removed).
    pub fn clear_ab_path_state(&mut self) {
        self.ab_compare_state.ab_path_a.clear();
        self.ab_compare_state.ab_path_b.clear();
        self.ab_compare_state.ab_path_a_selected = None;
        self.ab_compare_state.ab_path_b_selected = None;
        self.ab_compare_state.ab_add_menu_target = None;
    }
}
