use super::common::factories::{album, track};
use super::common::{
    FilterType, TestEQFilter, add_eq_band, clamp_parameter, denormalize_parameter,
    denormalize_parameter_log, normalize_parameter, normalize_parameter_log, remove_eq_band,
    validate_eq_filter,
};
use gpui_design::DesignSystem;
use sotf_audio_player::ui_params::TuiEditablePlugin;
use sotf_audio_player::{
    MetadataImportCandidate, NodePosition, PluginGraph, PluginSettings, PluginType,
};
use sotf_audio_player_gpui::IconName;
use sotf_audio_player_gpui::app::keybindings::KeymapPreset;
use sotf_audio_player_gpui::app::state::{
    ExternalPluginUiState, ExternalPluginWorkerHealth, PluginGraphState, PluginState,
    external_plugin_error_key, external_plugin_worker_health,
};
use sotf_audio_player_gpui::app::types::MetadataEditorState;
use sotf_audio_player_gpui::components::design::typography_rems_from_rules;
use sotf_audio_player_gpui::components::dialogs::get_keybindings_for_screen;
use sotf_audio_player_gpui::components::home::album_card::{
    AlbumCardMode, album_card_height, format_channel_info, format_dr, format_sample_info,
    get_format_from_path,
};
use sotf_audio_player_gpui::components::plugins::ab_compare_view_state;
use sotf_audio_player_gpui::components::plugins::common::{
    compute_transfer, format_shortcut_label,
};
use sotf_audio_player_gpui::components::plugins::custom_view_registry::{
    GpuiViewRegistry, plugin_type_key,
};
use sotf_audio_player_gpui::components::plugins::spatial_spider::data::correlation_row;
use sotf_audio_player_gpui::components::plugins::theme::{
    PluginThemeId, plugin_theme_id_for_app_theme,
};
use sotf_audio_player_gpui::components::plugins::ui_layout_renderer::extract_file_paths;
use sotf_audio_player_gpui::components::wizard_continue_label;
use sotf_audio_player_gpui::components::{settings_tab_icon_name, settings_tab_label};
use sotf_audio_player_gpui::i18n::{
    AutoEqFormTranslations, HeadphoneEasyTranslations, HeadphoneEqTranslations, Language,
    PluginGraphTranslations, RoomEqEasyTranslations, SettingsSurfaceTranslations,
    StreamsTranslations, Translations,
};
use sotf_audio_player_gpui::plugin_file_picker::{FilePickerOpenTarget, file_picker_open_target};
use sotf_audio_player_gpui::theme::{Theme, ThemeId};
use sotf_audio_player_gpui::{ExternalPluginScanCounts, InputMode, Screen, SettingsTab};
use sotf_audio_player_midi::auto_map;
use sotf_audio_player_midi::layouts::{lcxl_layout, xone_k2_layout};
use sotf_plugins::layout_solver::solve_layout;
use sotf_plugins::param_specs::{self, ParamType};
use sotf_plugins::plugin_layout::{ColumnRole, ControlType};
use sotf_plugins::{
    ExternalPluginSandboxMode, ExternalPluginState, PluginDescriptor, PluginFormat,
    PluginScanStatus, PluginUiKind, catalog_entry,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::visit::Visit;

#[derive(Default)]
struct TextLabelVisitor {
    labels: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for TextLabelVisitor {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "label"
            && let syn::Expr::Path(receiver) = call.receiver.as_ref()
            && receiver.path.is_ident("text")
            && let Some(syn::Expr::Lit(argument)) = call.args.first()
            && let syn::Lit::Str(label) = &argument.lit
        {
            self.labels.insert(label.value());
        }

        syn::visit::visit_expr_method_call(self, call);
    }
}

fn app_source(relative: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

#[test]
fn metadata_candidate_title_matches_editor_scope() {
    let mut album = album("Original Album")
        .add_track(track("Original Track").build())
        .build();
    album.id = Some(1);
    let candidate = MetadataImportCandidate {
        provider_id: "musicbrainz".to_string(),
        provider_entity_id: "scenario-release".to_string(),
        title: Some("Imported Track".to_string()),
        artist: Some("Imported Artist".to_string()),
        album_artist: Some("Imported Artist".to_string()),
        album_title: Some("Imported Album".to_string()),
        year: Some(2024),
        track_number: Some(1),
        disc_number: Some(1),
        isrc: None,
        score: 97,
    };

    let mut album_editor = MetadataEditorState::for_album(&album).unwrap();
    album_editor.apply_candidate(candidate.clone());
    assert_eq!(album_editor.fields.title, "Imported Album");

    let mut track_editor = MetadataEditorState::for_track(&album.tracks[0]);
    track_editor.apply_candidate(candidate);
    assert_eq!(track_editor.fields.title, "Imported Track");
}

fn repo_source(relative: &str) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("app-gpui should live under crates/");
    std::fs::read_to_string(repo_root.join(relative))
        .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"))
}

fn matrix_first_gain(graph: &PluginGraph, plugin_instance_id: usize) -> f32 {
    graph
        .nodes
        .values()
        .find(|node| node.plugin.id == plugin_instance_id)
        .and_then(|node| match &node.plugin.settings {
            PluginSettings::Matrix { matrix, .. } => matrix.first().copied(),
            _ => None,
        })
        .expect("matrix plugin instance should exist")
}

#[test]
fn matrix_stale_coordinates_do_not_alias_after_channel_shrink() {
    use sotf_audio_player_gpui::components::plugins::checked_matrix_cell_index;

    assert_eq!(checked_matrix_cell_index(2, 0, 2, 2, 4), None);
    assert_eq!(checked_matrix_cell_index(0, 2, 2, 2, 4), None);
    assert_eq!(checked_matrix_cell_index(1, 1, 2, 2, 4), Some(3));
}

#[test]
fn spatial_spider_correlation_row_rejects_invalid_shape() {
    let matrix = [1.0, -0.2, -0.2, 1.0];

    assert_eq!(correlation_row(&matrix, 2, 1), Some(&matrix[2..4]));
    assert_eq!(correlation_row(&matrix, 3, 0), None);
    assert_eq!(correlation_row(&matrix, 2, 2), None);
}

#[test]
fn matrix_selection_follows_instance_identity_across_reorder_and_removal() {
    use sotf_audio_player_gpui::components::plugins::matrix_settings_mut_by_instance_id;

    let mut graph = PluginGraph::new();
    let first_node = graph
        .add_plugin_node(&PluginType::Matrix, NodePosition::new(0.0, 0.0))
        .expect("first matrix should be constructible");
    let selected_node = graph
        .add_plugin_node(&PluginType::Matrix, NodePosition::new(100.0, 0.0))
        .expect("selected matrix should be constructible");
    let first_instance_id = graph.nodes[&first_node].plugin.id;
    let selected_instance_id = graph.nodes[&selected_node].plugin.id;

    graph.nodes.get_mut(&first_node).unwrap().position.x = 200.0;
    graph.nodes.get_mut(&selected_node).unwrap().position.x = 0.0;
    if let Some(PluginSettings::Matrix { matrix, .. }) =
        matrix_settings_mut_by_instance_id(&mut graph, selected_instance_id)
    {
        matrix[0] = 0.25;
    } else {
        panic!("selected matrix should resolve by stable instance ID");
    }

    assert_eq!(matrix_first_gain(&graph, selected_instance_id), 0.25);
    assert_eq!(matrix_first_gain(&graph, first_instance_id), 1.0);

    graph.remove_node(first_node);
    assert!(matrix_settings_mut_by_instance_id(&mut graph, first_instance_id).is_none());
    assert!(matrix_settings_mut_by_instance_id(&mut graph, selected_instance_id).is_some());
}

#[test]
fn nonlinear_matrix_modal_mutates_displayed_instance_not_an_unrelated_node() {
    use sotf_audio_player_gpui::components::plugins::matrix_settings_mut_by_instance_id;

    let mut graph = PluginGraph::new();
    graph
        .add_plugin_node(&PluginType::EQ, NodePosition::new(0.0, 0.0))
        .expect("EQ should be constructible");
    let unrelated_node = graph
        .add_plugin_node(&PluginType::Matrix, NodePosition::new(200.0, 100.0))
        .expect("unrelated matrix should be constructible");
    let displayed_node = graph
        .add_plugin_node(&PluginType::Matrix, NodePosition::new(100.0, 200.0))
        .expect("displayed matrix should be constructible");
    let unrelated_id = graph.nodes[&unrelated_node].plugin.id;
    let displayed_id = graph.nodes[&displayed_node].plugin.id;

    if let Some(PluginSettings::Matrix { matrix, .. }) =
        matrix_settings_mut_by_instance_id(&mut graph, displayed_id)
    {
        matrix[0] = 0.5;
    } else {
        panic!("displayed nonlinear Matrix node should resolve by instance ID");
    }

    assert_eq!(matrix_first_gain(&graph, displayed_id), 0.5);
    assert_eq!(matrix_first_gain(&graph, unrelated_id), 1.0);
}

#[test]
fn escape_closed_graph_modal_cannot_redirect_later_rack_matrix_edits() {
    use sotf_audio_player_gpui::components::plugins::plugin_instance_id_for_render;

    let graph = PluginGraph::with_default_rack();
    let rack_instance_id = graph
        .get_plugin(0)
        .expect("default rack should expose a linear plugin")
        .id;
    let (stale_node_uuid, stale_instance_id) = graph
        .nodes
        .iter()
        .find_map(|(node_id, node)| {
            (node.plugin.id != rack_instance_id).then_some((*node_id, node.plugin.id))
        })
        .expect("default rack should contain another graph node");

    assert_eq!(
        plugin_instance_id_for_render(
            InputMode::EditingPluginNode,
            Some(stale_node_uuid),
            &graph,
            0,
        ),
        Some(stale_instance_id),
    );
    assert_eq!(
        plugin_instance_id_for_render(InputMode::Normal, Some(stale_node_uuid), &graph, 0),
        Some(rack_instance_id),
        "normal rack rendering must ignore a stale graph-modal node ID",
    );

    let original_settings = &graph.nodes[&stale_node_uuid].plugin.settings;
    let original_settings_json =
        serde_json::to_string(original_settings).expect("plugin settings should serialize");
    let mut graph_state = PluginGraphState {
        editing_plugin_node: Some(stale_node_uuid),
        editing_graph_node_uuid: Some(stale_node_uuid),
        editing_original_settings_json: Some(original_settings_json),
        editing_original_enabled: Some(true),
        confirm_close_dirty: true,
        ..PluginGraphState::default()
    };
    assert!(!graph_state.settings_are_dirty(Some(original_settings), Some(true)));
    assert!(graph_state.settings_are_dirty(Some(original_settings), Some(false)));
    let mut bypassed_plugin = graph.nodes[&stale_node_uuid].plugin.clone();
    bypassed_plugin.enabled = false;
    graph_state.restore_original(&mut bypassed_plugin);
    assert!(bypassed_plugin.enabled, "discard must restore bypass state");
    graph_state.editing_original_settings_json = Some("{}".to_string());
    assert!(graph_state.settings_are_dirty(Some(original_settings), Some(true)));
    graph_state.clear_editing_context();
    assert!(graph_state.editing_plugin_node.is_none());
    assert!(graph_state.editing_graph_node_uuid.is_none());
    assert!(graph_state.editing_original_settings_json.is_none());
    assert!(graph_state.editing_original_enabled.is_none());
    assert!(!graph_state.confirm_close_dirty);

    let cancel_source = app_source("ui/player_view.rs");
    assert!(
        cancel_source.contains("InputMode::EditingPluginNode")
            && cancel_source.contains(".clear_editing_context();"),
        "the universal Cancel handler must clear graph-modal identity before returning to Normal"
    );
}

#[test]
fn graph_modal_dirty_close_uses_keyboard_accessible_actions() {
    let modal_source = app_source("components/plugins/ui_graph/player_view.rs");
    for (id, label) in [
        ("modal-continue-editing", "continue_editing"),
        ("modal-keep-changes", "keep_changes"),
        ("modal-close", "discard_changes"),
    ] {
        assert!(
            modal_source.contains(&format!(
                "Button::new(\n                                            \"{id}\""
            )) || modal_source.contains(&format!("Button::new(\"{id}\"")),
            "{id} must be a keyboard-activatable toolkit Button"
        );
        assert!(modal_source.contains(label), "{id} must use localized text");
    }
    assert!(
        modal_source.contains(".aria_label("),
        "dirty-close actions must expose accessible labels"
    );
}

#[test]
fn every_file_path_param_resolves_its_settings_value() {
    for plugin_type in PluginType::all() {
        let mut settings = PluginSettings::default_for(&plugin_type)
            .expect("catalog plugin types must have default settings");
        let file_params: Vec<(usize, &'static str)> = settings
            .param_specs()
            .iter()
            .enumerate()
            .filter(|(_, spec)| matches!(spec.param_type, ParamType::FilePath))
            .map(|(index, spec)| (index, spec.engine_key))
            .collect();

        if file_params.is_empty() {
            continue;
        }

        for (_, engine_key) in &file_params {
            let value = format!("/tmp/{engine_key}.test");
            match (&mut settings, *engine_key) {
                (PluginSettings::Convolution { ir_file, .. }, "ir_file") => *ir_file = value,
                (PluginSettings::BinauralDecoder { sofa_file, .. }, "sofa_file") => {
                    *sofa_file = value;
                }
                (
                    PluginSettings::BinauralDecoder {
                        hrtf_database_dir, ..
                    },
                    "hrtf_database_dir",
                ) => {
                    *hrtf_database_dir = value;
                }
                (PluginSettings::XTC { room_ir_file, .. }, "room_ir_file") => {
                    *room_ir_file = Some(value);
                }
                (PluginSettings::ABCompare { path_a_file, .. }, "path_a_config") => {
                    *path_a_file = value;
                }
                (PluginSettings::ABCompare { path_b_file, .. }, "path_b_config") => {
                    *path_b_file = value;
                }
                _ => panic!(
                    "FilePath ParamSpec {engine_key} for {plugin_type:?} lacks test coverage"
                ),
            }
        }

        let paths = extract_file_paths(settings.param_specs(), &settings);
        for (index, engine_key) in file_params {
            let expected = format!("/tmp/{engine_key}.test");
            assert_eq!(
                paths.get(&index).map(String::as_str),
                Some(expected.as_str()),
                "{plugin_type:?}.{engine_key} did not resolve from PluginSettings"
            );
        }
    }
}

#[test]
fn ab_compare_view_follows_reloaded_and_discarded_plugin_settings() {
    let mut graph = PluginGraph::new();
    let node_id = graph
        .add_plugin_node(&PluginType::ABCompare, NodePosition::new(0.0, 0.0))
        .unwrap();
    let plugin = &mut graph.nodes.get_mut(&node_id).unwrap().plugin;
    let PluginSettings::ABCompare {
        path_a_config,
        path_a_file,
        ..
    } = &mut plugin.settings
    else {
        unreachable!();
    };
    *path_a_config = r#"{"type":"Plugin","plugin_type":"gain","parameters":{}}"#.into();
    *path_a_file = "/tmp/original-a.json".into();

    let original_json = serde_json::to_string(&plugin.settings).unwrap();
    let reloaded: PluginSettings = serde_json::from_str(&original_json).unwrap();
    let (reloaded_a, reloaded_a_file, _, _) = ab_compare_view_state(&reloaded);
    assert_eq!(reloaded_a.len(), 1);
    assert_eq!(reloaded_a_file.as_deref(), Some("/tmp/original-a.json"));

    let graph_state = PluginGraphState {
        editing_original_settings_json: Some(original_json),
        editing_original_enabled: Some(plugin.enabled),
        ..PluginGraphState::default()
    };
    let PluginSettings::ABCompare {
        path_a_config,
        path_a_file,
        ..
    } = &mut plugin.settings
    else {
        unreachable!();
    };
    *path_a_config = r#"{"type":"Plugin","plugin_type":"eq","parameters":{}}"#.into();
    *path_a_file = "/tmp/replacement-a.json".into();
    graph_state.restore_original(plugin);

    let (discarded_a, discarded_a_file, _, _) = ab_compare_view_state(&plugin.settings);
    assert_eq!(discarded_a[0].plugin_type, "gain");
    assert_eq!(discarded_a_file.as_deref(), Some("/tmp/original-a.json"));
}

#[test]
fn async_ab_picker_captures_graph_identity_before_await() {
    let source = app_source("ui/player_view.rs");
    let handler = source
        .split("pub(crate) fn on_open_ab_config_file")
        .nth(1)
        .expect("A/B file handler must exist");
    let capture = handler
        .find("let target_node_id")
        .expect("handler must snapshot graph node identity");
    let spawn = handler
        .find("cx.spawn")
        .expect("handler must open the file dialog asynchronously");
    assert!(
        capture < spawn,
        "graph identity must be captured before await"
    );
    assert!(
        handler.contains("graph.nodes.contains_key(&node_id)")
            && handler.contains("PluginUpdateEffect::Structural"),
        "async result must reject a removed node and update UI only after structural success"
    );
}

#[test]
fn plugin_rendering_uses_cached_registry_and_safe_layout_slots() {
    let plugins = app_source("components/plugins/mod.rs");
    assert!(
        plugins.contains("static GPUI_VIEW_REGISTRY")
            || plugins.contains("OnceLock<GpuiViewRegistry>"),
        "plugin renderer should cache the custom view registry"
    );
    assert!(
        !plugins.contains("let registry = GpuiViewRegistry::new();"),
        "plugin renderer must not construct GpuiViewRegistry per render"
    );

    let layout = app_source("ui/three_panel_layout.rs");
    assert!(
        !layout.contains(".unwrap()"),
        "three-panel rendering must not panic when a solved layout slot is missing"
    );
}

#[test]
fn dev_api_compile_guard_is_active_for_release_builds() {
    let lib = app_source("lib.rs");
    assert!(
        lib.contains("#[cfg(all(feature = \"dev-api\", not(debug_assertions)))]")
            && lib.contains("compile_error!("),
        "release builds must not compile the dev API"
    );
    assert!(
        !lib.contains("// #[cfg(all(feature = \"dev-api\", not(debug_assertions)))]"),
        "dev-api release compile guard must not be commented out"
    );
}

#[test]
fn release_gpui_recipe_excludes_dev_api_feature() {
    let justfile = repo_source("justfile");
    let gpui_recipe = justfile
        .split("\n[group('build')]\ngpui:\n")
        .nth(1)
        .and_then(|rest| rest.split("\n\n").next())
        .expect("missing build gpui recipe");

    assert!(
        gpui_recipe.contains("{{release_test_features}}"),
        "release gpui recipe must use the feature set that excludes dev-api"
    );
    assert!(
        !gpui_recipe.contains("{{test_features_macos}}") && !gpui_recipe.contains("dev-api"),
        "release gpui recipe must not compile the QA-only dev-api feature"
    );
}

#[test]
fn eq_renderer_caches_static_frequency_grid() {
    let eq_render = app_source("components/plugins/ui_eq/render.rs");
    assert!(
        eq_render.contains("static EQ_FREQUENCY_POINTS")
            && eq_render.contains("eq_frequency_points()"),
        "EQ renderer should reuse the log-spaced frequency grid"
    );
    assert!(
        !eq_render.contains("let freq_points: Vec<f64> = (0..num_points)"),
        "EQ renderer must not rebuild the static frequency grid every render"
    );
}

#[test]
fn eq_renderer_uses_curve_render_cache_helper() {
    let eq_render = app_source("components/plugins/ui_eq/render.rs");
    assert!(
        eq_render.contains("struct EqCurveRenderCache")
            && eq_render.contains("fn get_or_build(")
            && eq_render.contains("eq_curve_cache()")
            && eq_render.contains(".lock()")
            && eq_render.contains("cache.get_or_build(filters, freq_points)")
            && eq_render.contains("filter.topology")
            && eq_render.contains(".lambda")
            && eq_render.contains("filter.kautz_sections"),
        "EQ curve and band-response data should be built through a cache helper"
    );
    assert!(
        !eq_render.contains("let combined_response: Vec<f64> = freq_points"),
        "EQ renderer must not allocate combined response data inline every render"
    );
}

#[test]
fn app_gpui_unwrap_expect_production_audit_is_current() {
    fn visit_rs_files(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path).unwrap_or_else(|err| {
            panic!("failed to read {}: {err}", path.display());
        }) {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["app", "components", "ui"] {
        visit_rs_files(&root.join(dir), &mut files);
    }

    let mut actual = BTreeMap::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        let source = std::fs::read_to_string(&file)
            .unwrap_or_else(|err| panic!("failed to read {relative}: {err}"));
        let count = source.matches(".unwrap()").count() + source.matches(".expect(").count();
        if count > 0 {
            actual.insert(relative, count);
        }
    }

    let expected = BTreeMap::from([
        ("app/cast.rs".to_string(), 2),
        ("app/dev_api/server/parse.rs".to_string(), 1),
        ("app/federation/local.rs".to_string(), 2),
        ("app/midi_input.rs".to_string(), 3),
        ("app/remote/consts.rs".to_string(), 5),
        ("components/headphone_eq/actions/misc.rs".to_string(), 2),
        ("components/plugins/spatial_spider/data.rs".to_string(), 13),
        (
            "components/plugins/ui_layout_renderer/render.rs".to_string(),
            1,
        ),
        ("components/spinorama_eq/misc.rs".to_string(), 1),
        ("components/spinorama_eq/types.rs".to_string(), 1),
    ]);

    assert_eq!(
        actual, expected,
        "app-gpui unwrap/expect audit changed; replace user-triggerable sites \
         with graceful handling or update this baseline with a reason"
    );
}

#[test]
fn upmixer_renderer_uses_static_config_metadata() {
    let upmixer = app_source("components/plugins/ui_upmixer/render.rs");
    assert!(
        upmixer.contains("static UPMIXER_CONFIG_SPECS")
            && upmixer.contains("upmixer_config_specs()"),
        "upmixer renderer should reuse static config tab metadata"
    );
    assert!(
        !upmixer.contains("CONFIG_ITEMS.iter().enumerate().map(|(i, label)|"),
        "upmixer renderer must not rebuild tab ids and labels ad hoc every render"
    );
}

#[test]
fn plugin_ui_p1_hardcoded_pixels_are_documented_or_tokenized() {
    for relative in [
        "components/plugins/ui_eq/render.rs",
        "components/plugins/ui_upmixer/render.rs",
    ] {
        let source = app_source(relative);
        assert!(
            source.contains("Ds") && source.contains("d."),
            "{relative} should use the app design-token shim for spacing and sizing"
        );
        assert!(
            source.contains("intentional:") || source.contains("CHART_"),
            "{relative} should document domain-specific fixed geometry that cannot be a design token"
        );
    }
}

#[test]
fn channel_mute_solo_buttons_have_click_handlers() {
    let mute_solo = app_source("components/plugins/ui_mute_solo.rs");
    assert!(
        mute_solo
            .matches(".on_mouse_down(MouseButton::Left")
            .count()
            >= 3,
        "channel mute/solo/dim controls must be clickable"
    );
    assert!(mute_solo.contains("MsdAction::Mute"));
    assert!(mute_solo.contains("MsdAction::Solo"));
    assert!(mute_solo.contains("MsdAction::Dim"));
    assert!(
        mute_solo.contains("pending_plugin_update"),
        "channel mute/solo/dim clicks must schedule plugin graph reconfiguration"
    );
}

#[test]
fn test_typography_rems_platform_presets_affect_type_scale() {
    let neutral = typography_rems_from_rules(&DesignSystem::neutral().typography);
    let apple = typography_rems_from_rules(&DesignSystem::apple_hig().typography);
    let material = typography_rems_from_rules(&DesignSystem::material3().typography);

    assert!(apple.text_sm.0 > neutral.text_sm.0);
    assert!(apple.text_lg.0 > neutral.text_lg.0);
    assert!(material.text_lg.0 > neutral.text_lg.0);
}

#[test]
fn test_plugin_file_picker_keys_have_open_targets() {
    assert_eq!(
        file_picker_open_target("sofa_file"),
        Some(FilePickerOpenTarget::Sofa)
    );
    assert_eq!(
        file_picker_open_target("ir_file"),
        Some(FilePickerOpenTarget::Ir)
    );
    assert_eq!(
        file_picker_open_target("room_ir_file"),
        Some(FilePickerOpenTarget::Ir)
    );
    assert_eq!(
        file_picker_open_target("path_a_config"),
        Some(FilePickerOpenTarget::AbConfig("a"))
    );
    assert_eq!(
        file_picker_open_target("path_b_config"),
        Some(FilePickerOpenTarget::AbConfig("b"))
    );
}

#[test]
fn test_ab_compare_paths_tab_declares_actionable_file_pickers() {
    let params = param_specs::ab_compare::PARAMS;
    let paths_tab = param_specs::ab_compare::LAYOUT
        .tabs
        .iter()
        .find(|tab| tab.name == "Paths")
        .expect("A/B Compare must expose its config loaders in a Paths tab");

    let file_picker_keys: Vec<&'static str> = paths_tab
        .controls
        .iter()
        .filter(|control| matches!(control.control_type, ControlType::FilePicker))
        .map(|control| params[control.param_index].engine_key)
        .collect();

    assert_eq!(file_picker_keys, vec!["path_a_config", "path_b_config"]);
    for key in file_picker_keys {
        assert!(
            matches!(params.iter().find(|param| param.engine_key == key), Some(param) if matches!(param.param_type, ParamType::FilePath)),
            "{key} must remain a FilePath parameter"
        );
        assert!(
            file_picker_open_target(key).is_some(),
            "{key} must have a GPUI open action"
        );
    }
}

#[test]
fn test_settings_tabs_have_distinct_scan_icons() {
    assert_eq!(
        settings_tab_icon_name(SettingsTab::Library),
        IconName::Library
    );
    assert_eq!(
        settings_tab_icon_name(SettingsTab::Theme),
        IconName::PenTool
    );
    assert_eq!(
        settings_tab_icon_name(SettingsTab::AudioDevice),
        IconName::Speaker
    );
    assert_eq!(
        settings_tab_icon_name(SettingsTab::ReleaseChannel),
        IconName::AudioWaveform
    );
}

#[test]
fn test_settings_tabs_use_product_labels() {
    let translations = Translations::for_language(Language::English);

    assert_eq!(
        settings_tab_label(SettingsTab::Library, &translations),
        "Local Library"
    );
    assert_eq!(
        settings_tab_label(SettingsTab::Theme, &translations),
        "Appearance"
    );
    assert_eq!(
        settings_tab_label(SettingsTab::Misc, &translations),
        "Resources"
    );
    assert_eq!(
        settings_tab_label(SettingsTab::ReleaseChannel, &translations),
        "Features"
    );
}

#[test]
fn listening_test_workflow_is_localized_in_every_supported_language() {
    let localized_titles: Vec<_> = Language::all()
        .iter()
        .map(|language| {
            let translations = Translations::for_language(*language);
            let listening = &translations.listening_test;
            for value in [
                listening.eq.mode_eq,
                listening.eq.mode_blind,
                listening.eq.title,
                listening.eq.subtitle,
                listening.eq.session_setup,
                listening.eq.question,
                listening.eq.original,
                listening.eq.filtered,
                listening.eq.submit,
                listening.eq.shortcuts,
                listening.eq.add_ab_plugin,
                listening.setup.title,
                listening.setup.subtitle,
                listening.setup.measure_prepare,
                listening.setup.graph_hint,
                listening.setup.level.target_metric,
                listening.setup.level.momentary_lufs,
                listening.setup.level.short_term_lufs,
                listening.setup.level.rms_dbfs,
                listening.setup.level.window,
                listening.setup.level.tolerance,
                listening.setup.level.max_correction,
                listening.setup.level.media_identity,
                listening.setup.level.load_saved_media,
                listening.trial.start_blind_ab,
                listening.trial.start_abx,
                listening.trial.notes_placeholder,
                listening.status.select_paths,
                listening.status.graph_updated,
                listening.status.session_saved,
            ] {
                assert!(
                    !value.trim().is_empty(),
                    "missing listening-test translation for {}",
                    language.code()
                );
            }
            listening.setup.title
        })
        .collect();

    assert_eq!(
        localized_titles
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        Language::all().len(),
        "each current language needs its own listening-test title"
    );
}

#[test]
fn listening_test_exposes_configurable_matching_and_verified_media_reload() {
    let screen = app_source("components/listening_test.rs");
    let state = app_source("app/state/plugin.rs");
    let playback = app_source("ui/player_view.rs");

    for control in [
        "listening-metric-momentary",
        "listening-metric-short-term",
        "listening-metric-rms",
        "listening-segment-start",
        "listening-window",
        "listening-tolerance",
        "listening-max-correction",
        "load-listening-media",
    ] {
        assert!(
            screen.contains(control),
            "missing listening control {control}"
        );
    }
    assert!(screen.contains("verify_media_segment"));
    assert!(screen.contains("measurement.residual_error_db()"));
    assert!(screen.contains("measurement.within_tolerance()"));
    assert!(!screen.contains("duration_ms: 3_000"));
    assert!(!screen.contains("metric: LevelMatchMetric::ShortTermLufs"));
    assert!(state.contains("level_match_config"));
    assert!(state.contains("segment_start_ms"));
    assert!(playback.contains("play_listening_source_at"));
    assert!(playback.contains("track_queue_playback"));
}

#[test]
fn listening_test_keyboard_workflow_is_registry_backed() {
    let actions = app_source("app/actions.rs");
    let bindings = app_source("app/keybindings/listening_test.rs");
    let registry = app_source("app/keybindings/mod.rs");
    let render = app_source("ui/render.rs");
    let screen = app_source("components/listening_test.rs");

    for action in [
        "EarTrainingShowEqBands",
        "EarTrainingShowBlindComparison",
        "EarTrainingStart",
        "EarTrainingPlayOriginal",
        "EarTrainingPlayFiltered",
        "EarTrainingSelectPreviousBand",
        "EarTrainingSelectNextBand",
        "EarTrainingSubmit",
        "EarTrainingNextQuestion",
        "ListeningCapturePathA",
        "ListeningCapturePathB",
        "ListeningPrepare",
        "ListeningStartBlindAb",
        "ListeningStartAbx",
        "ListeningPlayCue1",
        "ListeningPlayCue2",
        "ListeningPlayCue3",
        "ListeningCommitAnswer1",
        "ListeningCommitAnswer2",
    ] {
        assert!(actions.contains(action), "missing action {action}");
        assert!(bindings.contains(action), "missing binding for {action}");
    }

    assert!(bindings.contains("Some(\"ListeningTest\")"));
    assert!(registry.contains("bindings.extend(listening_test_bindings())"));
    assert!(registry.contains("KeybindingCategory::ListeningTests"));
    assert!(render.contains("\"PlayerView ListeningTest\""));
    assert!(render.contains("Self::listening_capture_path_a"));
    assert!(render.contains("Self::listening_commit_answer_2"));
    assert!(render.contains("Self::ear_training_play_original"));
    assert!(render.contains("Self::ear_training_next_question"));
    assert!(screen.contains("render_eq_training_workbench"));
    assert!(screen.contains("activate_eq_training_path"));
    assert!(screen.contains("fn listening_cue_for_position"));
    assert!(screen.contains("fn listening_answer_for_position"));
}

#[test]
fn autoeq_forms_use_the_workflow_detail_level() {
    for (path, state_name) in [
        (
            "components/headphone_eq/step_2_optimisation/misc.rs",
            "headphone_eq",
        ),
        ("components/room_eq/step_3_configure/misc.rs", "room_eq"),
        (
            "components/spinorama_eq/step_2_configure/misc.rs",
            "spinorama",
        ),
    ] {
        let source = app_source(path);
        assert!(
            source.contains(&format!("detail_level: {state_name}.detail_level")),
            "{path} must pass its persisted detail level to AutoEqForm"
        );
        assert!(
            !source.contains("detail_level: DetailLevel::Expert"),
            "{path} must not force the expert form"
        );
    }
}

#[test]
fn headphone_easy_mode_is_localized_in_every_supported_language() {
    let bundles: Vec<_> = Language::all()
        .iter()
        .copied()
        .map(HeadphoneEasyTranslations::for_language)
        .collect();

    for bundle in &bundles {
        for value in [
            bundle.title,
            bundle.description,
            bundle.safety,
            bundle.apply,
            bundle.undo,
            bundle.edit_in_studio,
            bundle.no_result,
            bundle.no_undo,
            bundle.restored,
        ] {
            assert!(!value.trim().is_empty());
        }
        assert!(!bundle.summary(8, -6.0).trim().is_empty());
        assert!(!bundle.applied(8, -6.0).trim().is_empty());
    }

    assert_eq!(
        bundles
            .iter()
            .map(|bundle| bundle.title)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        Language::all().len()
    );
}

#[test]
fn headphone_easy_mode_has_apply_undo_and_advanced_handoff() {
    let state = app_source("app/types/headphone_eq.rs");
    let actions = app_source("components/headphone_eq/actions/misc.rs");
    let export = app_source("components/headphone_eq/step_4_export.rs");

    assert!(state.contains("easy_mode_undo_graph: Option<PluginGraph>"));
    assert!(state.contains("easy_mode_last_apply: Option<HeadphoneEasyApplyOutcome>"));
    assert!(actions.contains("apply_headphone_easy_chain("));
    assert!(actions.contains("easy_mode_undo_graph = Some(previous_graph)"));
    assert!(actions.contains("state.app.plugin_state.graph = previous"));
    assert!(actions.contains("current_screen = crate::app::Screen::Studio"));
    assert!(export.contains("detail_level == DetailLevel::Simple"));
    assert!(export.contains("apply-headphone-easy-chain"));
    assert!(export.contains("undo-headphone-easy-chain"));
    assert!(export.contains("edit-headphone-easy-chain"));
}

#[test]
fn room_eq_easy_layouts_are_localized_in_every_supported_language() {
    use sotf_audio_player::room_eq_types::RoomEqEasyLayout;

    let bundles = Language::all()
        .iter()
        .copied()
        .map(RoomEqEasyTranslations::for_language)
        .collect::<Vec<_>>();

    for bundle in &bundles {
        assert!(!bundle.layout.trim().is_empty());
        assert!(!bundle.measured_roles.trim().is_empty());
        assert!(!bundle.channels_loaded.trim().is_empty());
        for layout in RoomEqEasyLayout::ALL {
            assert!(!bundle.description(layout).trim().is_empty());
            assert!(!bundle.configuration_title(layout, 6).trim().is_empty());
        }
    }

    assert_eq!(
        bundles
            .iter()
            .map(|bundle| bundle.layout)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        Language::all().len()
    );
}

#[test]
fn room_eq_easy_layout_selection_is_validated_before_optimization() {
    let state = app_source("app/types/room_eq/room_eq_state.rs");
    let configure = app_source("components/room_eq/step_3_configure/misc.rs");
    let optimise = app_source("components/room_eq/step_4_optimise/room.rs");

    assert!(state.contains("easy_layout: RoomEqEasyLayout"));
    assert!(configure.contains("room_eq.easy_layout.next()"));
    assert!(configure.contains("configure_preset_defaults"));
    assert!(optimise.contains("apply_room_eq_easy_layout("));
    assert!(optimise.contains("invalid_layout(&error)"));
}

#[test]
fn test_ios_settings_tabs_hide_local_library_and_keybindings() {
    let ios_tabs = SettingsTab::visible_tabs_for_ios(true);
    assert!(!ios_tabs.contains(&SettingsTab::Library));
    assert!(!ios_tabs.contains(&SettingsTab::Keybindings));
    assert!(ios_tabs.contains(&SettingsTab::Servers));
    assert!(ios_tabs.contains(&SettingsTab::AudioDevice));

    let desktop_tabs = SettingsTab::visible_tabs_for_ios(false);
    assert!(desktop_tabs.contains(&SettingsTab::Library));
    assert!(desktop_tabs.contains(&SettingsTab::Keybindings));
}

#[test]
fn test_album_card_height_grid() {
    assert_eq!(album_card_height(AlbumCardMode::Grid), 180.0);
}

#[test]
fn test_album_card_height_list() {
    assert_eq!(album_card_height(AlbumCardMode::List), 80.0);
}

#[test]
fn test_album_card_height_compact() {
    assert_eq!(album_card_height(AlbumCardMode::Compact), 56.0);
}

#[test]
fn test_format_sample_info_standard_44k() {
    assert_eq!(
        format_sample_info(Some(16), Some(44100)),
        Some("16/44.1k".to_string())
    );
}

#[test]
fn test_format_sample_info_hires_96k() {
    assert_eq!(
        format_sample_info(Some(24), Some(96000)),
        Some("24/96k".to_string())
    );
}

#[test]
fn test_format_sample_info_48k() {
    assert_eq!(
        format_sample_info(Some(24), Some(48000)),
        Some("24/48k".to_string())
    );
}

#[test]
fn test_format_sample_info_192k() {
    assert_eq!(
        format_sample_info(Some(32), Some(192000)),
        Some("32/192k".to_string())
    );
}

#[test]
fn test_format_sample_info_bit_depth_only() {
    assert_eq!(
        format_sample_info(Some(24), None),
        Some("24bit".to_string())
    );
}

#[test]
fn test_format_sample_info_sample_rate_only() {
    assert_eq!(
        format_sample_info(None, Some(44100)),
        Some("44.1k".to_string())
    );
}

#[test]
fn test_format_sample_info_none() {
    assert_eq!(format_sample_info(None, None), None);
}

#[test]
fn test_format_channel_info_surround_51() {
    assert_eq!(format_channel_info(Some(6)), Some("5.1".to_string()));
}

#[test]
fn test_format_channel_info_stereo_hidden() {
    assert_eq!(format_channel_info(Some(2)), None);
}

#[test]
fn test_format_channel_info_high_count() {
    assert_eq!(
        format_channel_info(Some(10)),
        Some("10ch (5.1.4/7.1.2)".to_string())
    );
}

#[test]
fn test_get_format_flac() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.flac")),
        Some("FLAC".to_string())
    );
}

#[test]
fn test_get_format_mp3() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.mp3")),
        Some("MP3".to_string())
    );
}

#[test]
fn test_get_format_wav() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.wav")),
        Some("WAV".to_string())
    );
}

#[test]
fn test_get_format_m4a() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.m4a")),
        Some("M4A".to_string())
    );
}

#[test]
fn test_get_format_ogg() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.ogg")),
        Some("OGG".to_string())
    );
}

#[test]
fn test_get_format_no_extension() {
    assert_eq!(get_format_from_path(Path::new("/music/album/track")), None);
}

#[test]
fn test_format_dr_some() {
    assert_eq!(format_dr(Some(14.3)), Some("14".to_string()));
}

#[test]
fn test_format_dr_rounds() {
    assert_eq!(format_dr(Some(14.7)), Some("15".to_string()));
}

#[test]
fn test_format_dr_none() {
    assert_eq!(format_dr(None), None);
}

#[test]
fn test_format_shortcut_label_first_letter() {
    assert_eq!(format_shortcut_label("Threshold", Some('t')), "[T]hreshold");
}

#[test]
fn test_format_shortcut_label_middle_letter() {
    assert_eq!(format_shortcut_label("Gain", Some('a')), "G[A]in");
}

#[test]
fn test_format_shortcut_label_not_found() {
    assert_eq!(format_shortcut_label("Gain", Some('x')), "[X] Gain");
}

#[test]
fn test_format_shortcut_label_case_insensitive() {
    assert_eq!(format_shortcut_label("RATIO", Some('r')), "[R]ATIO");
}

#[test]
fn test_format_shortcut_label_none() {
    assert_eq!(format_shortcut_label("Attack", None), "Attack");
}

#[test]
fn test_album_card_mode_eq() {
    assert_eq!(AlbumCardMode::Grid, AlbumCardMode::Grid);
    assert_ne!(AlbumCardMode::Grid, AlbumCardMode::List);
    assert_ne!(AlbumCardMode::List, AlbumCardMode::Compact);
}

#[test]
fn test_album_card_mode_copy() {
    let mode = AlbumCardMode::Grid;
    let copy = mode;
    assert_eq!(mode, copy);
}

#[test]
fn test_format_sample_info_very_high_sample_rate() {
    // DSD64 equivalent
    assert_eq!(
        format_sample_info(Some(1), Some(2822400)),
        Some("1/2822.4k".to_string())
    );
}

#[test]
fn test_format_sample_info_zero_values() {
    assert_eq!(
        format_sample_info(Some(0), Some(0)),
        Some("0/0k".to_string())
    );
}

#[test]
fn test_format_dr_zero() {
    assert_eq!(format_dr(Some(0.0)), Some("0".to_string()));
}

#[test]
fn test_format_dr_negative() {
    assert_eq!(format_dr(Some(-5.0)), Some("-5".to_string()));
}

#[test]
fn test_get_format_uppercase_extension() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.FLAC")),
        Some("FLAC".to_string())
    );
}

#[test]
fn test_get_format_mixed_case() {
    assert_eq!(
        get_format_from_path(Path::new("/music/album/track.FlAc")),
        Some("FLAC".to_string())
    );
}

#[test]
fn test_format_shortcut_label_empty_string() {
    assert_eq!(format_shortcut_label("", Some('a')), "[A] ");
}

#[test]
fn test_format_shortcut_label_single_char() {
    assert_eq!(format_shortcut_label("G", Some('g')), "[G]");
}

#[test]
fn test_format_sample_info_common_rates() {
    let test_cases = vec![
        (44100, "44.1k"),
        (48000, "48k"),
        (88200, "88.2k"),
        (96000, "96k"),
        (176400, "176.4k"),
        (192000, "192k"),
        (352800, "352.8k"),
        (384000, "384k"),
    ];

    for (rate, expected) in test_cases {
        assert_eq!(
            format_sample_info(Some(24), Some(rate)),
            Some(format!("24/{}", expected)),
            "Failed for rate {}",
            rate
        );
    }
}

#[test]
fn test_limiter_clips_at_threshold() {
    assert_eq!(compute_transfer(-10.0, -20.0, 10.0, 0.0, true), -20.0);
}

#[test]
fn test_limiter_passes_below_threshold() {
    assert_eq!(compute_transfer(-30.0, -20.0, 10.0, 0.0, true), -30.0);
}

#[test]
fn test_compressor_linear_region() {
    let result = compute_transfer(-50.0, -20.0, 4.0, 6.0, false);
    assert!((result - (-50.0)).abs() < 0.001);
}

#[test]
fn test_compressor_compression_region() {
    let result = compute_transfer(-10.0, -20.0, 4.0, 0.0, false);
    // threshold + (input - threshold) / ratio = -20 + 10/4 = -17.5
    assert!((result - (-17.5)).abs() < 0.001);
}

#[test]
fn test_compressor_knee_region() {
    let result = compute_transfer(-20.0, -20.0, 4.0, 6.0, false);
    assert!(result <= -20.0);
    assert!(result >= -25.0);
}

#[test]
fn test_all_themes_have_names() {
    for theme_id in ThemeId::all() {
        let name = theme_id.name();
        assert!(!name.is_empty(), "Theme {:?} has empty name", theme_id);
    }
}

#[test]
fn test_theme_count() {
    assert_eq!(ThemeId::all().len(), 9);
}

#[test]
fn test_theme_names_unique() {
    let names: Vec<_> = ThemeId::all().iter().map(|t| t.name()).collect();
    let mut unique_names = names.clone();
    unique_names.sort();
    unique_names.dedup();
    assert_eq!(
        names.len(),
        unique_names.len(),
        "Theme names must be unique"
    );
}

#[test]
fn test_builtin_themes_pass_core_contrast_validation() {
    for theme_id in ThemeId::all() {
        let theme = Theme::from_id(*theme_id);
        let validation = theme.validate_accessibility();
        assert!(
            validation.is_ok(),
            "{} should pass core contrast validation: {:?}",
            theme_id.name(),
            validation.err()
        );
    }
}

#[test]
fn test_theme_contrast_validation_rejects_unreadable_text() {
    let mut theme = Theme::dark();
    theme.text_primary = theme.background;

    let error = theme.validate_accessibility().unwrap_err();
    assert!(error.contains("text_primary/background"));
}

#[test]
fn test_light_app_theme_always_uses_studio_cream_for_plugins() {
    let app_theme = Theme::from_id(ThemeId::Light);
    for selected in PluginThemeId::all() {
        assert_eq!(
            plugin_theme_id_for_app_theme(*selected, &app_theme, ThemeId::Light),
            PluginThemeId::StudioCream
        );
    }
}

#[test]
fn test_dark_app_theme_uses_graphite_unless_brutalist_is_selected() {
    let app_theme = Theme::from_id(ThemeId::Dark);
    assert_eq!(
        plugin_theme_id_for_app_theme(PluginThemeId::Graphite, &app_theme, ThemeId::Dark),
        PluginThemeId::Graphite
    );
    assert_eq!(
        plugin_theme_id_for_app_theme(PluginThemeId::StudioCream, &app_theme, ThemeId::Dark),
        PluginThemeId::Graphite
    );
    assert_eq!(
        plugin_theme_id_for_app_theme(PluginThemeId::Brutalist, &app_theme, ThemeId::Dark),
        PluginThemeId::Brutalist
    );
}

#[test]
fn test_accessibility_palettes_use_the_high_contrast_plugin_chassis() {
    for theme_id in [
        ThemeId::BlackAndWhite,
        ThemeId::Protanopia,
        ThemeId::Deuteranopia,
        ThemeId::Tritanopia,
    ] {
        let app_theme = Theme::from_id(theme_id);
        assert_eq!(
            plugin_theme_id_for_app_theme(PluginThemeId::Graphite, &app_theme, theme_id),
            PluginThemeId::Brutalist
        );
    }
}

#[test]
fn test_all_languages_have_names() {
    for lang in Language::all() {
        let name = lang.name();
        assert!(!name.is_empty(), "Language {:?} has empty name", lang);
    }
}

#[test]
fn test_all_languages_have_codes() {
    for lang in Language::all() {
        let code = lang.code();
        assert_eq!(code.len(), 2, "Language {:?} should have 2-char code", lang);
    }
}

#[test]
fn test_language_count() {
    assert_eq!(Language::all().len(), 4);
}

#[test]
fn test_language_codes_unique() {
    let codes: Vec<_> = Language::all().iter().map(|l| l.code()).collect();
    let mut unique_codes = codes.clone();
    unique_codes.sort();
    unique_codes.dedup();
    assert_eq!(
        codes.len(),
        unique_codes.len(),
        "Language codes must be unique"
    );
}

#[test]
fn test_normalize_parameter_min() {
    assert!((normalize_parameter(-60.0, -60.0, 0.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_parameter_max() {
    assert!((normalize_parameter(0.0, -60.0, 0.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_normalize_parameter_mid() {
    assert!((normalize_parameter(-30.0, -60.0, 0.0) - 0.5).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_min() {
    assert!((denormalize_parameter(0.0, -60.0, 0.0) - (-60.0)).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_max() {
    assert!((denormalize_parameter(1.0, -60.0, 0.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_denormalize_parameter_mid() {
    assert!((denormalize_parameter(0.5, -60.0, 0.0) - (-30.0)).abs() < 0.001);
}

#[test]
fn test_normalize_denormalize_roundtrip() {
    let original = -25.0;
    let normalized = normalize_parameter(original, -60.0, 0.0);
    let denormalized = denormalize_parameter(normalized, -60.0, 0.0);
    assert!((denormalized - original).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_within_range() {
    assert!((clamp_parameter(-30.0, -60.0, 0.0) - (-30.0)).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_below_min() {
    assert!((clamp_parameter(-100.0, -60.0, 0.0) - (-60.0)).abs() < 0.001);
}

#[test]
fn test_clamp_parameter_above_max() {
    assert!((clamp_parameter(10.0, -60.0, 0.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_log_min() {
    assert!((normalize_parameter_log(20.0, 20.0, 20000.0) - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_log_max() {
    assert!((normalize_parameter_log(20000.0, 20.0, 20000.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_denormalize_log_min() {
    assert!((denormalize_parameter_log(0.0, 20.0, 20000.0) - 20.0).abs() < 0.1);
}

#[test]
fn test_denormalize_log_max() {
    assert!((denormalize_parameter_log(1.0, 20.0, 20000.0) - 20000.0).abs() < 1.0);
}

#[test]
fn test_log_normalize_denormalize_roundtrip() {
    let original = 1000.0;
    let normalized = normalize_parameter_log(original, 20.0, 20000.0);
    let denormalized = denormalize_parameter_log(normalized, 20.0, 20000.0);
    assert!(
        (denormalized - original).abs() < 1.0,
        "Expected ~{}, got {}",
        original,
        denormalized
    );
}

#[test]
fn test_screen_includes_library() {
    // Verify key screen variants exist and are usable
    let _now_playing = Screen::NowPlaying;
    let _library = Screen::Library;
    let _settings = Screen::Settings;
    let _plugins = Screen::PluginGraph;
    let _recording = Screen::Recording;
    let _headphone = Screen::HeadphoneEq;
    let _spinorama = Screen::Spinorama;
    let _room_eq = Screen::RoomEq;
    let _queue = Screen::Queue;
    let _spectrum = Screen::Spectrum;
    let _studio = Screen::Studio;
    let _playlists = Screen::Playlists;
}

#[test]
fn test_screen_equality() {
    assert_eq!(Screen::Library, Screen::Library);
    assert_ne!(Screen::Library, Screen::Settings);
}

#[test]
fn test_settings_tab_variants_exist() {
    let _library = SettingsTab::Library;
    let _theme = SettingsTab::Theme;
    let _language = SettingsTab::Language;
    let _keybindings = SettingsTab::Keybindings;
    let _audio = SettingsTab::AudioDevice;
    let _misc = SettingsTab::Misc;
    let _federation = SettingsTab::Federation;
    let _servers = SettingsTab::Servers;
    let _release = SettingsTab::ReleaseChannel;
}

#[test]
fn test_settings_tab_equality() {
    assert_eq!(SettingsTab::Library, SettingsTab::Library);
    assert_ne!(SettingsTab::Library, SettingsTab::Theme);
}

#[test]
fn test_sample_info_and_format_together() {
    let format = get_format_from_path(Path::new("/music/album/track.flac"));
    let sample_info = format_sample_info(Some(24), Some(96000));

    assert_eq!(format, Some("FLAC".to_string()));
    assert_eq!(sample_info, Some("24/96k".to_string()));

    let metadata = format!(
        "{} {}",
        format.unwrap_or_default(),
        sample_info.unwrap_or_default()
    );
    assert_eq!(metadata, "FLAC 24/96k");
}

#[test]
fn test_normalization_for_slider() {
    let min = -60.0;
    let max = 0.0;
    let default_db = -20.0;

    let normalized = normalize_parameter(default_db, min, max);
    assert!((normalized - 0.6666).abs() < 0.01);

    let user_position = 0.75;
    let new_value = denormalize_parameter(user_position, min, max);
    assert!((new_value - (-15.0)).abs() < 0.001);
}

#[test]
fn test_log_normalization_for_frequency() {
    let freq_1k = normalize_parameter_log(1000.0, 20.0, 20000.0);
    assert!(
        freq_1k > 0.4 && freq_1k < 0.7,
        "1kHz should be mid-range on log scale: {}",
        freq_1k
    );
}

#[test]
fn test_compressor_curve_full_range() {
    let threshold = -20.0;
    let ratio = 4.0;
    let knee = 6.0;

    let test_inputs = [-60.0, -40.0, -30.0, -20.0, -10.0, 0.0];

    for &input in &test_inputs {
        let output = compute_transfer(input, threshold, ratio, knee, false);
        assert!(
            output <= input + 0.001,
            "Compressor output {} should not exceed input {}",
            output,
            input
        );
        assert!(
            output.is_finite(),
            "Output must be finite for input {}",
            input
        );
    }
}

#[test]
fn test_theme_and_language_consistency() {
    assert!(
        ThemeId::all().len() >= 5,
        "Should have at least 5 theme options"
    );
    assert!(
        Language::all().len() >= 3,
        "Should have at least 3 language options"
    );
    assert!(Language::all().contains(&Language::English));
}

#[test]
fn test_dr_formatting_edge_cases() {
    // Rust uses banker's rounding (round half to even) for {:.0}
    assert_eq!(format_dr(Some(0.0)), Some("0".to_string()));
    assert_eq!(format_dr(Some(0.4)), Some("0".to_string()));
    assert_eq!(format_dr(Some(0.5)), Some("0".to_string())); // Banker's: 0.5 -> 0
    assert_eq!(format_dr(Some(1.5)), Some("2".to_string())); // Banker's: 1.5 -> 2
    assert_eq!(format_dr(Some(20.0)), Some("20".to_string()));
    assert_eq!(format_dr(None), None);
}

#[test]
fn test_default_eq_filter_values() {
    let filter = TestEQFilter::default_peak();
    assert_eq!(filter.filter_type, FilterType::Peak);
    assert!((filter.frequency - 1000.0).abs() < 0.001);
    assert!((filter.q - 1.0).abs() < 0.001);
    assert!((filter.gain_db - 0.0).abs() < 0.001);
}

#[test]
fn test_add_eq_band_to_empty_list() {
    let mut filters = Vec::new();
    assert_eq!(add_eq_band(&mut filters), 1);
    assert_eq!(filters[0].filter_type, FilterType::Peak);
}

#[test]
fn test_add_eq_band_to_existing_list() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::LowShelf, 100.0, 0.7, 3.0),
        TestEQFilter::new(FilterType::HighShelf, 10000.0, 0.7, -2.0),
    ];
    assert_eq!(add_eq_band(&mut filters), 3);
    assert_eq!(filters[2].filter_type, FilterType::Peak);
}

#[test]
fn test_add_multiple_eq_bands() {
    let mut filters = Vec::new();
    add_eq_band(&mut filters);
    add_eq_band(&mut filters);
    add_eq_band(&mut filters);
    assert_eq!(filters.len(), 3);
    for filter in &filters {
        assert_eq!(filter.filter_type, FilterType::Peak);
    }
}

#[test]
fn test_remove_eq_band_valid_index() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::LowShelf, 100.0, 0.7, 3.0),
        TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0),
        TestEQFilter::new(FilterType::HighShelf, 10000.0, 0.7, -2.0),
    ];
    assert_eq!(remove_eq_band(&mut filters, 1).unwrap(), 2);
    assert_eq!(filters[0].filter_type, FilterType::LowShelf);
    assert_eq!(filters[1].filter_type, FilterType::HighShelf);
}

#[test]
fn test_remove_eq_band_out_of_bounds() {
    let mut filters = vec![
        TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0),
        TestEQFilter::new(FilterType::Peak, 2000.0, 1.0, 0.0),
    ];
    let result = remove_eq_band(&mut filters, 5);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid band index"));
}

#[test]
fn test_remove_last_eq_band_fails() {
    let mut filters = vec![TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 0.0)];
    let result = remove_eq_band(&mut filters, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Cannot remove the last"));
}

#[test]
fn test_validate_eq_filter_valid() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 3.0)).is_ok());
}

#[test]
fn test_validate_eq_filter_frequency_too_low() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 10.0, 1.0, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_frequency_too_high() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 25000.0, 1.0, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_frequency_at_limits() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 20.0, 1.0, 0.0)).is_ok());
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 20000.0, 1.0, 0.0)).is_ok());
}

#[test]
fn test_validate_eq_filter_q_too_low() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 0.05, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_q_too_high() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 15.0, 0.0)).is_err());
}

#[test]
fn test_validate_eq_filter_q_at_limits() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 0.1, 0.0)).is_ok());
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 10.0, 0.0)).is_ok());
}

#[test]
fn test_validate_eq_filter_gain_too_low() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, -30.0)).is_err());
}

#[test]
fn test_validate_eq_filter_gain_too_high() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 30.0)).is_err());
}

#[test]
fn test_validate_eq_filter_gain_at_limits() {
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, -24.0)).is_ok());
    assert!(validate_eq_filter(&TestEQFilter::new(FilterType::Peak, 1000.0, 1.0, 24.0)).is_ok());
}

#[test]
fn test_eq_filter_types() {
    let types = [
        FilterType::Peak,
        FilterType::LowShelf,
        FilterType::HighShelf,
        FilterType::LowPass,
        FilterType::HighPass,
        FilterType::BandPass,
        FilterType::Notch,
    ];
    for filter_type in types {
        assert!(validate_eq_filter(&TestEQFilter::new(filter_type, 1000.0, 1.0, 0.0)).is_ok());
    }
}

#[test]
fn test_add_and_remove_eq_band_roundtrip() {
    let mut filters = vec![TestEQFilter::new(FilterType::Peak, 500.0, 1.5, 2.0)];
    add_eq_band(&mut filters);
    assert_eq!(filters.len(), 2);
    assert!(remove_eq_band(&mut filters, 1).is_ok());
    assert_eq!(filters.len(), 1);
    assert!((filters[0].frequency - 500.0).abs() < 0.001);
}

#[test]
fn test_library_state_set_search_query_clears_previous() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let mut state = LibraryState::new_for_test();

    state.set_search_query("first query".to_string());
    assert_eq!(state.search_query, "first query");

    state.set_search_query("second query".to_string());
    assert_eq!(state.search_query, "second query");
}

#[test]
fn test_library_state_set_search_query_empty() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let mut state = LibraryState::new_for_test();

    state.set_search_query("test".to_string());
    state.set_search_query("".to_string());
    assert_eq!(state.search_query, "");
}

#[test]
fn test_library_state_search_query_default_empty() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let state = LibraryState::new_for_test();
    assert_eq!(state.search_query, "");
}

#[test]
fn test_library_state_content_generation_tracks_content_invalidations() {
    use sotf_audio_player_gpui::app::state::library::LibraryState;
    let mut state = LibraryState::new_for_test();

    let initial = state.content_generation();
    state.set_search_query("view-only filter".to_string());
    assert_eq!(state.content_generation(), initial);

    state.invalidate_cache();
    assert_ne!(state.content_generation(), initial);
}

#[test]
fn test_input_mode_is_text_input_search() {
    assert!(InputMode::Search.is_text_input());
}

#[test]
fn test_input_mode_is_text_input_command_palette() {
    assert!(InputMode::CommandPalette.is_text_input());
}

#[test]
fn test_input_mode_is_text_input_normal() {
    assert!(!InputMode::Normal.is_text_input());
}

#[test]
fn test_input_mode_is_text_input_add_directory() {
    assert!(InputMode::AddDirectory.is_text_input());
}

#[test]
fn test_input_mode_is_text_input_editing_param() {
    // EditingParam uses stepper/knob interaction, not text input
    assert!(!InputMode::EditingParam.is_text_input());
}
#[test]
fn wizard_actions_name_the_destination_or_finish() {
    assert_eq!(
        wizard_continue_label(Some("Optimization")),
        "Continue to Optimization"
    );
    assert_eq!(wizard_continue_label(None), "Finish");
}

#[test]
fn guided_workflows_keep_navigation_in_the_shared_header() {
    let room_load = app_source("components/room_eq/step_1_load.rs");
    let headphone_measurement = app_source("components/headphone_eq/step_1_measurements.rs");

    assert!(!room_load.contains("next-from-load"));
    assert!(!headphone_measurement.contains("headphone-next-step"));
}

#[test]
fn plugin_choices_and_identity_are_explicit() {
    let layout_renderer = app_source("components/plugins/ui_layout_renderer/render.rs");
    let plugin_root = app_source("components/plugins/mod.rs");
    let plugin_shell = app_source("components/plugins/ui_plugin_shell.rs");

    assert!(!layout_renderer.contains("clicking advances to the next option"));
    assert!(
        layout_renderer.contains("ParamType::Choice { labels, .. } => render_param_as_button_set")
    );
    assert!(plugin_root.contains("ui_plugin_shell::render_plugin_shell"));
    assert!(plugin_shell.contains("plugin_description(plugin_type, text)"));
    assert!(plugin_shell.contains("(\"shell-bypass\", plugin_idx)"));
}

#[test]
fn every_screen_has_contextual_keyboard_help() {
    for screen in Screen::all() {
        for language in Language::all() {
            let bindings = get_keybindings_for_screen(*screen, *language, KeymapPreset::Default);
            assert!(
                !bindings.is_empty(),
                "{screen:?} has no contextual keyboard help for {}",
                language.code()
            );

            assert!(
                bindings.iter().all(|(keys, description)| {
                    !keys.contains("Click")
                        && !keys.contains("Drag")
                        && !description.trim().is_empty()
                }),
                "{screen:?} has invalid contextual keyboard help for {}",
                language.code()
            );
        }
    }
}

#[test]
fn contextual_keyboard_help_is_derived_from_runtime_bindings() {
    use sotf_audio_player_gpui::app::keybindings::{
        get_documented_keybindings_for_screen, get_keybindings,
    };

    let help_source = app_source("components/dialogs/misc.rs");
    assert!(help_source.contains("get_documented_keybindings_for_screen"));
    assert!(!help_source.contains("vec!["));
    assert!(!help_source.contains("match screen"));

    for preset in KeymapPreset::all() {
        let runtime = get_keybindings(*preset);
        let runtime_actions = runtime
            .iter()
            .map(|binding| binding.action().name())
            .collect::<std::collections::BTreeSet<_>>();
        let runtime_key_specs = runtime
            .iter()
            .map(|binding| {
                binding
                    .keystrokes()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<std::collections::BTreeSet<_>>();

        for screen in Screen::all() {
            for binding in get_documented_keybindings_for_screen(*screen, *preset) {
                let action_name = binding.action_name.unwrap_or_else(|| {
                    panic!(
                        "{preset:?} {screen:?} help row '{}' has no runtime action",
                        binding.description
                    )
                });
                assert!(
                    runtime_actions.contains(action_name),
                    "{preset:?} {screen:?} help row '{}' references unregistered action {action_name}",
                    binding.description
                );
                assert!(
                    runtime_key_specs.contains(&binding.raw_key_spec),
                    "{preset:?} {screen:?} help row '{}' has non-runtime key spec '{}'",
                    binding.description,
                    binding.raw_key_spec
                );
            }
        }
    }
}

#[test]
fn documented_keybindings_are_localized_from_the_registry() {
    use sotf_audio_player_gpui::app::keybindings::{
        KeybindingCategory, KeymapPreset, get_documented_keybindings,
        get_documented_keybindings_for_screen,
    };
    use sotf_audio_player_gpui::i18n::KeybindingTranslations;

    for language in Language::all() {
        let text = KeybindingTranslations::for_language(*language);
        for category in KeybindingCategory::all() {
            assert!(!text.category_name(*category).trim().is_empty());
        }
        for preset in KeymapPreset::all() {
            assert!(!text.preset_name(*preset).trim().is_empty());
            assert!(!text.preset_description(*preset).trim().is_empty());
            for binding in get_documented_keybindings(*preset) {
                assert!(
                    !text
                        .action_description(binding.description)
                        .trim()
                        .is_empty(),
                    "missing {:?} action copy for {}",
                    preset,
                    language.code()
                );
            }
            for screen in Screen::all() {
                for binding in get_documented_keybindings_for_screen(*screen, *preset) {
                    assert!(
                        !text
                            .action_description(binding.description)
                            .trim()
                            .is_empty(),
                        "missing {:?} {:?} screen-help copy for {}",
                        preset,
                        screen,
                        language.code()
                    );
                }
            }
        }
    }

    let common = app_source("app/keybindings/common.rs");
    for binding in [
        r#"KeyBinding::new("shift-l", actions::SwitchToLibrary"#,
        r#"KeyBinding::new("shift-q", actions::SwitchToQueue"#,
        r#"KeyBinding::new("shift-p", actions::SwitchToStudio"#,
        r#"KeyBinding::new("shift-o", actions::SwitchToDevices"#,
    ] {
        assert!(
            common.contains(binding),
            "documented screen jump is not registered: {binding}"
        );
    }
    assert!(common.contains(r#""shift-d""#));
    assert!(common.contains("actions::SwitchToDirectories"));
    assert!(
        app_source("app/keybindings/mod.rs")
            .contains(r#"KeyBinding::new("f1", actions::ToggleScreenGuide"#)
    );
}

#[test]
fn keymap_presets_have_no_duplicate_runtime_dispatches_per_context() {
    use sotf_audio_player_gpui::app::keybindings::{
        KeymapPreset, get_keybindings, validate_keybinding_registry,
    };

    for preset in KeymapPreset::all() {
        validate_keybinding_registry(*preset).unwrap();
        let mut seen = BTreeMap::new();
        for binding in get_keybindings(*preset) {
            let keys = binding
                .keystrokes()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            let context = format!("{:?}", binding.predicate());
            let action = binding.action().name();
            let identity = (context, keys);
            assert!(
                seen.insert(identity.clone(), action).is_none(),
                "{preset:?} registers more than one command for {identity:?}"
            );
        }
    }
}

#[test]
fn command_palette_rows_are_localized_searchable_runtime_actions() {
    use sotf_audio_player_gpui::app::keybindings::{
        command_palette_action, search_command_palette_commands,
    };
    use sotf_audio_player_gpui::i18n::KeybindingTranslations;

    for preset in KeymapPreset::all() {
        let english = KeybindingTranslations::for_language(Language::English);
        let commands = search_command_palette_commands(
            *preset,
            "",
            |action| english.action_description(action),
            |category| english.category_name(category),
        );
        assert!(!commands.is_empty(), "{preset:?} has no palette commands");

        for command in &commands {
            let action = command_palette_action(*preset, command.action_name)
                .unwrap_or_else(|| panic!("{preset:?} cannot resolve {}", command.action_name));
            assert_eq!(action.name(), command.action_name);
        }

        let palette_rows = commands
            .iter()
            .filter(|command| command.description == "Command palette")
            .collect::<Vec<_>>();
        assert_eq!(
            palette_rows.len(),
            1,
            "{preset:?} must expose one command-palette toggle"
        );
        let toggle = palette_rows[0];
        assert!(toggle.action_name.ends_with("ToggleCommandPalette"));

        let shortcut_matches = search_command_palette_commands(
            *preset,
            &toggle.key,
            |action| english.action_description(action),
            |category| english.category_name(category),
        );
        assert!(
            shortcut_matches
                .iter()
                .any(|command| command.action_name == toggle.action_name),
            "{preset:?} shortcut search did not find the palette toggle"
        );
    }

    let french = KeybindingTranslations::for_language(Language::French);
    let localized_description = search_command_palette_commands(
        KeymapPreset::Default,
        "palette de commandes",
        |action| french.action_description(action),
        |category| french.category_name(category),
    );
    assert!(localized_description.iter().any(|command| {
        command.description == "Palette de commandes"
            && command.action_name.ends_with("ToggleCommandPalette")
    }));

    let localized_category = search_command_palette_commands(
        KeymapPreset::Default,
        "tests d’écoute",
        |action| french.action_description(action),
        |category| french.category_name(category),
    );
    assert!(
        localized_category
            .iter()
            .any(|command| command.category == "Tests d’écoute")
    );
}

#[test]
fn every_screen_has_an_owned_state_and_responsive_checklist() {
    #[derive(Clone, Copy)]
    enum Coverage {
        Owned(&'static str),
        NotApplicable(&'static str),
    }

    struct Checklist {
        screen: Screen,
        empty: Coverage,
        loading: Coverage,
        error: Coverage,
    }

    use Coverage::{NotApplicable as Na, Owned};
    let checklists = [
        Checklist {
            screen: Screen::Home,
            empty: Owned("components/home/home_screen/misc.rs"),
            loading: Owned("components/home/library/types.rs"),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::HomeShelf,
            empty: Owned("components/home/home_screen/misc.rs"),
            loading: Owned("components/home/library/types.rs"),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::NowPlaying,
            empty: Owned("components/home/queue/misc.rs"),
            loading: Na("Playback is push-driven and has no screen-owned loading phase."),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::Library,
            empty: Owned("components/home/library/types.rs"),
            loading: Owned("components/home/library/types.rs"),
            error: Owned("components/settings/library/misc.rs"),
        },
        Checklist {
            screen: Screen::Streams,
            empty: Owned("components/streams.rs"),
            loading: Na("Saved streams are local and synchronously available."),
            error: Owned("components/streams.rs"),
        },
        Checklist {
            screen: Screen::Queue,
            empty: Owned("components/home/queue/misc.rs"),
            loading: Na("The queue is local and has no loading phase."),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::Playlists,
            empty: Owned("ui/phone.rs"),
            loading: Na("Playlist navigation is local and has no loading phase."),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::Spectrum,
            empty: Owned("components/plugins/ui_spectrum.rs"),
            loading: Na("Spectrum frames stream continuously without a loading phase."),
            error: Na("No recoverable screen-local error state exists for absent analyzer data."),
        },
        Checklist {
            screen: Screen::Settings,
            empty: Na("Settings always expose at least one configuration section."),
            loading: Owned("components/settings/servers/misc.rs"),
            error: Owned("components/settings/servers/misc.rs"),
        },
        Checklist {
            screen: Screen::SettingsDetail,
            empty: Na("Settings detail always owns a selected configuration section."),
            loading: Owned("components/settings/servers/misc.rs"),
            error: Owned("components/settings/servers/misc.rs"),
        },
        Checklist {
            screen: Screen::StudioHub,
            empty: Owned("components/plugins/mod.rs"),
            loading: Na("The local plugin graph has no loading phase."),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::EqCurve,
            empty: Owned("components/plugins/mod.rs"),
            loading: Na("The local EQ editor has no loading phase."),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::Studio,
            empty: Owned("components/plugins/mod.rs"),
            loading: Na("The local plugin rack has no loading phase."),
            error: Owned("ui/render.rs"),
        },
        Checklist {
            screen: Screen::Recording,
            empty: Owned("components/recording/config/misc.rs"),
            loading: Owned("components/recording/capture/misc.rs"),
            error: Owned("components/recording/mod.rs"),
        },
        Checklist {
            screen: Screen::RoomEq,
            empty: Owned("components/room_eq/step_1_load.rs"),
            loading: Owned("components/room_eq/step_4_optimise/room.rs"),
            error: Owned("components/room_eq/step_1_load.rs"),
        },
        Checklist {
            screen: Screen::HeadphoneEq,
            empty: Owned("components/headphone_eq/step_1_measurements.rs"),
            loading: Owned("components/headphone_eq/step_1_measurements.rs"),
            error: Owned("components/headphone_eq/step_1_measurements.rs"),
        },
        Checklist {
            screen: Screen::Spinorama,
            empty: Owned("components/spinorama_eq/step_1_select/misc.rs"),
            loading: Owned("components/spinorama_eq/step_1_select/misc.rs"),
            error: Owned("components/spinorama_eq/step_1_select/misc.rs"),
        },
        Checklist {
            screen: Screen::PluginGraph,
            empty: Owned("components/plugins/ui_graph/player_view.rs"),
            loading: Na("The local workflow graph has no loading phase."),
            error: Owned("components/plugins/ui_graph/keyboard.rs"),
        },
        Checklist {
            screen: Screen::ListeningTest,
            empty: Owned("components/listening_test.rs"),
            loading: Na("Offline preparation reports progress as an operation, not page loading."),
            error: Owned("components/listening_test.rs"),
        },
    ];

    let covered = checklists
        .iter()
        .map(|checklist| format!("{:?}", checklist.screen))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        covered,
        Screen::all()
            .iter()
            .map(|screen| format!("{screen:?}"))
            .collect(),
        "screen owner checklist drifted from Screen::all()"
    );
    for checklist in checklists {
        for (state_name, coverage) in [
            (
                "normal",
                Owned("tests/e2e/scenarios/player/screen_matrix.rs"),
            ),
            ("empty", checklist.empty),
            ("loading", checklist.loading),
            ("error", checklist.error),
            (
                "narrow",
                Owned("tests/e2e/scenarios/player/screen_matrix.rs"),
            ),
            ("wide", Owned("tests/e2e/scenarios/player/screen_matrix.rs")),
        ] {
            match coverage {
                Owned(path) => assert!(
                    !app_source(path).trim().is_empty(),
                    "{:?} {state_name} owner is empty: {path}",
                    checklist.screen
                ),
                Na(reason) => assert!(
                    reason.split_whitespace().count() >= 6,
                    "{:?} {state_name} N/A needs a concrete reason",
                    checklist.screen
                ),
            }
        }
    }
}

#[test]
fn autoeq_form_translations_are_complete_and_used_by_all_consumers() {
    let mut localized_presets = BTreeSet::new();

    for language in Language::all() {
        let text = AutoEqFormTranslations::for_language(*language);
        localized_presets.insert(text.preset);

        let outer = [
            text.preset,
            text.filter_design,
            text.optimization_quality,
            text.home_cinema,
            text.delay_correction,
            text.target,
            text.optimization_goal,
            text.capability,
            text.strategy,
            text.per_measurement_weights,
            text.reference_channel,
            text.min_frequency_hz,
            text.max_frequency_hz,
            text.max_delay_ms,
            text.primary_seat,
            text.max_deviation_db,
            text.slope_db_oct,
            text.reference_frequency_hz,
            text.bass_boost_db,
            text.shelf_frequency_hz,
            text.system_type,
            text.optimization_mode,
            text.target_curve,
            text.room_configuration,
            text.optimizer_configuration,
        ];
        let params = [
            text.parameters.loss_function,
            text.parameters.number_filters,
            text.parameters.filter_type,
            text.parameters.variance_lambda,
            text.parameters.iir_parameters,
            text.parameters.sample_rate_hz,
            text.parameters.min_q,
            text.parameters.max_q,
            text.parameters.min_db,
            text.parameters.max_db,
            text.parameters.spacing_weight,
            text.parameters.min_spacing_oct,
            text.parameters.fir_parameters,
            text.parameters.regularization,
            text.parameters.crossover_configuration,
            text.parameters.crossover_frequency_hz,
            text.parameters.crossover_type,
            text.parameters.fir_band,
            text.parameters.bass_management,
            text.parameters.manual_f3_hz,
            text.parameters.order,
            text.parameters.safety_margin_oct,
            text.parameters.split_frequency_hz,
            text.parameters.lf_max_q,
            text.parameters.hf_max_q,
            text.parameters.smoothing,
            text.parameters.weights,
            text.parameters.smoothing_resolution,
            text.parameters.smooth_window_oct,
            text.parameters.seed,
        ];
        let blocks = [
            text.blocks.fir_taps,
            text.blocks.phase,
            text.blocks.peq_model,
            text.blocks.algorithm,
            text.blocks.population,
            text.blocks.max_evaluations,
            text.blocks.tolerance,
            text.blocks.absolute_tolerance,
            text.blocks.bo_initial,
            text.blocks.bo_batch,
            text.blocks.bo_std_stop,
            text.blocks.bo_acquisition,
            text.blocks.de_strategy,
            text.blocks.mutation,
            text.blocks.recombination,
            text.blocks.adaptive_weight_f,
            text.blocks.adaptive_weight_cr,
            text.blocks.local_refinement,
            text.blocks.local_algorithm,
        ];
        let sections = [
            text.sections.recommended,
            text.sections.processing,
            text.sections.edit_custom_target_curve,
            text.sections.flat_loss_description,
            text.sections.epa_loss_description,
        ];
        assert!(
            outer
                .into_iter()
                .chain(params)
                .chain(blocks)
                .chain(sections)
                .all(|value| !value.trim().is_empty()),
            "AutoEQ form translation is incomplete for {}",
            language.code()
        );
    }
    assert_eq!(localized_presets.len(), Language::all().len());

    for path in [
        "components/autoeq/render_body_simple.rs",
        "components/autoeq/render_body_room_eq.rs",
        "components/autoeq/render_section_algorithm.rs",
        "components/autoeq/render_section_capability.rs",
        "components/autoeq/render_section_delay.rs",
        "components/autoeq/render_section_filter_design.rs",
        "components/autoeq/render_section_goals.rs",
        "components/autoeq/render_section_home_cinema.rs",
        "components/autoeq/render_section_multi_measurement.rs",
        "components/autoeq/render_section_optimization_goal.rs",
        "components/autoeq/render_section_target.rs",
        "components/autoeq/render_block_eq_design.rs",
        "components/autoeq/render_block_eq_iir_filters.rs",
        "components/autoeq/render_block_eq_mixed.rs",
        "components/autoeq/render_block_eq_fir.rs",
        "components/autoeq/render_block_optimizer.rs",
    ] {
        let source = app_source(path).replace("Text::new(\"qEHVI\")", "");
        for literal_surface in [
            "Text::new(\"",
            "Text::label(\"",
            "Text::section_header(\"",
            ".label(\"",
        ] {
            assert!(
                !source.contains(literal_surface),
                "{path} contains untranslated visible text via {literal_surface}"
            );
        }
    }

    for path in [
        "components/headphone_eq/step_2_optimisation/misc.rs",
        "components/room_eq/step_3_configure/misc.rs",
        "components/spinorama_eq/step_2_configure/misc.rs",
    ] {
        assert!(
            app_source(path).contains(".language(state.app.ui_state.language)"),
            "{path} does not pass the selected language into AutoEqForm"
        );
    }
}

#[test]
fn headphone_eq_translations_are_complete_and_visible_copy_is_extracted() {
    let mut localized_titles = BTreeSet::new();
    for language in Language::all() {
        let text = HeadphoneEqTranslations::for_language(*language);
        localized_titles.insert(text.title);
        assert!(
            [
                text.title,
                text.measurement_step,
                text.optimization_step,
                text.listen_step,
                text.close,
                text.back,
                text.select_measurement,
                text.select_measurement_description,
                text.measurement_file,
                text.measurement_file_description,
                text.headphone_search,
                text.search_placeholder,
                text.available_headphones,
                text.frequency_response,
                text.configure_optimization,
                text.configure_optimization_description,
                text.custom_target_curve,
                text.generate_headphone_eq,
                text.listen_preview,
                text.listen_preview_description,
                text.optimization_results,
                text.response_visualization,
                text.eq_filters,
                text.no_results,
                text.no_results_description,
                text.apply_export,
                text.apply_export_description,
                text.export,
                text.export_description,
                text.no_target_curve,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "Headphone EQ copy is incomplete for {}",
            language.code()
        );
    }
    assert_eq!(localized_titles.len(), Language::all().len());

    for path in [
        "components/headphone_eq/mod.rs",
        "components/headphone_eq/step_1_measurements.rs",
        "components/headphone_eq/step_2_optimisation/misc.rs",
        "components/headphone_eq/step_3_listen.rs",
        "components/headphone_eq/step_4_export.rs",
    ] {
        let source = app_source(path);
        for literal_surface in [
            "Text::new(\"",
            "Text::label(\"",
            "Text::section_header(\"",
            ".label(\"",
            ".placeholder(\"",
            ".title(\"",
        ] {
            assert!(
                !source.contains(literal_surface),
                "{path} contains untranslated visible text via {literal_surface}"
            );
        }
    }
}

#[test]
fn stream_translations_are_complete_and_visible_copy_is_extracted() {
    let mut localized_empty_states = BTreeSet::new();
    for language in Language::all() {
        let text = StreamsTranslations::for_language(*language);
        localized_empty_states.insert(text.no_saved_streams);
        assert!(
            [
                text.title,
                text.subtitle,
                text.no_saved_streams,
                text.name,
                text.seekable,
                text.url_placeholder,
                text.save,
                text.play,
                text.queue,
                text.remove,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "stream copy is incomplete for {}",
            language.code()
        );
    }
    assert_eq!(localized_empty_states.len(), Language::all().len());

    let source = app_source("components/streams.rs");
    assert!(
        !source.contains("Text::new(\"")
            && !source.contains(".child(\"")
            && !source.contains(".label(\"")
            && !source
                .replace(".placeholder(\"mp3\")", "")
                .contains(".placeholder(\"")
            && !source.contains(", \"Save\")")
            && !source.contains(", \"Play\")")
            && !source.contains(", \"Queue\")"),
        "Streams contains first-party visible literals outside its technical format placeholder"
    );
}

#[test]
fn plugin_graph_keyboard_copy_is_complete_and_handler_is_wired() {
    let mut localized_titles = BTreeSet::new();
    for language in Language::all() {
        let text = PluginGraphTranslations::for_language(*language);
        localized_titles.insert(text.keyboard_editor);
        assert!(
            [
                text.keyboard_editor,
                text.selected,
                text.none_selected,
                text.add_plugin,
                text.connect_source,
                text.no_connect_source,
                text.keyboard_hint,
                text.special_node_read_only,
                text.select_node_first,
                text.connection_created,
                text.connection_failed,
                text.disconnected,
                text.plugin_added,
                text.node_removed,
                text.input_sources,
                text.player_audio_files,
                text.output_devices,
                text.no_output_devices,
                text.dynamics,
                text.spatial,
                text.monitor,
                text.denoising,
                text.utility,
                text.edit_parameters,
                text.bypass_activate,
                text.solo,
                text.remove,
                text.nodes.signal,
                text.nodes.source,
                text.nodes.reset_view,
                text.nodes.input,
                text.nodes.plugins,
                text.nodes.load,
                text.nodes.save,
                text.nodes.close,
                text.nodes.parametric_eq,
                text.nodes.no_plugin_data,
                text.nodes.gain,
                text.nodes.audio_player,
                text.nodes.audio_player_description,
                text.nodes.output_device,
                text.nodes.output_device_description,
                text.nodes.input_device,
                text.nodes.input_device_description,
                text.nodes.unknown_node_type,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "Plugin Graph keyboard copy is incomplete for {}",
            language.code()
        );
    }
    assert_eq!(localized_titles.len(), Language::all().len());

    let keyboard = app_source("components/plugins/ui_graph/keyboard.rs");
    for key in [
        "\"tab\"",
        "\"[\" | \"]\"",
        "\"a\"",
        "\"enter\" | \"e\"",
        "\"b\"",
        "\"c\"",
        "\"x\"",
        "\"delete\" | \"backspace\"",
        "\"left\" | \"right\" | \"up\" | \"down\"",
    ] {
        assert!(keyboard.contains(key), "Plugin Graph does not handle {key}");
    }
    assert!(keyboard.contains("add_plugin_node"));
    assert!(keyboard.contains("add_connection"));
    assert!(keyboard.contains("connections"));
    assert!(keyboard.contains("remove_node"));
    assert!(keyboard.contains("toggle_plugin"));
    assert!(keyboard.contains("editing_graph_node_uuid"));

    let render = app_source("ui/render.rs");
    assert!(keyboard.contains("macro_rules! graph_action_handler"));
    for handler in [
        "graph_select_next_node",
        "graph_select_previous_node",
        "graph_select_next_plugin_type",
        "graph_select_previous_plugin_type",
        "graph_select_next_port",
        "graph_select_previous_port",
        "graph_add_selected_plugin",
        "graph_edit_selected_node",
        "graph_toggle_selected_bypass",
        "graph_connect_selected_node",
        "graph_disconnect_selected_node",
        "graph_remove_selected_node",
        "graph_move_selected_left",
        "graph_move_selected_right",
        "graph_move_selected_up",
        "graph_move_selected_down",
        "graph_move_selected_left_large",
        "graph_move_selected_right_large",
        "graph_move_selected_up_large",
        "graph_move_selected_down_large",
    ] {
        assert!(
            keyboard.contains(handler),
            "Plugin Graph action handler {handler} is not implemented"
        );
        assert!(
            render.contains(&format!("Self::{handler}")),
            "Plugin Graph action handler {handler} is not registered"
        );
    }
    assert!(
        !render.contains("view.handle_plugin_graph_keyboard(event, cx)"),
        "Plugin Graph raw key dispatcher must not bypass registered GPUI actions"
    );
    let graph =
        app_source("components/plugins/ui_graph/player_view.rs").replace(".child(\"!\")", "");
    assert!(graph.contains("self.render_graph_keyboard_bar(cx)"));
    for literal_surface in [
        "Text::new(\"",
        "Text::label(\"",
        "Text::section_header(\"",
        ".label(\"",
        ".placeholder(\"",
        ".title(\"",
        ".child(\"",
    ] {
        assert!(
            !graph.contains(literal_surface),
            "Plugin Graph contains untranslated visible text via {literal_surface}"
        );
    }
}

#[test]
fn plugin_rack_copy_is_complete_and_visible_literals_are_extracted() {
    use sotf_audio_player_gpui::i18n::{PluginCommonTranslations, PluginRackTranslations};

    let mut localized_titles = BTreeSet::new();
    for language in Language::all() {
        let text = PluginRackTranslations::for_language(*language);
        localized_titles.insert(text.signal_chain);
        assert!(
            [
                text.signal_chain,
                text.graph_routing_title,
                text.graph_routing_description,
                text.open_graph_view,
                text.preset_name_placeholder,
                text.ok,
                text.save_new,
                text.empty_rack,
                text.load,
                text.save,
                text.applying,
                text.no_plugin_selected,
                text.add_plugin_to_start,
                text.configuration,
                text.view,
                text.skin,
                text.output,
                text.binaural_preview,
                text.plugin_presets,
                text.plugin_configuration,
                text.native_ui,
                text.simple,
                text.add_plugin_for_comparison,
                text.move_plugin_up,
                text.move_plugin_down,
                text.remove_plugin,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "Plugin Rack copy is incomplete for {}",
            language.code()
        );
        let common = PluginCommonTranslations::for_language(*language);
        assert!(
            PluginType::all()
                .iter()
                .all(|plugin_type| !common.description(plugin_type).trim().is_empty()),
            "plugin descriptions are incomplete for {}",
            language.code()
        );
        assert!(
            [
                "SETUP",
                "CHANNELS",
                "GLOBAL",
                "DYNAMICS",
                "TIMING",
                "OUTPUT",
                "Primary",
                "Spatial",
                "Output",
                "LFE & Bass",
                "SubHarmonic",
                "Dialogue",
                "Ambient",
                "Height",
                "HR Direct",
                "Decorrelation",
                "Analysis",
                "Diagnostic",
                "Source Extraction",
                "Bands",
                "Thresh",
                "Ratio",
                "Attack",
                "Release",
                "Mix",
                "Freq",
                "Gain",
                "Active",
                "Solo",
                "Q",
                "Mono",
                "Left",
                "Right",
                "Ch",
                "Enabled",
                "Dim Gain",
                "Fade Time",
                "Knee",
                "Link",
                "Bins",
                "Min Hz",
                "Max Hz",
                "Smooth",
                "AutoGain",
                "Bypass",
                "Link Amt",
                "Link Ch",
                "Lookahead",
                "M/S Mode",
                "Preset",
                "SC Tilt",
                "XOver 1",
                "XOver 2",
                "XOver 3",
                "XOver 4",
                "AG Max",
                "AG Smooth",
                "Amb Boost",
                "Bandpass",
                "Bleed",
                "Boost",
                "Centroid",
                "Coherence",
                "Density",
                "Dir Leak",
                "Duration",
                "HF Cap",
                "LFE Cut",
                "LFE Gain",
                "LFO Rate",
                "Rear Boost",
                "Reflection",
                "Safety",
                "Sharpen",
                "Threshold",
                "Top Gain",
                "Trans Red",
                "Variance",
                "Voice Hi",
                "Voice Lo",
                "Weight",
            ]
            .iter()
            .all(|label| common.has_localized_label(label)),
            "plugin labels are incomplete for {}",
            language.code()
        );
    }
    assert_eq!(localized_titles.len(), Language::all().len());

    let source = app_source("components/plugins/ui_rack/plugin.rs")
        .replace(".child(\"S\")", "")
        .replace(".child(\"P\")", "")
        .replace(".child(\"🔒\")", "")
        .replace(".child(\":::\")", "")
        .replace(".child(\"+\")", "")
        .replace(".child(\"|\")", "")
        .replace(".child(\"X\")", "")
        .replace(".label(\"OUT\")", "");
    for literal_surface in [
        "Text::new(\"",
        "Text::label(\"",
        "Text::section_header(\"",
        ".label(\"",
        ".placeholder(\"",
        ".title(\"",
        ".child(\"",
    ] {
        assert!(
            !source.contains(literal_surface),
            "Plugin Rack contains untranslated visible text via {literal_surface}"
        );
    }
}

#[test]
fn every_upmixer_plugin_label_literal_has_a_translation() {
    use sotf_audio_player_gpui::i18n::PluginCommonTranslations;

    let source = app_source("components/plugins/ui_upmixer/render.rs");
    let file = syn::parse_file(&source).expect("upmixer source should parse as Rust");
    let mut visitor = TextLabelVisitor::default();
    visitor.visit_file(&file);
    let labels = visitor.labels;

    assert!(
        !labels.is_empty(),
        "upmixer should contain localized labels"
    );
    let missing_by_language = Language::all()
        .iter()
        .copied()
        .filter(|language| *language != Language::English)
        .filter_map(|language| {
            let common = PluginCommonTranslations::for_language(language);
            let missing = labels
                .iter()
                .filter(|label| !common.has_localized_label(label))
                .cloned()
                .collect::<Vec<_>>();
            (!missing.is_empty()).then_some((language.code(), missing))
        })
        .collect::<Vec<_>>();
    assert!(
        missing_by_language.is_empty(),
        "missing upmixer plugin labels: {missing_by_language:?}"
    );
}

#[test]
fn dialog_server_and_phone_copy_is_complete_and_direct_literals_are_extracted() {
    use sotf_audio_player_gpui::i18n::{
        AppearanceTranslations, AudioDeviceTranslations, ContextMenuTranslations,
        DialogTranslations, EqDiscoveryTranslations, FederationTranslations,
        FileDialogTranslations, FooterTranslations, MetadataEditorTranslations, PhoneTranslations,
        PlaybackApplyTranslations, RecordingWorkflowTranslations, RoomEqWorkflowTranslations,
        ServerSettingsTranslations, SettingsSurfaceTranslations, SpectrumTranslations,
        TutorialTranslations, WorkflowTranslations,
    };

    for language in Language::all() {
        let dialog = DialogTranslations::for_language(*language);
        assert!(
            [
                dialog.global_keybindings,
                dialog.jump_to_screens,
                dialog.increase_volume,
                dialog.decrease_volume,
                dialog.show_keyboard_shortcuts,
                dialog.show_help_support,
                dialog.about.about_title,
                dialog.about.app_name,
                dialog.about.github_repository,
                dialog.about.source_code_and_docs,
                dialog.about.report_issues,
                dialog.about.bug_tracker,
                dialog.about.feature_requests,
                dialog.about.github_discussions,
                dialog.about.community_forum,
                dialog.about.audio_science_review,
                dialog.about.license_gpl,
                dialog.about.open_source_license,
                dialog.about.press_escape_to_close,
                dialog.about.close,
                dialog.about.help_support_title,
                dialog.about.request_new_features,
                dialog.about.share_feature_ideas,
                dialog.about.report_bugs,
                dialog.help_fix_issues,
                dialog.view_source_and_docs,
                dialog.press_escape_or_help_to_close,
                dialog.press_escape_or_question_to_close,
                dialog.empty_library_welcome,
                dialog.empty_library_title,
                dialog.empty_library_description,
                dialog.not_now,
                dialog.add_music_folders,
                dialog.add_remote_source,
                dialog.edit_metadata,
                dialog.preview,
                dialog.search_musicbrainz,
                dialog.load_apo,
                dialog.load_sofa,
                dialog.save_plugin_preset,
                dialog.load_plugin_preset,
                dialog.keyboard_shortcuts,
                dialog.channel_conflict,
                dialog.dont_show_again,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "dialog copy is incomplete for {}",
            language.code()
        );
        assert!(
            Screen::all()
                .iter()
                .all(|screen| !dialog.screen_name(*screen).trim().is_empty())
        );

        let metadata = MetadataEditorTranslations::for_language(*language);
        assert!(
            [
                metadata.fields.title,
                metadata.fields.artist,
                metadata.fields.album_artist,
                metadata.fields.year,
                metadata.fields.year_placeholder,
                metadata.fields.genre,
                metadata.fields.composer,
                metadata.fields.disc,
                metadata.fields.track,
                metadata.fields.conductor,
                metadata.fields.performer,
                metadata.fields.isrc,
                metadata.fields.ensemble,
                metadata.fields.edition,
                metadata.target,
                metadata.preview,
                metadata.search_musicbrainz,
                metadata.searching_musicbrainz,
                metadata.untitled,
                metadata.unknown,
                metadata.tag_writing_unsupported,
                metadata.preview_before_applying,
                metadata.unsupported_before_apply,
                metadata.backups_created,
                metadata.cancel,
                metadata.apply_changes,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "metadata-editor copy is incomplete for {}",
            language.code()
        );
        assert!(metadata.target_label("Album").contains("Album"));
        assert!(metadata.preview_summary(3, 1, true).contains('3'));

        let file_dialog = FileDialogTranslations::for_language(*language);
        assert!(
            [
                file_dialog.press_escape_to_skip,
                file_dialog.enter_apo_path,
                file_dialog.enter_sofa_path,
                file_dialog.load_or_cancel,
                file_dialog.existing_presets,
                file_dialog.save_name_or_overwrite,
                file_dialog.save_hint,
                file_dialog.preset_name_or_select,
                file_dialog.available_presets,
                file_dialog.no_presets_found,
                file_dialog.load_hint,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );

        let workflow = WorkflowTranslations::for_language(*language);
        assert!(
            [
                workflow.success,
                workflow.failed,
                workflow.loading,
                workflow.loading_versions,
                workflow.no_versions_available,
                workflow.phase_data,
                workflow.downloading_measurement,
                workflow.signal_recording,
                workflow.audio_device_configuration,
                workflow.evaluate_recordings,
                workflow.empty_pass_through,
                workflow.optimizing,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );
        assert!(workflow.progress(42.0).contains("42"));
        assert!(workflow.iteration_loss(3, 0.25).contains('3'));

        let room_eq = RoomEqWorkflowTranslations::for_language(*language);
        assert!(
            [
                room_eq.load_measurement_description,
                room_eq.no_channels_configured,
                room_eq.delay_optimizer_help,
                room_eq.negligible_delay_warning,
                room_eq.delay_probe_help,
                room_eq.no_optimization_results,
                room_eq.choose_configuration,
                room_eq.simple_mode_description,
                room_eq.full_mode_description,
                room_eq.single,
                room_eq.multi_driver,
                room_eq.dismiss,
                room_eq.load_from_recording,
                room_eq.go_to_recording,
                room_eq.import_from_file,
                room_eq.reset_to_defaults,
                room_eq.select,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );

        let discovery = EqDiscoveryTranslations::for_language(*language);
        assert!(
            [
                discovery.speaker_search_description,
                discovery.loading_speakers,
                discovery.spinorama_after_select,
                discovery.headphone_search_description,
                discovery.loading_headphones,
                discovery.custom_target_help,
                discovery.spinorama_iir_only,
                discovery.search_speakers,
                discovery.search_headphones,
                discovery.save_eq_file,
                discovery.cancel,
                discovery.generate_speaker_eq,
                discovery.generate_headphone_eq,
                discovery.browse,
                discovery.refresh,
                discovery.load_from_file,
                discovery.download_from_spinorama,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );

        let recording = RecordingWorkflowTranslations::for_language(*language);
        assert!(
            [
                recording.sweep_frequency_range_only,
                recording.no_channels_configured,
                recording.spl_calibration_instructions,
                recording.timestamped_subdirectory,
                recording.bass_precision_description,
                recording.bass_anchor_description,
                recording.probe_description,
                recording.no_probe_captured,
                recording.cancel,
                recording.add_speaker,
                recording.remove_speaker,
                recording.add,
                recording.playback_device,
                recording.recording_device,
                recording.microphone_calibration,
                recording.output_directory,
                recording.advanced_measurement_quality,
                recording.single,
                recording.multi,
                recording.no_recordings_available,
                recording.go_back_to_capture,
                recording.all_channels,
                recording.run_bass_anchor,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );
        assert!(recording.seconds(10).contains("10"));

        let context = ContextMenuTranslations::for_language(*language);
        assert!(
            [
                context.suspend_incompatible_and_play,
                context.remove_incompatible_and_play,
                context.cancel_playback,
                context.play_now,
                context.add_to_queue,
                context.edit_metadata,
                context.play_from_here,
                context.remove_from_queue,
                context.toggle_enabled,
                context.move_up,
                context.move_down,
                context.remove_plugin,
                context.remove_directory,
                context.rescan_library,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );

        let appearance = AppearanceTranslations::for_language(*language);
        assert!(!appearance.navigation_mode_description.trim().is_empty());
        let federation = FederationTranslations::for_language(*language);
        assert!(
            [
                federation.no_remote_sources,
                federation.cancel_scan,
                federation.test,
                federation.scan,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );

        let settings = SettingsSurfaceTranslations::for_language(*language);
        let external = settings.external;
        assert!(
            [
                settings.max_cpu_cores_description,
                settings.cpu_cores_unit,
                external.title,
                external.activate,
                external.scan,
                external.activating,
                external.scanning,
                external.runtime_access,
                external.enabled,
                external.disabled,
                external.import_grants,
                external.media_roots,
                external.protected_import_roots,
                external.runtime_error,
                external.scan_error,
                external.results,
                external.loadable,
                external.discovered,
                external.unsupported,
                external.none_found,
                external.more_results,
                external.path,
                external.channels,
                external.isolated,
                external.add_to_rack,
                external.added_to_rack,
                external.add_error,
                external.instruments_unsupported,
                external.parameters_unavailable,
                external.saved_state,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );

        let footer = FooterTranslations::for_language(*language);
        assert!(
            [
                footer.hide,
                footer.previous_track,
                footer.seek_back_30s,
                footer.play,
                footer.pause,
                footer.seek_forward_30s,
                footer.next_track,
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );
        let audio_device = AudioDeviceTranslations::for_language(*language);
        assert!(
            [audio_device.file_player, audio_device.hal_device]
                .iter()
                .all(|value| !value.trim().is_empty())
        );
        let playback = PlaybackApplyTranslations::for_language(*language);
        assert!(!playback.clear_eq.trim().is_empty());
        let spectrum = SpectrumTranslations::for_language(*language);
        assert!(
            [
                spectrum.none,
                spectrum.standard,
                spectrum.min_frequency_short
            ]
            .iter()
            .all(|value| !value.trim().is_empty())
        );

        let tutorial = TutorialTranslations::for_language(*language);
        assert_eq!(tutorial.screens.len(), 7);
        assert!(
            tutorial.screens.iter().all(|screen| {
                !screen.title.trim().is_empty()
                    && screen.content.len() == 3
                    && screen
                        .content
                        .iter()
                        .all(|paragraph| !paragraph.trim().is_empty())
            }),
            "tutorial copy is incomplete for {}",
            language.code()
        );
        assert!(
            [tutorial.previous, tutorial.next, tutorial.get_started]
                .iter()
                .all(|value| !value.trim().is_empty()),
            "tutorial navigation copy is incomplete for {}",
            language.code()
        );

        let server = ServerSettingsTranslations::for_language(*language);
        assert!(
            [
                server.serves_media,
                server.sotf_api,
                server.url,
                server.token,
                server.scan_to_add,
                server.mpd_server,
                server.tls,
                server.authentication,
                server.client_certificate_help,
                server.dlna_server,
                server.protocol,
                server.http_compatibility,
                server.remote_players,
                server.api_address_help,
                server.api_token_help,
                server.no_remote_players,
                server.selected,
                server.certificate,
                server.password,
                server.add_server,
                server.scan_qr,
                server.test,
                server.select,
                server.remove,
                server.enable,
                server.disable,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "server-settings copy is incomplete for {}",
            language.code()
        );

        let phone = PhoneTranslations::for_language(*language);
        assert!(
            [
                phone.home_empty,
                phone.home,
                phone.screen_guide,
                phone.show_tutorial,
                phone.see_all,
                phone.search_library,
                phone.queue_empty,
                phone.up_next,
                phone.plugin_chain,
                phone.add,
                phone.no_plugin_selected,
                phone.add_filter,
                phone.reset,
                phone.delete_filter,
                phone.no_touch_parameters,
                phone.edit,
                phone.filters,
                phone.open_rack_editor,
                phone.remove,
                phone.no_saved_streams,
                phone.play,
                phone.back,
                phone.next,
                phone.search_shortcuts,
                phone.magic_radio,
                phone.back_to_genres,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "phone copy is incomplete for {}",
            language.code()
        );
    }

    let sources = [
        app_source("components/dialogs/player_view.rs"),
        app_source("components/settings/servers/misc.rs").replace(".child(\"\")", ""),
        app_source("ui/phone.rs").replace(".child(\"SOTF\")", ""),
        app_source("components/dialogs/tutorial/consts.rs").replace("Text::new(\"\\u{2022}\")", ""),
        app_source("components/dialogs/tutorial/types.rs").replace(".child(\"\\u{1f4a1}\")", ""),
    ];
    for source in sources {
        for literal_surface in [
            "Text::new(\"",
            "Text::label(\"",
            "Text::section_header(\"",
            ".label(\"",
            ".placeholder(\"",
            ".title(\"",
            ".child(\"",
        ] {
            assert!(
                !source.contains(literal_surface),
                "localized surface contains untranslated visible text via {literal_surface}"
            );
        }
    }
}

#[test]
fn external_plugin_scan_counts_preserve_every_status() {
    let descriptor = |name: &str, scan_status: PluginScanStatus| PluginDescriptor {
        id: format!("clap.{name}"),
        name: name.to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0".to_string(),
        format: PluginFormat::Clap,
        path: PathBuf::from(format!("/tmp/{name}.clap")),
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: Vec::new(),
        scan_status,
    };
    let plugins = vec![
        descriptor("loadable", PluginScanStatus::Loadable),
        descriptor("discovered", PluginScanStatus::Discovered),
        descriptor("unsupported-a", PluginScanStatus::UnsupportedByBuild),
        descriptor("unsupported-b", PluginScanStatus::UnsupportedByBuild),
    ];

    assert_eq!(
        ExternalPluginScanCounts::from_plugins(&plugins),
        ExternalPluginScanCounts {
            total: 4,
            loadable: 1,
            discovered: 1,
            unsupported: 2,
        }
    );
}

#[test]
fn external_plugin_scan_pagination_reaches_every_result() {
    let mut ui = ExternalPluginUiState::default();
    assert_eq!(ui.visible_scan_result_count(250), 100);
    assert_eq!(ui.show_more_scan_results(250), 200);
    assert_eq!(ui.show_more_scan_results(250), 250);
    assert_eq!(ui.show_more_scan_results(250), 250);

    ui.reset_scan_result_pagination();
    assert_eq!(ui.visible_scan_result_count(250), 100);
    assert_eq!(ui.visible_scan_result_count(37), 37);
}

#[test]
fn external_plugin_worker_health_uses_current_state_not_historical_counters() {
    use sotf_audio::engine::{
        IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
        IsolatedExternalPluginWorkerEvent, IsolatedExternalPluginWorkerStatus,
    };

    let status = |event, sandbox_status, error: Option<&str>, sandbox_reason: Option<&str>| {
        IsolatedExternalPluginWorkerStatus {
            plugin_index: 0,
            node_id: 7,
            plugin_instance_id: Some(11),
            event,
            error: error.map(str::to_string),
            worker_start_count: 9,
            worker_exit_count: 8,
            worker_launch_failure_count: 7,
            block_timeout_count: 6,
            block_worker_failure_count: 5,
            block_wrong_sequence_count: 4,
            sandbox_status,
            sandbox_backend: IsolatedExternalPluginSandboxBackend::LinuxLandlock,
            sandbox_reason: sandbox_reason.map(str::to_string),
        }
    };

    assert_eq!(
        external_plugin_worker_health(&status(
            Some(IsolatedExternalPluginWorkerEvent::AlreadyRunning),
            IsolatedExternalPluginSandboxStatus::Enforced,
            None,
            None,
        )),
        ExternalPluginWorkerHealth::Healthy,
        "historical failures must not keep a recovered worker unhealthy"
    );
    assert_eq!(
        external_plugin_worker_health(&status(
            Some(IsolatedExternalPluginWorkerEvent::NotRunning),
            IsolatedExternalPluginSandboxStatus::Enforced,
            None,
            None,
        )),
        ExternalPluginWorkerHealth::Failed
    );
    assert_eq!(
        external_plugin_worker_health(&status(
            Some(IsolatedExternalPluginWorkerEvent::Started { pid: 42 }),
            IsolatedExternalPluginSandboxStatus::Disabled,
            None,
            Some("sandbox disabled by policy"),
        )),
        ExternalPluginWorkerHealth::Degraded
    );
    assert_eq!(
        external_plugin_worker_health(&status(
            Some(IsolatedExternalPluginWorkerEvent::Started { pid: 42 }),
            IsolatedExternalPluginSandboxStatus::Enforced,
            Some("worker handshake failed"),
            None,
        )),
        ExternalPluginWorkerHealth::Failed
    );
    assert_eq!(
        external_plugin_worker_health(&status(
            None,
            IsolatedExternalPluginSandboxStatus::Enforced,
            None,
            None,
        )),
        ExternalPluginWorkerHealth::Degraded
    );
}

#[test]
fn external_plugin_sandbox_backends_have_localized_human_labels() {
    use sotf_audio::engine::{
        IsolatedExternalPluginSandboxBackend, IsolatedExternalPluginSandboxStatus,
        IsolatedExternalPluginWorkerEvent, IsolatedExternalPluginWorkerStatus,
    };

    let backends = [
        IsolatedExternalPluginSandboxBackend::Unknown,
        IsolatedExternalPluginSandboxBackend::LinuxLandlock,
        IsolatedExternalPluginSandboxBackend::MacosAppSandboxHelper,
        IsolatedExternalPluginSandboxBackend::MacosProcessIsolation,
        IsolatedExternalPluginSandboxBackend::WindowsProcessIsolation,
    ];
    let languages = [
        Language::English,
        Language::French,
        Language::German,
        Language::Spanish,
    ];

    for language in languages {
        for backend in backends {
            let status = IsolatedExternalPluginWorkerStatus {
                plugin_index: 0,
                node_id: 1,
                plugin_instance_id: Some(2),
                event: Some(IsolatedExternalPluginWorkerEvent::AlreadyRunning),
                error: None,
                worker_start_count: 1,
                worker_exit_count: 0,
                worker_launch_failure_count: 0,
                block_timeout_count: 0,
                block_worker_failure_count: 0,
                block_wrong_sequence_count: 0,
                sandbox_status: IsolatedExternalPluginSandboxStatus::Enforced,
                sandbox_backend: backend,
                sandbox_reason: None,
            };
            let label = SettingsSurfaceTranslations::for_language(language)
                .external
                .worker
                .sandbox_label(&status);
            for debug_variant in [
                "LinuxLandlock",
                "MacosAppSandboxHelper",
                "MacosProcessIsolation",
                "WindowsProcessIsolation",
            ] {
                assert!(
                    !label.contains(debug_variant),
                    "sandbox label leaked debug enum {debug_variant}: {label}"
                );
            }
        }
    }
}

#[test]
fn external_plugin_engine_diagnostics_attach_to_descriptor_and_persist() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("diagnostic.clap");
    std::fs::write(&path, b"fixture").unwrap();
    let descriptor = PluginDescriptor {
        id: "clap.diagnostic".to_string(),
        name: "Diagnostic Plug-in".to_string(),
        vendor: "Test Vendor".to_string(),
        version: "1.0".to_string(),
        format: PluginFormat::Clap,
        path,
        audio_inputs: 2,
        audio_outputs: 4,
        is_instrument: false,
        categories: vec!["Effect".to_string()],
        scan_status: PluginScanStatus::Loadable,
    };
    let error_key = external_plugin_error_key(&descriptor);
    let mut state = PluginState::new();
    state
        .add_plugin_settings(PluginSettings::External {
            state: ExternalPluginState::new(
                descriptor,
                ExternalPluginSandboxMode::Isolated,
                vec![1, 2, 3],
            ),
        })
        .unwrap();
    let engine_index = state
        .graph
        .get_engine_index_by_linear_position(state.selected_plugin_index)
        .unwrap();
    let plugin_instance_id = state
        .graph
        .get_plugin(state.selected_plugin_index)
        .unwrap()
        .id;
    let worker_error = "worker launch rejected by sandbox".to_string();
    let worker_status = sotf_audio::engine::IsolatedExternalPluginWorkerStatus {
        // Deliberately stale/transient slot: stable instance identity must win.
        plugin_index: engine_index + 100,
        node_id: 42,
        plugin_instance_id: Some(plugin_instance_id),
        event: Some(sotf_audio::engine::IsolatedExternalPluginWorkerEvent::NotRunning),
        error: Some(worker_error.clone()),
        worker_start_count: 0,
        worker_exit_count: 0,
        worker_launch_failure_count: 1,
        block_timeout_count: 0,
        block_worker_failure_count: 0,
        block_wrong_sequence_count: 0,
        sandbox_status: sotf_audio::engine::IsolatedExternalPluginSandboxStatus::Enforced,
        sandbox_backend:
            sotf_audio::engine::IsolatedExternalPluginSandboxBackend::MacosProcessIsolation,
        sandbox_reason: None,
    };

    let build_diagnostic = sotf_audio::engine::PluginBuildDiagnostic::chain_plugin(
        engine_index,
        Some(plugin_instance_id),
        "external",
        "1 plugin(s) skipped",
    );
    state.sync_external_plugin_engine_diagnostics(
        vec![build_diagnostic.clone()],
        vec![worker_status.clone()],
    );
    assert_eq!(
        state.external_plugin_ui.build_diagnostics,
        vec![build_diagnostic.clone()]
    );
    assert_eq!(
        state.external_plugin_build_diagnostic(Some(plugin_instance_id), Some(engine_index)),
        Some(&build_diagnostic)
    );

    let graph_fallback = sotf_audio::engine::PluginBuildDiagnostic::graph_node(
        engine_index,
        None,
        "external",
        "graph node failed",
    );
    state.sync_external_plugin_engine_diagnostics(
        vec![graph_fallback.clone()],
        vec![worker_status.clone()],
    );
    assert_eq!(
        state.external_plugin_build_diagnostic(Some(plugin_instance_id), Some(engine_index)),
        Some(&graph_fallback),
        "legacy graph diagnostics must fall back to the exact engine node id"
    );

    let host_global = sotf_audio::engine::PluginBuildDiagnostic::host("host setup failed");
    state.sync_external_plugin_engine_diagnostics(
        vec![host_global.clone()],
        vec![worker_status.clone()],
    );
    assert_eq!(
        state.external_plugin_ui.build_diagnostics,
        vec![host_global]
    );
    assert!(
        state
            .external_plugin_build_diagnostic(Some(plugin_instance_id), Some(engine_index))
            .is_none(),
        "a host-global diagnostic must not be blamed on every external plugin"
    );
    assert_eq!(
        state.external_plugin_ui.worker_statuses,
        std::slice::from_ref(&worker_status)
    );
    assert_eq!(
        state
            .external_plugin_ui
            .worker_errors
            .get(&plugin_instance_id),
        Some(&worker_error)
    );
    assert!(
        !state
            .external_plugin_ui
            .load_errors
            .contains_key(&error_key)
    );

    state.sync_external_plugin_engine_diagnostics(Vec::new(), Vec::new());
    assert!(state.external_plugin_ui.build_diagnostics.is_empty());
    assert!(state.external_plugin_ui.worker_statuses.is_empty());
    assert_eq!(
        state
            .external_plugin_ui
            .worker_errors
            .get(&plugin_instance_id),
        Some(&worker_error),
        "worker failures must remain visible after the worker status disappears"
    );

    let mut still_stopped = worker_status.clone();
    still_stopped.error = None;
    state.sync_external_plugin_engine_diagnostics(Vec::new(), vec![still_stopped.clone()]);
    assert_eq!(
        state
            .external_plugin_ui
            .worker_errors
            .get(&plugin_instance_id),
        Some(&worker_error),
        "a stopped status without a fresh error must not erase the last actionable failure"
    );

    let mut recovered = still_stopped;
    recovered.error = None;
    recovered.event = Some(sotf_audio::engine::IsolatedExternalPluginWorkerEvent::AlreadyRunning);
    state.sync_external_plugin_engine_diagnostics(Vec::new(), vec![recovered]);
    assert!(
        !state
            .external_plugin_ui
            .worker_errors
            .contains_key(&plugin_instance_id),
        "an explicit healthy worker status must clear the stale runtime error"
    );
}

#[test]
fn external_plugin_worker_diagnostics_distinguish_duplicate_descriptor_instances() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("duplicate-diagnostic.clap");
    std::fs::write(&path, b"fixture").unwrap();
    let settings = PluginSettings::External {
        state: ExternalPluginState::new(
            PluginDescriptor {
                id: "clap.duplicate-diagnostic".to_string(),
                name: "Duplicate Diagnostic Plug-in".to_string(),
                vendor: "Test Vendor".to_string(),
                version: "1.0".to_string(),
                format: PluginFormat::Clap,
                path,
                audio_inputs: 2,
                audio_outputs: 2,
                is_instrument: false,
                categories: vec!["Effect".to_string()],
                scan_status: PluginScanStatus::Loadable,
            },
            ExternalPluginSandboxMode::Isolated,
            Vec::new(),
        ),
    };
    let mut state = PluginState::new();
    state.add_plugin_settings(settings.clone()).unwrap();
    let first_instance_id = state
        .graph
        .get_plugin(state.selected_plugin_index)
        .unwrap()
        .id;
    state.add_plugin_settings(settings).unwrap();
    let second_instance_id = state
        .graph
        .get_plugin(state.selected_plugin_index)
        .unwrap()
        .id;
    assert_ne!(first_instance_id, second_instance_id);

    let worker_status = |plugin_instance_id, node_id, error: Option<&str>| {
        sotf_audio::engine::IsolatedExternalPluginWorkerStatus {
            // Both transient indices are deliberately unusable: identity must
            // come exclusively from the persisted player instance id.
            plugin_index: usize::MAX,
            node_id,
            plugin_instance_id: Some(plugin_instance_id),
            event: Some(sotf_audio::engine::IsolatedExternalPluginWorkerEvent::NotRunning),
            error: error.map(str::to_string),
            worker_start_count: 0,
            worker_exit_count: 0,
            worker_launch_failure_count: u64::from(error.is_some()),
            block_timeout_count: 0,
            block_worker_failure_count: 0,
            block_wrong_sequence_count: 0,
            sandbox_status: sotf_audio::engine::IsolatedExternalPluginSandboxStatus::Enforced,
            sandbox_backend:
                sotf_audio::engine::IsolatedExternalPluginSandboxBackend::MacosProcessIsolation,
            sandbox_reason: None,
        }
    };

    state.sync_external_plugin_engine_diagnostics(
        Vec::new(),
        vec![
            worker_status(first_instance_id, 101, Some("first worker failed")),
            worker_status(second_instance_id, 102, Some("second worker failed")),
        ],
    );
    assert_eq!(
        state
            .external_plugin_ui
            .worker_errors
            .get(&first_instance_id)
            .map(String::as_str),
        Some("first worker failed")
    );
    assert_eq!(
        state
            .external_plugin_ui
            .worker_errors
            .get(&second_instance_id)
            .map(String::as_str),
        Some("second worker failed")
    );

    let mut recovered_first = worker_status(first_instance_id, 101, None);
    recovered_first.event =
        Some(sotf_audio::engine::IsolatedExternalPluginWorkerEvent::AlreadyRunning);
    state.sync_external_plugin_engine_diagnostics(Vec::new(), vec![recovered_first]);
    assert!(
        !state
            .external_plugin_ui
            .worker_errors
            .contains_key(&first_instance_id)
    );
    assert_eq!(
        state
            .external_plugin_ui
            .worker_errors
            .get(&second_instance_id)
            .map(String::as_str),
        Some("second worker failed"),
        "a healthy duplicate instance must not clear its sibling's failure"
    );
}

#[test]
fn room_eq_report_copy_is_complete_and_direct_literals_are_extracted() {
    use sotf_audio_player_gpui::i18n::RoomEqReportTranslations;

    for language in Language::all() {
        let text = RoomEqReportTranslations::for_language(*language);
        assert!(
            [
                text.optimization_summary,
                text.bass_management,
                text.bass_routing_graph,
                text.rms_programme_gain,
                text.all_channels_overview,
                text.epa_scores,
                text.metric,
                text.before_eq,
                text.after_eq,
                text.delta,
                text.preference,
                text.evaluation,
                text.potency,
                text.activity,
                text.sharpness_acum,
                text.roughness,
                text.total_loudness_sone,
                text.loudness_balance,
                text.epa_interpretation,
                text.eq_filters,
                text.impulse_response,
                text.type_label,
                text.crossover_label,
                text.room_eq_filters,
                text.broadband_precorrection_filters,
                text.crossover_frequencies,
                text.original_vs_corrected,
                text.sum,
                text.original,
                text.tonal_balance,
                text.phase_response,
                text.ir,
                text.group_delay,
                text.smoothing,
                text.auto,
                text.trend,
                text.normalize,
                text.channel,
                text.main_ms,
                text.pre_peak_db,
                text.post_peak_db,
                text.pre_audible_db,
                text.post_audible_db,
                text.penalty,
                text.fir_temporal_masking,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "RoomEQ report copy is incomplete for {}",
            language.code()
        );
    }

    for path in [
        "components/room_eq/render.rs",
        "components/room_eq/step_5_review/render.rs",
    ] {
        let source = app_source(path);
        for literal_surface in [
            "Text::new(\"",
            "Text::label(\"",
            "Text::section_header(\"",
            ".label(\"",
            ".placeholder(\"",
            ".title(\"",
            ".child(\"",
        ] {
            assert!(
                !source.contains(literal_surface),
                "{path} contains untranslated visible text via {literal_surface}"
            );
        }
    }
}

#[test]
fn level_meter_copy_is_complete_and_only_technical_literals_remain() {
    use sotf_audio_player_gpui::i18n::LevelMeterTranslations;

    for language in Language::all() {
        let text = LevelMeterTranslations::for_language(*language);
        assert!(
            [
                text.gain_reduction,
                text.peak,
                text.true_peak,
                text.lufs,
                text.peak_spread,
                text.even,
                text.stereo_width,
                text.mono,
                text.wide,
                text.level_meters,
            ]
            .iter()
            .all(|value| !value.trim().is_empty()),
            "level-meter copy is incomplete for {}",
            language.code()
        );
    }

    let source = app_source("components/plugins/level_meters/render.rs");
    let source = [
        ".child(\"GR\")",
        ".child(\"dBFS\")",
        ".child(\"-60\")",
        ".child(\"-30\")",
        ".child(\"-10\")",
        ".child(\"0\")",
        ".child(\"6 dB\")",
        ".child(\"12 dB\")",
        ".child(\"24+\")",
        ".child(\"50%\")",
        ".child(\"X\")",
        ".child(\"M\")",
        ".child(\"S\")",
        ".child(\"D\")",
    ]
    .into_iter()
    .fold(source, |source, literal| source.replace(literal, ""));
    for literal_surface in [
        "Text::new(\"",
        "Text::label(\"",
        "Text::section_header(\"",
        ".label(\"",
        ".placeholder(\"",
        ".title(\"",
        ".child(\"",
    ] {
        assert!(
            !source.contains(literal_surface),
            "level meters contain untranslated visible text via {literal_surface}"
        );
    }
}

#[test]
fn application_visible_literal_sinks_require_explicit_technical_allowlist() {
    fn visit_rs_files(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read directory entry: {err}"));
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    fn first_literal_argument_after<'a>(source: &'a str, prefix: &str) -> Vec<&'a str> {
        let mut literals = Vec::new();
        let mut remainder = source;
        while let Some(offset) = remainder.find(prefix) {
            let after_prefix = &remainder[offset + prefix.len()..];
            let literal = after_prefix.trim_start();
            if !literal.starts_with('"') {
                remainder = after_prefix;
                continue;
            }
            let literal = &literal[1..];
            let bytes = literal.as_bytes();
            let mut escaped = false;
            let mut end = None;
            for (index, byte) in bytes.iter().enumerate() {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    end = Some(index);
                    break;
                }
            }
            let end = end.unwrap_or_else(|| panic!("unterminated visible literal after {prefix}"));
            literals.push(&literal[..end]);
            remainder = &literal[end + 1..];
        }
        literals
    }

    fn nth_literal_argument_after<'a>(
        source: &'a str,
        prefix: &str,
        target_argument: usize,
    ) -> Vec<&'a str> {
        fn literal_from_argument(argument: &str) -> Option<&str> {
            let argument = argument.trim();
            let literal = argument.strip_prefix('"')?;
            let mut escaped = false;
            for (index, byte) in literal.as_bytes().iter().enumerate() {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    return Some(&literal[..index]);
                }
            }
            None
        }

        let mut literals = Vec::new();
        let mut remainder = source;
        while let Some(offset) = remainder.find(prefix) {
            let arguments = &remainder[offset + prefix.len()..];
            let bytes = arguments.as_bytes();
            let mut argument_start = 0usize;
            let mut argument_index = 0usize;
            let mut parentheses = 0usize;
            let mut brackets = 0usize;
            let mut braces = 0usize;
            let mut in_string = false;
            let mut escaped = false;
            let mut consumed = 0usize;

            for (index, byte) in bytes.iter().enumerate() {
                if in_string {
                    if escaped {
                        escaped = false;
                    } else if *byte == b'\\' {
                        escaped = true;
                    } else if *byte == b'"' {
                        in_string = false;
                    }
                    continue;
                }

                match *byte {
                    b'"' => in_string = true,
                    b'(' => parentheses += 1,
                    b')' if parentheses > 0 => parentheses -= 1,
                    b'[' => brackets += 1,
                    b']' if brackets > 0 => brackets -= 1,
                    b'{' => braces += 1,
                    b'}' if braces > 0 => braces -= 1,
                    b',' if parentheses == 0 && brackets == 0 && braces == 0 => {
                        if argument_index == target_argument
                            && let Some(literal) =
                                literal_from_argument(&arguments[argument_start..index])
                        {
                            literals.push(literal);
                        }
                        argument_index += 1;
                        argument_start = index + 1;
                    }
                    b')' if brackets == 0 && braces == 0 => {
                        if argument_index == target_argument
                            && let Some(literal) =
                                literal_from_argument(&arguments[argument_start..index])
                        {
                            literals.push(literal);
                        }
                        consumed = index + 1;
                        break;
                    }
                    _ => {}
                }
            }

            remainder = if consumed == 0 {
                arguments
            } else {
                &arguments[consumed..]
            };
        }
        literals
    }

    // Stable product/format identifiers are deliberately not translated. This
    // list is restricted to symbols, units, channel layouts, brands, and the
    // canonical plugin names shared with presets and the engine registry.
    let allowed = [
        "",
        "!",
        "#",
        "+",
        "-",
        "/",
        "0",
        "-10",
        "-30",
        "-60",
        "-60 dB",
        "0.00",
        "0 dB",
        "2D",
        "3D",
        "1 kHz",
        "2 kHz",
        "1/3 Oct",
        "1/2 Oct",
        "1/6 Oct",
        "1 Oct",
        "1/12 Oct",
        "1/24 Oct",
        "1/48 Oct",
        "+3dB/oct",
        "+6dB/oct",
        "2 ch (Stereo)",
        "4 ch (Quad)",
        "6 ch (5.1)",
        "8 ch (7.1)",
        "6 dB",
        "12 dB",
        "24+",
        "50%",
        "D",
        "dBFS",
        "Emacs",
        "GR",
        "IR",
        "M",
        "M S D",
        "ON",
        "OUT",
        "OUT\\\\IN",
        "OUTPUT",
        "P",
        "PIR",
        "R:",
        "S",
        "SOTF",
        "SPL",
        "SRC",
        "Vim",
        "VSCode",
        "X",
        "AAE Reverb",
        "A/B Compare",
        "AEC",
        "AirPlay",
        "Binaural Decoder",
        "Channel Mute/Solo",
        "Compressor",
        "Convolution",
        "Crossfeed",
        "Crosstalk Cancellation",
        "Declick",
        "De-Esser",
        "Delay",
        "Denoiser",
        "Downmix",
        "Dynamic EQ",
        "EQ",
        "Expander",
        "FIR Designer",
        "Fletcher-Munson",
        "Gain",
        "Gate",
        "Hiss Reducer",
        "JSON",
        "Limiter",
        "FIR EQ",
        "Loudness Compensation",
        "Loudness Monitor",
        "Matrix Mixer",
        "Mono to Stereo",
        "Parametric EQ",
        "Pink (+3dB/oct)",
        "PND Varispeed",
        "Saturation",
        "Spectral Compressor",
        "Spectrum Analyzer",
        "Speech Denoiser",
        "Stereo Imager",
        "Transient Shaper",
        "Upmixer",
        "© 2026 Spinorama",
        "[/]",
        "\\u{1f4a1}",
        "\\u{2022}",
        "(chain out)",
        "mp3",
        "qEHVI",
        // Room-EQ smoothing mode identifier paired with octave fractions.
        "Raw",
        "|",
        "::",
        ":::",
        "<",
        ">",
        "?",
        "←/→",
        "↑/↓",
        "↑",
        "↓",
        "−",
        "\\u{25B2}",
        "\\u{25BC}",
        "\\u{2715}",
        "↵",
        "→",
        "⟳",
        "⤓",
        "⚙",
        "⚠",
        "♪",
        "ρ",
        "…",
        "◎",
        "🔒",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for directory in ["app", "components", "ui"] {
        visit_rs_files(&root.join(directory), &mut files);
    }
    let mut violations = Vec::new();
    for path in files.into_iter().filter(|path| {
        path.file_name()
            .is_none_or(|name| name != "translations.rs")
            && path.file_name().is_none_or(|name| name != "tests.rs")
            && !path
                .components()
                .any(|component| component.as_os_str() == "tests")
    }) {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
            // Typed translation lookups are not direct visible literals.
            .replace("text.label(\"", "text.localized_label(\"");
        for prefix in [
            "Text::new(",
            "Text::caption(",
            "Text::body(",
            "Text::eyebrow(",
            "Text::label(",
            "Text::selectable(",
            "Text::section_header(",
            "Heading::new(",
            "Heading::h1(",
            "Heading::h4(",
            "Badge::new(",
            ".label(",
            ".placeholder(",
            ".title(",
            ".child(",
            ".aria_label(",
            ".tooltip(",
            ".description(",
        ] {
            for literal in first_literal_argument_after(&source, prefix) {
                if !allowed.contains(literal) {
                    violations.push(format!(
                        "{} contains non-allowlisted visible literal {literal:?} via {prefix}",
                        path.display()
                    ));
                }
            }
        }
        for (prefix, argument) in [
            ("Button::new(", 1),
            ("MenuItem::new(", 1),
            ("AccordionItem::new(", 1),
            ("SelectOption::new(", 1),
            ("TabItem::new(", 1),
            ("ButtonSetOption::new(", 1),
            ("EmptyState::new(", 0),
        ] {
            for literal in nth_literal_argument_after(&source, prefix, argument) {
                if !allowed.contains(literal) {
                    violations.push(format!(
                        "{} contains non-allowlisted visible literal {literal:?} via argument {} of {prefix}",
                        path.display(),
                        argument + 1,
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "first-party visible literal allowlist violations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn first_party_toast_templates_are_in_the_runtime_translation_catalog() {
    use sotf_audio_player_gpui::i18n::RuntimeMessageTranslations;

    fn visit_rs_files(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).expect("read source directory") {
                visit_rs_files(&entry.expect("read source entry").path(), files);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path.to_path_buf());
        }
    }

    fn first_argument(source: &str, start: usize) -> Option<&str> {
        let bytes = source.as_bytes();
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for index in start..bytes.len() {
            let byte = bytes[index];
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' if depth == 0 => return Some(&source[start..index]),
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => return Some(&source[start..index]),
                _ => {}
            }
        }
        None
    }

    fn first_string_literal(argument: &str) -> Option<String> {
        let bytes = argument.as_bytes();
        let start = bytes.iter().position(|byte| *byte == b'"')? + 1;
        let mut escaped = false;
        for (offset, &byte) in bytes[start..].iter().enumerate() {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                return Some(
                    argument[start..start + offset]
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\"),
                );
            }
        }
        None
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for directory in ["app", "components", "ui"] {
        visit_rs_files(&root.join(directory), &mut files);
    }

    let mut templates = BTreeSet::new();
    let markers = [
        "ToastMessage::success(",
        "ToastMessage::error(",
        "ToastMessage::info(",
        "ToastMessage::warning(",
        "ToastMessage::persistent(",
    ];
    for path in files.into_iter().filter(|path| {
        !path
            .components()
            .any(|component| component.as_os_str() == "tests")
            && !path.ends_with("runtime_messages.rs")
    }) {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for marker in markers {
            let mut cursor = 0;
            while let Some(relative) = source[cursor..].find(marker) {
                let argument_start = cursor + relative + marker.len();
                if let Some(argument) = first_argument(&source, argument_start)
                    && let Some(template) = first_string_literal(argument)
                {
                    templates.insert(template);
                }
                cursor = argument_start;
            }
        }

        let mut cursor = 0;
        let status_marker = "status_message =";
        while let Some(relative) = source[cursor..].find(status_marker) {
            let value_start = cursor + relative + status_marker.len();
            let value = source[value_start..].trim_start();
            if (value.starts_with('"') || value.starts_with("format!"))
                && let Some(template) = first_string_literal(value)
            {
                templates.insert(template);
            }
            cursor = value_start;
        }

        let mut cursor = 0;
        let error_marker = "error_message = Some";
        while let Some(relative) = source[cursor..].find(error_marker) {
            let some_start = cursor + relative + error_marker.len();
            let remainder = &source[some_start..];
            if let Some(open_relative) = remainder.find('(') {
                let argument_start = some_start + open_relative + 1;
                if let Some(argument) = first_argument(&source, argument_start)
                    && let Some(template) = first_string_literal(argument)
                {
                    templates.insert(template);
                }
            }
            cursor = some_start;
        }
    }

    let missing = templates
        .into_iter()
        .filter(|template| !RuntimeMessageTranslations::is_catalogued(template))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "first-party toast/status templates missing from runtime translations:\n{}",
        missing.join("\n")
    );
}

#[test]
fn runtime_message_translations_preserve_dynamic_values_and_external_fallbacks() {
    use sotf_audio_player_gpui::i18n::RuntimeMessageTranslations;

    let french = RuntimeMessageTranslations::for_language(Language::French);
    assert_eq!(
        french.translate("Scan complete: 12 albums, 345 tracks merged."),
        "Analyse terminée : 12 albums et 345 pistes fusionnés."
    );
    assert_eq!(
        french.translate("Playback error: device vanished"),
        "Erreur de lecture : device vanished"
    );

    let german = RuntimeMessageTranslations::for_language(Language::German);
    assert_eq!(
        german.translate("External plugin vendor error"),
        "External plugin vendor error"
    );
}

#[test]
fn application_translation_keys_have_placeholder_parity_and_orphan_debt_does_not_grow() {
    fn initializer_body<'a>(source: &'a str, language: &str, next: Option<&str>) -> &'a str {
        let marker = format!("    pub fn {language}() -> Self {{");
        let start = source
            .find(&marker)
            .unwrap_or_else(|| panic!("missing {language} translation initializer"));
        let body = &source[start + marker.len()..];
        next.and_then(|next_language| {
            body.find(&format!("    pub fn {next_language}() -> Self {{"))
        })
        .map_or(body, |end| &body[..end])
    }

    fn placeholder_signature(body: &str) -> Vec<(String, Vec<String>)> {
        body.lines()
            .filter_map(|line| {
                let (field, value) = line.trim().split_once(':')?;
                if field.is_empty()
                    || !field
                        .chars()
                        .all(|character| character == '_' || character.is_ascii_alphanumeric())
                {
                    return None;
                }

                let value = value.trim_start();
                if !value.starts_with('"') {
                    return None;
                }

                let mut placeholders = Vec::new();
                let mut chars = value.chars().peekable();
                while let Some(character) = chars.next() {
                    if character != '{' {
                        continue;
                    }
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        continue;
                    }

                    let mut placeholder = String::new();
                    let mut closed = false;
                    for character in chars.by_ref() {
                        if character == '}' {
                            closed = true;
                            break;
                        }
                        placeholder.push(character);
                    }
                    assert!(closed, "{field} contains an unmatched format placeholder");
                    placeholders.push(placeholder);
                }

                Some((field.to_string(), placeholders))
            })
            .collect()
    }

    fn visit_rs_files(path: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read directory entry: {err}"));
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    fn contains_identifier(source: &str, identifier: &str) -> bool {
        source
            .split(|character: char| character != '_' && !character.is_ascii_alphanumeric())
            .any(|token| token == identifier)
    }

    let source = app_source("app/i18n/translations.rs");
    let languages = [
        ("english", Some("french")),
        ("french", Some("german")),
        ("german", Some("spanish")),
        ("spanish", None),
    ];
    let signatures = languages
        .map(|(language, next)| placeholder_signature(initializer_body(&source, language, next)));
    for (language, signature) in languages
        .iter()
        .skip(1)
        .map(|(language, _)| *language)
        .zip(signatures.iter().skip(1))
    {
        assert_eq!(
            signature, &signatures[0],
            "{language} translation keys or format placeholders drift from English"
        );
    }

    let struct_start = source
        .find("pub struct Translations {")
        .expect("Translations struct should exist");
    let struct_body = &source[struct_start
        ..source[struct_start..]
            .find("\n}\n\nimpl Translations")
            .map(|offset| struct_start + offset)
            .expect("Translations impl should follow its struct")];
    let fields = struct_body
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("pub ")
                .and_then(|line| line.split_once(':'))
                .map(|(field, _)| field.to_string())
        })
        .collect::<Vec<_>>();

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    visit_rs_files(root, &mut files);
    let production_sources = files
        .into_iter()
        .filter(|path| {
            !path.ends_with("app/i18n/translations.rs")
                && path.file_name().is_none_or(|name| name != "tests.rs")
                && !path
                    .components()
                    .any(|component| component.as_os_str() == "tests")
        })
        .map(|path| {
            std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        })
        .collect::<Vec<_>>();
    let orphans = fields
        .iter()
        .filter(|field| {
            !production_sources
                .iter()
                .any(|source| contains_identifier(source, field))
        })
        .collect::<Vec<_>>();
    assert!(
        orphans.is_empty(),
        "application translation fields must be used by production code: {orphans:?}"
    );
}

#[test]
fn every_app_plugin_has_gpui_and_simple_parameter_surfaces() {
    let registry = GpuiViewRegistry::new();

    for plugin_type in PluginType::all() {
        let settings = PluginSettings::default_for(&plugin_type).unwrap();
        let ui_key = plugin_type_key(&settings);
        let catalog = catalog_entry(plugin_type.wire_name()).unwrap();
        match catalog.metadata.ui {
            PluginUiKind::Custom => assert!(
                registry.get(ui_key).is_some(),
                "{} catalog says custom UI but registry has none",
                plugin_type.name()
            ),
            PluginUiKind::Generated => assert!(
                registry.get(ui_key).is_none() && settings.layout().is_some(),
                "{} catalog says generated UI but custom/declarative registration disagrees",
                plugin_type.name()
            ),
            other => panic!(
                "generic application plugin {} has invalid UI exposure {other:?}",
                plugin_type.name()
            ),
        }
        assert!(
            registry.get(ui_key).is_some() || settings.layout().is_some(),
            "{} has neither a custom GPUI view nor a declarative PluginLayout",
            plugin_type.name()
        );

        let descriptors = settings.get_descriptors();
        let values = settings.get_params();
        assert_eq!(
            descriptors.len(),
            values.len(),
            "{} simple-view descriptor/value drift",
            plugin_type.name()
        );
        assert!(
            descriptors
                .iter()
                .all(|descriptor| !descriptor.name.trim().is_empty()),
            "{} exposes an unnamed simple-view parameter",
            plugin_type.name()
        );
        assert!(
            values.iter().all(|value| !value.name.trim().is_empty()),
            "{} exposes an unnamed simple-view value",
            plugin_type.name()
        );
        if !settings.param_specs().is_empty() {
            assert!(
                !descriptors.is_empty(),
                "{} has host parameters but no simple-view metadata",
                plugin_type.name()
            );
        }
    }
}

#[test]
fn both_midi_controller_layouts_expose_the_same_plugin_parameters() {
    let controllers = [xone_k2_layout(), lcxl_layout()];

    for plugin_type in PluginType::all() {
        let settings = PluginSettings::default_for(&plugin_type).unwrap();
        let params = settings.param_specs();
        let expected: BTreeSet<_> = params
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| {
                (!matches!(spec.param_type, ParamType::FilePath)).then_some(index)
            })
            .collect();

        let mapped: Vec<BTreeSet<_>> = controllers
            .iter()
            .map(|layout| {
                auto_map::auto_map(layout, params, 0, plugin_type.name())
                    .bindings
                    .into_iter()
                    .map(|binding| binding.param_index)
                    .collect()
            })
            .collect();

        assert_eq!(
            mapped[0],
            expected,
            "{} Xone K2 mapping omits or invents parameters",
            plugin_type.name()
        );
        assert_eq!(
            mapped[1],
            expected,
            "{} Launch Control XL mapping omits or invents parameters",
            plugin_type.name()
        );
        assert_eq!(
            mapped[0],
            mapped[1],
            "{} controller layouts do not map the same concepts",
            plugin_type.name()
        );
    }
}

#[test]
fn every_declarative_plugin_layout_solves_at_narrow_and_wide_widths() {
    for plugin_type in PluginType::all() {
        let settings = PluginSettings::default_for(&plugin_type).unwrap();
        let Some(layout) = settings.layout() else {
            continue;
        };

        for width in [320.0_f32, 700.0, 1400.0] {
            let solved = solve_layout(layout.column_constraints, width);
            let allocated_width: f32 = solved.columns.iter().map(|column| column.width).sum();
            assert!(
                allocated_width <= width + f32::EPSILON,
                "{} allocates {allocated_width}px into a {width}px viewport",
                plugin_type.name()
            );
            assert!(
                solved.is_visible(ColumnRole::Main),
                "{} collapses its primary controls at {width}px",
                plugin_type.name()
            );
            assert!(
                solved.columns.iter().all(|column| column.width > 0.0),
                "{} produces a non-positive column width at {width}px",
                plugin_type.name()
            );

            for constraint in layout.column_constraints {
                assert_ne!(
                    solved.is_visible(constraint.role),
                    solved.is_collapsed(constraint.role),
                    "{} does not resolve {:?} exactly once at {width}px",
                    plugin_type.name(),
                    constraint.role
                );
            }
        }
    }
}

#[test]
fn eq_drag_preview_is_taken_only_by_its_plugin() {
    use sotf_audio_player_gpui::app::state::EqDragPreview;
    use sotf_audio_player_gpui::app::state::plugin::PluginUiState;

    let preview = EqDragPreview {
        plugin_idx: 4,
        band_idx: 3,
        frequency: 1_250.0,
        gain_db: -2.5,
    };
    let mut state = PluginUiState::default();
    state.preview_eq_drag(preview);

    assert_eq!(state.take_eq_drag_preview_for(2), None);
    assert_eq!(state.eq_drag_preview, Some(preview));
    assert_eq!(state.take_eq_drag_preview_for(4), Some(preview));
    assert_eq!(state.eq_drag_preview, None);
}
