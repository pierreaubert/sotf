//! Mounted hosted controls use native readback and preserve saved state on failure.
#![cfg(feature = "dev-api")]

use gpui::prelude::*;
use gpui::{
    Bounds, Context, Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext,
    VisualTestContext, Window, WindowHandle, div, point, px, size,
};
use sotf_audio_player::{Player, PluginGraph, PluginSettings};
use sotf_audio_player_gpui::app::dev_api::{DevTrackExt, registry};
use sotf_audio_player_gpui::app::player_handle::PlayerHandle;
use sotf_audio_player_gpui::app::state::ui::LayoutState;
use sotf_audio_player_gpui::app::types::PluginUpdateType;
use sotf_audio_player_gpui::app::{App as PlayerApp, AppState};
use sotf_audio_player_gpui::components::plugins::ui_external::render_parameters;
use sotf_audio_player_gpui::theme::Theme;
use sotf_plugins::{
    ExternalPluginEditorData, ExternalPluginSandboxMode, ExternalPluginState, Parameter,
    ParameterChoice, ParameterId, ParameterValue, PluginDescriptor, PluginFormat, PluginScanStatus,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Independent GPUI test apps reuse window IDs in the process-wide dev registry.
static MOUNTED_REGISTRY_TEST_LOCK: Mutex<()> = Mutex::new(());

struct ExternalControlsView {
    state: Entity<AppState>,
    data: Arc<ExternalPluginEditorData>,
    _fixture: tempfile::TempDir,
    _observation: gpui::Subscription,
}

impl Render for ExternalControlsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .child(render_parameters(
                self.state.clone(),
                &self.data,
                &Theme::dark(),
                cx,
            ))
            .dev_frame()
    }
}

fn create_view(cx: &mut Context<ExternalControlsView>) -> ExternalControlsView {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("ui-parameter-fixture.clap");
    std::fs::write(&path, b"metadata fixture; never loaded").unwrap();
    let descriptor = PluginDescriptor {
        id: "ui.parameter-fixture".into(),
        name: "Hosted UI metadata fixture".into(),
        vendor: "Test".into(),
        version: "1".into(),
        format: PluginFormat::Clap,
        path,
        audio_inputs: 2,
        audio_outputs: 2,
        is_instrument: false,
        categories: vec![],
        scan_status: PluginScanStatus::Loadable,
    };
    let native = ExternalPluginState::new(
        descriptor.clone(),
        ExternalPluginSandboxMode::Isolated,
        vec![1, 2, 3],
    );
    let state = cx.new(|cx| {
        let mut app = PlayerApp::new();
        // Stored session updates are unrelated to this isolated widget fixture.
        app.plugin_state.update_state = Default::default();
        app.plugin_state.graph_state.draft = None;
        app.plugin_state.graph = PluginGraph::with_default_rack();
        app.plugin_state
            .graph
            .insert_plugin_settings(
                2,
                PluginSettings::External {
                    state: native.clone(),
                },
            )
            .unwrap();
        let layout = cx.new(|_| LayoutState::default());
        let mut player = PlayerHandle::new(Player::new());
        // This is a widget/rejection test, not a synthetic native acceptance.
        player.qa_disconnect_worker_for_test();
        AppState {
            app,
            player,
            layout,
        }
    });
    let instance_id = state
        .read(cx)
        .app
        .plugin_state
        .graph
        .get_plugin(2)
        .unwrap()
        .id;
    let mut unbounded = Parameter::new_float(
        "unbounded",
        "Exact value without native bounds",
        0.0,
        0.0,
        1.0,
    );
    unbounded.min_value = None;
    unbounded.max_value = None;
    let mut short_choices = Parameter::new_int("mode", "Mode", -1, -2, 0);
    short_choices.choices = ["Left", "Center", "Right"]
        .into_iter()
        .enumerate()
        .map(|(i, label)| ParameterChoice {
            label: label.into(),
            value: ParameterValue::Int(i as i32 - 2),
        })
        .collect();
    let mut menu_choices = Parameter::new_float("shape", "Shape", 0.25, 0.0, 1.0);
    menu_choices.choices = [
        "Gentle knee",
        "Smooth curve",
        "Strong compression",
        "Hard limiting",
    ]
    .into_iter()
    .enumerate()
    .map(|(i, label)| ParameterChoice {
        label: label.into(),
        value: ParameterValue::Float(i as f32 / 3.0),
    })
    .collect();
    let mut readonly = Parameter::new_float("meter", "Native meter", 0.0, -100.0, 0.0);
    readonly.read_only = true;
    let parameters = vec![
        Parameter::new_float(
            "gain",
            "A long native gain parameter name that must fit the narrow viewport",
            1.0,
            0.0,
            2.0,
        ),
        Parameter::new_bool("enabled", "Enabled", false),
        unbounded,
        Parameter::new_int("large", "Large integer", 0, 0, i32::MAX),
        Parameter::new_float("invalid", "Invalid native readback", 1.0, 0.0, 2.0),
        Parameter::new_float("missing", "Missing native readback", 1.0, 0.0, 2.0),
        Parameter::new_int("fixed", "Fixed native range", 3, 3, 3),
        short_choices,
        menu_choices,
        readonly,
    ];
    let values = [
        ("gain", ParameterValue::Float(0.25)),
        ("enabled", ParameterValue::Bool(false)),
        ("unbounded", ParameterValue::Float(2.5)),
        ("large", ParameterValue::Int(i32::MAX)),
        ("invalid", ParameterValue::Float(f32::NAN)),
        ("fixed", ParameterValue::Int(3)),
        ("mode", ParameterValue::Int(-1)),
        ("shape", ParameterValue::Float(1.0 / 3.0)),
        ("meter", ParameterValue::Float(-12.5)),
    ]
    .into_iter()
    .map(|(id, value)| (ParameterId::from(id), value))
    .collect::<HashMap<_, _>>();
    let observation = cx.observe(&state, |_, _, cx| cx.notify());
    ExternalControlsView {
        _observation: observation,
        _fixture: fixture,
        state,
        data: Arc::new(ExternalPluginEditorData {
            plugin_instance_id: Some(instance_id),
            descriptor,
            parameters,
            parameter_values: values,
            native_state: Ok(native),
        }),
    }
}

fn bounds(cx: &mut VisualTestContext, window_id: u64, selector: &str) -> Bounds<Pixels> {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    registry::lookup(window_id, selector)
        .unwrap_or_else(|| panic!("missing painted control {selector}"))
}

fn click(cx: &mut VisualTestContext, position: Point<Pixels>) {
    cx.simulate_mouse_move(position, None, Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

fn inspect(
    window: WindowHandle<ExternalControlsView>,
    cx: &mut VisualTestContext,
) -> (u64, serde_json::Value, Option<String>) {
    window
        .update(cx, |view, _, cx| {
            let state = view.state.read(cx);
            (
                state
                    .app
                    .plugin_state
                    .update_state
                    .profile_observation_revision,
                serde_json::to_value(&state.app.plugin_state.graph).unwrap(),
                state
                    .app
                    .ui_state
                    .toast_message
                    .as_ref()
                    .map(|toast| toast.message.clone()),
            )
        })
        .unwrap()
}

#[gpui::test]
fn mounted_external_graph_control_dispatches_by_instance_and_rejects_removed_target(
    cx: &mut TestAppContext,
) {
    let _registry_guard = MOUNTED_REGISTRY_TEST_LOCK.lock().unwrap();
    let window = cx.add_window(|_, cx| create_view(cx));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let (window_id, instance_id, target_node) = window
        .update(&mut cx, |view, window, cx| {
            let instance_id = view.data.plugin_instance_id.unwrap();
            let target = view.state.update(cx, |state, _| {
                let graph = &mut state.app.plugin_state.graph;
                let target = graph
                    .nodes
                    .values()
                    .find(|node| node.plugin.id == instance_id)
                    .unwrap()
                    .id;
                let matrix = graph
                    .node_id_for_role(sotf_audio_player::plugin_graph::NodeRole::Matrix)
                    .unwrap();
                // A parallel program route makes linear-position lookup invalid.
                let source = graph
                    .connections
                    .iter()
                    .find(|edge| edge.to_node == target)
                    .unwrap()
                    .from_node;
                graph.add_connection(source, 0, matrix, 0).unwrap();
                assert!(!graph.is_linear());
                assert!(graph.get_engine_index_by_linear_position(2).is_none());
                assert!(graph.get_engine_index(target).is_some());
                target
            });
            cx.notify();
            (
                window.window_handle().window_id().as_u64(),
                instance_id,
                target,
            )
        })
        .unwrap();
    let selector = format!("external.param.{instance_id}.1.choice.1");
    let (before_revision, before_graph, _) = inspect(window, &mut cx);
    let on = bounds(&mut cx, window_id, &selector);
    click(&mut cx, on.center());
    let (after_revision, after_graph, toast) = inspect(window, &mut cx);
    assert_eq!(after_revision, before_revision + 1);
    assert_eq!(after_graph, before_graph);
    assert!(
        toast
            .unwrap()
            .contains("player command worker is not available")
    );
    window
        .update(&mut cx, |view, _, cx| {
            view.state.update(cx, |state, cx| {
                state.app.plugin_state.graph.remove_node(target_node);
                cx.notify();
            });
        })
        .unwrap();
    let on = bounds(&mut cx, window_id, &selector);
    let (_, removed_graph, _) = inspect(window, &mut cx);
    let enabled = registry::snapshot_for(window_id)
        .into_iter()
        .find(|(key, _)| key == &selector)
        .unwrap()
        .1
        .state
        .enabled;
    assert_eq!(
        enabled,
        Some(false),
        "stale native data must not enable a removed instance"
    );
    click(&mut cx, on.center());
    let (revision, graph, _) = inspect(window, &mut cx);
    assert_eq!(revision, after_revision);
    assert_eq!(graph, removed_graph);
}

#[gpui::test]
fn mounted_external_widgets_fit_narrow_width_and_dispatch_without_optimistic_persistence(
    cx: &mut TestAppContext,
) {
    let _registry_guard = MOUNTED_REGISTRY_TEST_LOCK.lock().unwrap();
    let window = cx.add_window(|_, cx| create_view(cx));
    cx.simulate_window_resize(window.into(), size(px(380.0), px(800.0)));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let (window_id, data) = window
        .update(&mut cx, |view, window, _| {
            (
                window.window_handle().window_id().as_u64(),
                view.data.clone(),
            )
        })
        .unwrap();
    let id = data.plugin_instance_id.unwrap();
    let prefix = format!("external.param.{id}");
    let (before_revision, before, _) = inspect(window, &mut cx);
    for suffix in [
        "0.slider",
        "0.exact",
        "1.choice.0",
        "1.choice.1",
        "2.exact",
        "3.exact",
        "6.exact",
        "7.choice.0",
        "7.choice.2",
        "8.select",
        "9.readonly",
    ] {
        let b = bounds(&mut cx, window_id, &format!("{prefix}.{suffix}"));
        assert!(
            f32::from(b.origin.x) >= 0.0 && f32::from(b.origin.x + b.size.width) <= 380.5,
            "{suffix} overflows the narrow viewport: {b:?}"
        );
    }
    for suffix in [
        "2.slider", "3.slider", "4.slider", "4.exact", "5.slider", "5.exact", "6.slider",
        "7.slider", "7.exact", "8.slider", "8.exact", "9.slider", "9.exact", "9.select",
    ] {
        assert!(
            registry::lookup(window_id, &format!("{prefix}.{suffix}")).is_none(),
            "{suffix} must not fabricate an editable slider/value"
        );
    }
    let readonly = registry::snapshot_for(window_id)
        .into_iter()
        .find(|(key, _)| key == &format!("{prefix}.9.readonly"))
        .unwrap()
        .1;
    assert_eq!(readonly.state.text.as_deref(), Some("-12.50 · Read only"));
    assert_eq!(readonly.state.enabled, Some(false));
    let current_choice = registry::snapshot_for(window_id)
        .into_iter()
        .find(|(key, _)| key == &format!("{prefix}.7.choice.1"))
        .unwrap()
        .1;
    assert_eq!(current_choice.state.selected, Some(true));
    window
        .update(&mut cx, |view, _, cx| {
            assert!(
                view.state
                    .read(cx)
                    .app
                    .plugin_state
                    .graph
                    .get_engine_index_by_linear_position(2)
                    .is_some()
            );
        })
        .unwrap();
    let on = bounds(&mut cx, window_id, &format!("{prefix}.1.choice.1"));
    let enabled = registry::snapshot_for(window_id)
        .into_iter()
        .find(|(key, _)| key == &format!("{prefix}.1.choice.1"))
        .unwrap()
        .1
        .state
        .enabled;
    assert_eq!(
        enabled,
        Some(true),
        "the idle fixture must expose an enabled On choice"
    );
    click(&mut cx, on.center());
    let (revision, _, toast) = inspect(window, &mut cx);
    assert_eq!(
        revision,
        before_revision + 1,
        "the painted On option must dispatch its callback after hover; bounds={on:?}, toast={toast:?}"
    );
    assert!(
        toast
            .unwrap()
            .contains("player command worker is not available")
    );
    let slider = bounds(&mut cx, window_id, &format!("{prefix}.0.slider"));
    let start = slider.center();
    cx.simulate_mouse_move(start, None, Modifiers::default());
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    let end = point(
        slider.origin.x + slider.size.width * 0.75,
        slider.center().y,
    );
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    let (revision, _, _) = inspect(window, &mut cx);
    assert!(revision > before_revision + 1);
    let exact = bounds(&mut cx, window_id, &format!("{prefix}.2.exact"));
    click(&mut cx, exact.center());
    cx.simulate_keystrokes("cmd-a 3 . 1 2 5 enter");
    cx.run_until_parked();
    let (after_revision, _, _) = inspect(window, &mut cx);
    assert!(
        after_revision > revision,
        "the unbounded exact-value field must commit through the actor"
    );
    let short = bounds(&mut cx, window_id, &format!("{prefix}.7.choice.2"));
    click(&mut cx, short.center());
    let (short_revision, _, _) = inspect(window, &mut cx);
    assert!(
        short_revision > after_revision,
        "native labeled buttons must dispatch"
    );
    let menu = bounds(&mut cx, window_id, &format!("{prefix}.8.select"));
    click(&mut cx, menu.center());
    window
        .update(&mut cx, |view, _, cx| {
            assert_eq!(
                view.state
                    .read(cx)
                    .app
                    .plugin_state
                    .external_plugin_ui
                    .choice_menu,
                Some((id, 8))
            );
        })
        .unwrap();
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    let (menu_revision, after, _) = inspect(window, &mut cx);
    assert!(
        menu_revision > short_revision,
        "the native choices dropdown must commit via keyboard"
    );
    window
        .update(&mut cx, |view, _, cx| {
            assert!(
                view.state
                    .read(cx)
                    .app
                    .plugin_state
                    .external_plugin_ui
                    .choice_menu
                    .is_none()
            );
        })
        .unwrap();
    assert_eq!(
        after, before,
        "failed submissions cannot change saved native state or neighbors"
    );
    assert_eq!(
        data.parameter_values[&ParameterId::from("gain")],
        ParameterValue::Float(0.25)
    );
    close_window(window, &mut cx);
}

#[gpui::test]
fn mounted_external_controls_disable_for_capture_failure_and_pending_graph(
    cx: &mut TestAppContext,
) {
    let _registry_guard = MOUNTED_REGISTRY_TEST_LOCK.lock().unwrap();
    let window = cx.add_window(|_, cx| create_view(cx));
    let mut cx = VisualTestContext::from_window(window.into(), cx);
    let (window_id, id) = window
        .update(&mut cx, |view, window, _| {
            (
                window.window_handle().window_id().as_u64(),
                view.data.plugin_instance_id.unwrap(),
            )
        })
        .unwrap();
    for capture_failed in [false, true] {
        window
            .update(&mut cx, |view, _, cx| {
                view.state.update(cx, |state, _| {
                    state.app.plugin_state.update_state.pending_plugin_update = if capture_failed {
                        None
                    } else {
                        Some(PluginUpdateType::Structural)
                    };
                });
                if capture_failed {
                    let mut data = (*view.data).clone();
                    data.native_state = Err("native state capture failed".into());
                    view.data = Arc::new(data);
                }
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();
        let (revision, _, _) = inspect(window, &mut cx);
        for suffix in [
            "0.slider",
            "0.exact",
            "1.choice.1",
            "2.exact",
            "3.exact",
            "6.exact",
            "7.choice.2",
            "8.select",
        ] {
            let selector = format!("external.param.{id}.{suffix}");
            let b = bounds(&mut cx, window_id, &selector);
            let entry = registry::snapshot_for(window_id)
                .into_iter()
                .find(|(key, _)| key == &selector)
                .unwrap()
                .1;
            assert_eq!(
                entry.state.enabled,
                Some(false),
                "{selector} should visibly disable"
            );
            click(&mut cx, b.center());
        }
        assert_eq!(inspect(window, &mut cx).0, revision);
    }
    close_window(window, &mut cx);
}

fn close_window(window: WindowHandle<ExternalControlsView>, cx: &mut VisualTestContext) {
    window
        .update(cx, |_, window, _| window.remove_window())
        .unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
}
