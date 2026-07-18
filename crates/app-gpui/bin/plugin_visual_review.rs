#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("plugin_visual_review requires macOS Metal rendering");
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
#[allow(
    dead_code,
    reason = "the shared asset module also contains desktop preset installation"
)]
#[path = "../main/assets.rs"]
mod assets;

#[cfg(target_os = "macos")]
mod macos {
    use super::assets::Assets;
    use anyhow::{Context as _, Result, anyhow};
    use clap::Parser;
    use gpui::{AppContext as _, VisualTestAppContext, px, size};
    use gpui_ui_kit::accessibility::{AccessibilityExt as _, AccessibilityTree};
    use serde_json::json;
    use sotf_audio_player::{Player, PluginType, ReleaseChannel};
    use sotf_audio_player_gpui::app::player_handle::PlayerHandle;
    use sotf_audio_player_gpui::app::state::plugin::PluginState;
    use sotf_audio_player_gpui::app::state::ui::LayoutState;
    use sotf_audio_player_gpui::app::{App, AppState, InputMode, Screen};
    use sotf_audio_player_gpui::components::plugins::editing::PluginEditingManager as _;
    use sotf_audio_player_gpui::ui::PlayerView;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::sync::Arc;

    const VIEWPORTS: [(u32, u32); 2] = [(700, 900), (1600, 1000)];

    #[derive(Debug, Parser)]
    #[command(about = "Capture compact and wide screenshots for every app plugin")]
    struct Args {
        /// Directory for PNG captures, the isolated QA state, and manifest.json.
        #[arg(long)]
        output: PathBuf,

        /// Capture only this plugin display name (case-insensitive).
        #[arg(long)]
        plugin: Option<String>,
    }

    fn slug(value: &str) -> String {
        let mut output = String::with_capacity(value.len());
        let mut pending_separator = false;
        for character in value.chars() {
            if character.is_ascii_alphanumeric() {
                if pending_separator && !output.is_empty() {
                    output.push('-');
                }
                output.push(character.to_ascii_lowercase());
                pending_separator = false;
            } else {
                pending_separator = true;
            }
        }
        output
    }

    fn load_fonts(cx: &mut gpui::App) -> Result<()> {
        let font_data = [
            "fonts/B612-Regular.ttf",
            "fonts/B612-Italic.ttf",
            "fonts/B612-Bold.ttf",
            "fonts/B612-BoldItalic.ttf",
        ]
        .into_iter()
        .map(|path| {
            Assets::get(path)
                .map(|file| file.data)
                .ok_or_else(|| anyhow!("missing embedded font {path}"))
        })
        .collect::<Result<Vec<_>>>()?;
        cx.text_system()
            .add_fonts(font_data)
            .context("loading embedded B612 fonts")
    }

    fn make_app_state(cx: &mut gpui::App) -> gpui::Entity<AppState> {
        cx.new(|cx| {
            let mut app = App::new();
            app.tutorial.completed = true;
            app.ui_state.startup_db_check_done = true;
            app.ui_state.release_channel = ReleaseChannel::Alpha;
            app.ui_state.current_screen = Screen::Studio;
            app.ui_state.input_mode = InputMode::Normal;
            app.library_view.loading_initial_data = false;

            AppState {
                app,
                layout: cx.new(|_| LayoutState::default()),
                player: PlayerHandle::new(Player::new()),
            }
        })
    }

    fn capture_viewport(
        cx: &mut VisualTestAppContext,
        output: &Path,
        width: u32,
        height: u32,
        plugin_types: &[PluginType],
        manifest: &mut Vec<serde_json::Value>,
    ) -> Result<()> {
        for (ordinal, requested_type) in plugin_types.iter().cloned().enumerate() {
            let app_state = cx.update(make_app_state);
            let requested_name = requested_type.name().to_string();
            let rendered_name = cx.update(|cx| {
                app_state.update(cx, |state, cx| {
                    // `PlayerView` normally syncs these through a deferred
                    // geometry update. Pin them to the requested off-screen
                    // viewport so plugin breakpoints cannot observe the prior
                    // window's dimensions during a deterministic capture.
                    state.app.ui_state.window_width = width as f32;
                    state.app.ui_state.window_height = height as f32;
                    state.app.plugin_state = PluginState::new();
                    state.app.add_plugin(&requested_type);
                    let selected = state.app.plugin_state.selected_plugin_index;
                    state.app.plugin_state.editing_plugin_index = Some(selected);
                    state.app.plugin_state.plugin_param_selection = 0;
                    state.app.plugin_state.update_state.pending_plugin_update = None;
                    state.app.ui_state.current_screen = Screen::Studio;
                    state.app.tutorial.current_hint = None;
                    cx.notify();
                    state
                        .app
                        .plugin_state
                        .graph
                        .get_plugin(selected)
                        .map(|plugin| plugin.plugin_type().name().to_string())
                })
            });
            let rendered_name = rendered_name.ok_or_else(|| {
                anyhow!("{requested_name} was not inserted into the visual-review rack")
            })?;

            // GPUI's normal frame path reuses unchanged paint ranges because
            // the on-screen Metal drawable retains its previous contents.
            // `render_to_image` starts from a cleared texture, so capture each
            // plugin in a new window whose first scene contains every region.
            let window = cx
                .open_offscreen_window(size(px(width as f32), px(height as f32)), {
                    let app_state = app_state.clone();
                    move |_, cx| cx.new(|cx| PlayerView::new_for_visual_qa(app_state, cx))
                })
                .with_context(|| format!("opening {requested_name} at {width}x{height}"))?;
            // Let the platform finish its initial frame so `refresh()` below
            // runs from DrawPhase::None and invalidates the entire view tree.
            cx.run_until_parked();
            cx.update(|cx| cx.global_mut::<AccessibilityTree>().clear());
            let (image, accessibility) = cx.update_window(window.into(), |_, window, cx| {
                window.refresh();
                let _ = window.draw(cx);
                let image = window.render_to_image()?;
                let snapshot = cx
                    .accessibility_tree()
                    .ok_or_else(|| anyhow!("accessibility tree is not initialized"))?
                    .to_bridge_snapshot();
                let blocking = snapshot
                    .blocking_entries()
                    .map(|node| node.element_key())
                    .collect::<Vec<_>>();
                if !blocking.is_empty() {
                    return Err(anyhow!(
                        "{requested_name} at {width}x{height} has unnamed accessibility nodes: {blocking:?}"
                    ));
                }
                let focusable_nodes = snapshot
                    .nodes
                    .iter()
                    .filter(|node| node.is_focusable_for_native_adapter())
                    .map(|node| {
                        json!({
                            "element": node.element_key(),
                            "role": node.role_name,
                            "label": node.label,
                            "value": {
                                "now": node.value.now,
                                "min": node.value.min,
                                "max": node.value.max,
                                "text": node.value.text,
                            },
                            "actions": node
                                .native_adapter_actions()
                                .iter()
                                .map(|action| action.as_str())
                                .collect::<Vec<_>>(),
                        })
                    })
                    .collect::<Vec<_>>();
                if focusable_nodes.is_empty() {
                    return Err(anyhow!(
                        "{requested_name} at {width}x{height} exposes no focusable accessibility nodes"
                    ));
                }
                Ok((
                    image,
                    json!({
                        "node_count": snapshot.nodes.len(),
                        "focusable_node_count": focusable_nodes.len(),
                        "all_nodes_named": true,
                        "focusable_nodes": focusable_nodes,
                    }),
                ))
            })??;
            let filename = format!(
                "{:02}-{}-{}x{}.png",
                ordinal + 1,
                slug(&requested_name),
                width,
                height
            );
            let path = output.join(&filename);
            image
                .save(&path)
                .with_context(|| format!("saving {}", path.display()))?;
            manifest.push(json!({
                "ordinal": ordinal + 1,
                "requested_plugin": requested_name,
                "rendered_plugin": rendered_name,
                "viewport": { "width": width, "height": height },
                "file": filename,
                "accessibility": accessibility,
            }));

            let empty_root = cx.update_window(window.into(), |_, window, cx| {
                let empty_root = window.replace_root(cx, |_, _| gpui::Empty);
                let _ = window.draw(cx);
                empty_root
            })?;
            drop(empty_root);
            cx.update_window(window.into(), |_, window, _| window.remove_window())?;
            cx.run_until_parked();
            drop(app_state);
            cx.run_until_parked();
        }

        cx.advance_clock(std::time::Duration::from_millis(500));
        cx.run_until_parked();
        Ok(())
    }

    pub fn run() -> Result<()> {
        let args = Args::parse();
        std::fs::create_dir_all(&args.output)
            .with_context(|| format!("creating {}", args.output.display()))?;
        let state_dir = args.output.join("qa-state");
        std::fs::create_dir_all(&state_dir)
            .with_context(|| format!("creating {}", state_dir.display()))?;
        sotf_audio_player::config::set_config_dir_override(state_dir);

        let platform = Rc::new(gpui_macos::MacPlatform::new(false));
        let mut cx = VisualTestAppContext::with_asset_source(platform, Arc::new(Assets));
        let ref_counts = cx.update(|cx| cx.ref_counts_drop_handle());
        cx.update(|cx| {
            cx.set_global(gpui_design::DesignSystemState::new());
            cx.set_global(AccessibilityTree::new());
            load_fonts(cx)
        })?;

        let plugin_types = if let Some(requested) = args.plugin.as_deref() {
            let plugin_type = PluginType::all()
                .into_iter()
                .find(|plugin_type| plugin_type.name().eq_ignore_ascii_case(requested))
                .ok_or_else(|| anyhow!("unknown plugin display name '{requested}'"))?;
            vec![plugin_type]
        } else {
            PluginType::all()
        };

        let mut manifest = Vec::new();
        for (width, height) in VIEWPORTS {
            capture_viewport(
                &mut cx,
                &args.output,
                width,
                height,
                &plugin_types,
                &mut manifest,
            )?;
        }

        let manifest_path = args.output.join("manifest.json");
        std::fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "capture_count": manifest.len(),
                "captures": manifest,
            }))?,
        )
        .with_context(|| format!("writing {}", manifest_path.display()))?;
        cx.update(|cx| cx.shutdown());
        cx.run_until_parked();
        drop(cx);
        drop(ref_counts);
        println!(
            "captured {} plugin views in {}",
            manifest.len(),
            args.output.display()
        );
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    macos::run()
}
