use super::super::commands::{DevCommand, DevQueryReply, DevReply};
use super::super::{performance, queries, registry};
use super::get::get_accessibility;
use super::get::get_health;
use super::get::get_query;
use super::get::get_snapshot;
use super::misc::POLL_INTERVAL;
use super::misc::http_response;
use super::misc::list_elements_json;
use super::misc::resolve_action_name;
use super::misc::split_path_query;
use super::post::post_action;
use super::post::post_click;
use super::post::post_drag;
use super::post::post_hover;
use super::post::post_input;
use super::post::post_key;
use super::post::post_qa_headphone_discovery_fixture;
use super::post::post_qa_recording_fake_capture;
use super::post::post_qa_room_eq;
use super::post::post_qa_room_eq_export_json;
use super::post::post_qa_room_eq_ui_fixture;
use super::post::post_qa_seed;
use super::post::post_qa_spinorama_discovery_fixture;
use super::post::post_qa_ui_environment;
use super::post::post_quit;
use super::post::post_resize;
use super::post::post_screenshot;
use super::post::post_scroll;
use super::post::post_text;
use super::qa::qa_headphone_discovery_fixture;
use super::qa::qa_recording_fake_capture;
use super::qa::qa_room_eq;
use super::qa::qa_room_eq_export_json;
use super::qa::qa_room_eq_ui_fixture;
use super::qa::qa_seed;
use super::qa::qa_spinorama_discovery_fixture;
use super::qa::qa_ui_environment;
use super::types::HttpRequest;
use super::with::health_payload;
use super::with::{with_app_state, with_player_view};
use crate::app::{InputMode, MetadataEditorState, Screen, SettingsTab};
use crate::components::plugins::level_meters::LevelMeterManager;
use crate::ui::PlayerView;
use anyhow::{Context as _, Result, anyhow};
use gpui::{
    AnyWindowHandle, App, AppContext, AsyncApp, Bounds, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, Point, ScrollDelta, ScrollWheelEvent, TouchPhase,
    WindowBounds, WindowOptions, point, px, size,
};
use gpui_ui_kit::accessibility::AccessibilityExt as _;
use serde_json::Value;
use sotf_dev_api::{CoordinateInput, PointerPhase};
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

static SNAPSHOT_REVISION: std::sync::OnceLock<std::sync::Mutex<(String, u64)>> =
    std::sync::OnceLock::new();

pub(super) async fn consume_commands(
    rx: Receiver<DevCommand>,
    window: AnyWindowHandle,
    cx: &mut AsyncApp,
) {
    loop {
        // Drain any commands the listener thread has queued.
        loop {
            match rx.try_recv() {
                Ok(cmd) => process_command(cmd, window, cx),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            }
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

pub(super) fn process_command(cmd: DevCommand, window: AnyWindowHandle, cx: &mut AsyncApp) {
    match cmd {
        DevCommand::Action {
            name,
            payload,
            reply,
        } => {
            let result = cx.update(|cx| dispatch_action(&name, payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            // Best effort — handler thread may have already given up.
            let _ = reply.send(dev_reply);
        }
        DevCommand::Query { path, reply } => {
            let result = cx.update(|cx| queries::resolve(&path, window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Key { keystroke, reply } => {
            let result = cx.update(|cx| dispatch_key(&keystroke, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Text { text, reply } => {
            let result = cx.update(|cx| dispatch_text(&text, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Input { input, reply } => {
            let result = cx.update(|cx| dispatch_coordinate_input(input, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Click { selector, reply } => {
            let result = cx.update(|cx| dispatch_click(&selector, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Hover { selector, reply } => {
            let result = cx.update(|cx| dispatch_hover(&selector, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Drag {
            source,
            target,
            reply,
        } => {
            let result = cx.update(|cx| dispatch_drag(&source, &target, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Scroll {
            selector,
            delta_y,
            reply,
        } => {
            let result = cx.update(|cx| dispatch_scroll(&selector, delta_y, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Resize {
            width,
            height,
            reply,
        } => {
            let result = cx.update(|cx| dispatch_resize(width, height, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Screenshot { name, reply } => {
            let reply_on_error = reply.clone();
            let result = cx.update(|cx| dispatch_screenshot(&name, window, reply, cx));
            if let Err(e) = result {
                let _ = reply_on_error.send(DevReply::err(format!("{e:#}")));
            }
        }
        DevCommand::Accessibility { reply } => {
            let result = cx.update(|cx| accessibility_payload(window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Snapshot { reply } => {
            let result = cx.update(|cx| snapshot_payload(window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Health { reply } => {
            let result = cx.update(|cx| health_payload(window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::Quit { reply } => {
            let result = cx.update(|cx| -> Result<()> {
                cx.quit();
                Ok(())
            });
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaSeed { payload, reply } => {
            let result = cx.update(|cx| qa_seed(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaUiEnvironment { payload, reply } => {
            let result = cx.update(|cx| qa_ui_environment(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaRecordingFakeCapture { payload, reply } => {
            let result = cx.update(|cx| qa_recording_fake_capture(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaHeadphoneDiscoveryFixture { payload, reply } => {
            let result = cx.update(|cx| qa_headphone_discovery_fixture(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaSpinoramaDiscoveryFixture { payload, reply } => {
            let result = cx.update(|cx| qa_spinorama_discovery_fixture(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaRoomEq { payload, reply } => {
            let result = cx.update(|cx| qa_room_eq(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaRoomEqUiFixture { payload, reply } => {
            let result = cx.update(|cx| qa_room_eq_ui_fixture(payload, window, cx));
            let dev_reply = match result {
                Ok(()) => DevReply::ok(),
                Err(e) => DevReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
        DevCommand::QaRoomEqExportJson { payload, reply } => {
            let result = cx.update(|cx| qa_room_eq_export_json(payload, window, cx));
            let dev_reply = match result {
                Ok(value) => DevQueryReply::ok(value),
                Err(e) => DevQueryReply::err(format!("{e:#}")),
            };
            let _ = reply.send(dev_reply);
        }
    }
}

pub(super) fn dispatch_key(
    keystroke_str: &str,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let keystroke = Keystroke::parse(keystroke_str)
        .map_err(|e| anyhow!("invalid keystroke `{keystroke_str}`: {e:?}"))?;
    window
        .update(cx, |_view, window, cx| {
            window.dispatch_keystroke(keystroke, cx);
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

/// Deliver a text string through the same key-dispatch mechanism as physical
/// typing, while keeping the HTTP protocol to one request per `type` command.
pub(super) fn dispatch_text(text: &str, window: AnyWindowHandle, cx: &mut App) -> Result<()> {
    for character in text.chars() {
        let keystroke = match character {
            ' ' => "space".to_owned(),
            '\t' => "tab".to_owned(),
            '\n' => "enter".to_owned(),
            _ => character.to_string(),
        };
        dispatch_key(&keystroke, window, cx)?;
    }
    Ok(())
}

pub(super) fn dispatch_click(selector: &str, window: AnyWindowHandle, cx: &mut App) -> Result<()> {
    let bounds = registry::lookup(window.window_id().as_u64(), selector)
        .ok_or_else(|| anyhow!("no tracked element for selector `{selector}` (was it painted?)"))?;
    let position: Point<gpui::Pixels> = bounds.center();
    window
        .update(cx, |_view, window, cx| {
            let modifiers = Default::default();
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    pressed_button: None,
                    modifiers,
                }),
                cx,
            );
            let down = MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
                first_mouse: false,
            };
            let up = MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers,
                click_count: 1,
            };
            window.dispatch_event(PlatformInput::MouseDown(down), cx);
            window.dispatch_event(PlatformInput::MouseUp(up), cx);
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

pub(super) fn dispatch_coordinate_input(
    input: CoordinateInput,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let viewport_revision = match &input {
        CoordinateInput::Pointer {
            viewport_revision, ..
        }
        | CoordinateInput::Scroll {
            viewport_revision, ..
        } => *viewport_revision,
        CoordinateInput::Touch { .. } => return Err(anyhow!("touch input is not supported")),
        CoordinateInput::Remote { .. } => return Err(anyhow!("remote input is not supported")),
    };
    let current_revision = current_snapshot_revision();
    if viewport_revision != current_revision {
        return Err(anyhow!(
            "stale viewport revision {viewport_revision}; current revision is {current_revision}"
        ));
    }

    window
        .update(cx, |_view, window, cx| {
            let bounds = window.bounds();
            let width = f32::from(bounds.size.width) as f64;
            let height = f32::from(bounds.size.height) as f64;
            let validate_point = |x: f64, y: f64| -> Result<Point<gpui::Pixels>> {
                if !x.is_finite()
                    || !y.is_finite()
                    || !(0.0..=width).contains(&x)
                    || !(0.0..=height).contains(&y)
                {
                    return Err(anyhow!(
                        "coordinate ({x}, {y}) is outside viewport {width}x{height}"
                    ));
                }
                Ok(point(px(x as f32), px(y as f32)))
            };
            let modifiers = Default::default();

            match input {
                CoordinateInput::Pointer {
                    phase,
                    x,
                    y,
                    button,
                    ..
                } => {
                    let position = validate_point(x, y)?;
                    let button = match button {
                        0 => MouseButton::Left,
                        1 => MouseButton::Right,
                        2 => MouseButton::Middle,
                        other => return Err(anyhow!("unsupported pointer button {other}")),
                    };
                    match phase {
                        PointerPhase::Move => window.dispatch_event(
                            PlatformInput::MouseMove(MouseMoveEvent {
                                position,
                                pressed_button: None,
                                modifiers,
                            }),
                            cx,
                        ),
                        PointerPhase::Down => window.dispatch_event(
                            PlatformInput::MouseDown(MouseDownEvent {
                                button,
                                position,
                                modifiers,
                                click_count: 1,
                                first_mouse: false,
                            }),
                            cx,
                        ),
                        PointerPhase::Up => window.dispatch_event(
                            PlatformInput::MouseUp(MouseUpEvent {
                                button,
                                position,
                                modifiers,
                                click_count: 1,
                            }),
                            cx,
                        ),
                    };
                }
                CoordinateInput::Scroll {
                    delta_x,
                    delta_y,
                    x,
                    y,
                    ..
                } => {
                    let position = validate_point(x, y)?;
                    if !delta_x.is_finite()
                        || !delta_y.is_finite()
                        || delta_x.abs() > 4096.0
                        || delta_y.abs() > 4096.0
                    {
                        return Err(anyhow!("scroll delta is non-finite or out of bounds"));
                    }
                    window.dispatch_event(
                        PlatformInput::ScrollWheel(ScrollWheelEvent {
                            position,
                            delta: ScrollDelta::Pixels(point(
                                px(delta_x as f32),
                                px(delta_y as f32),
                            )),
                            modifiers,
                            touch_phase: TouchPhase::Moved,
                        }),
                        cx,
                    );
                }
                CoordinateInput::Touch { .. } | CoordinateInput::Remote { .. } => unreachable!(),
            }
            Ok(())
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))??;
    Ok(())
}

pub(super) fn dispatch_hover(selector: &str, window: AnyWindowHandle, cx: &mut App) -> Result<()> {
    let bounds = registry::lookup(window.window_id().as_u64(), selector)
        .ok_or_else(|| anyhow!("no tracked element for selector `{selector}` (was it painted?)"))?;
    let position: Point<gpui::Pixels> = bounds.center();
    window
        .update(cx, |_view, window, cx| {
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    pressed_button: None,
                    modifiers: Default::default(),
                }),
                cx,
            );
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

pub(super) fn dispatch_drag(
    source: &str,
    target: &str,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let source_position: Point<gpui::Pixels> =
        registry::lookup(window.window_id().as_u64(), source)
            .ok_or_else(|| anyhow!("no tracked element for selector `{source}` (was it painted?)"))?
            .center();
    let target_position: Point<gpui::Pixels> =
        registry::lookup(window.window_id().as_u64(), target)
            .ok_or_else(|| anyhow!("no tracked element for selector `{target}` (was it painted?)"))?
            .center();
    let midpoint = point(
        source_position.x + (target_position.x - source_position.x) * 0.5,
        source_position.y + (target_position.y - source_position.y) * 0.5,
    );
    window
        .update(cx, |_view, window, cx| {
            let modifiers = Default::default();
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: source_position,
                    pressed_button: None,
                    modifiers,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: source_position,
                    modifiers,
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: midpoint,
                    pressed_button: Some(MouseButton::Left),
                    modifiers,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: target_position,
                    pressed_button: Some(MouseButton::Left),
                    modifiers,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: target_position,
                    modifiers,
                    click_count: 1,
                }),
                cx,
            );
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

pub(super) fn dispatch_scroll(
    selector: &str,
    delta_y: f32,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    let position: Point<gpui::Pixels> = registry::lookup(window.window_id().as_u64(), selector)
        .ok_or_else(|| anyhow!("no tracked element for selector `{selector}` (was it painted?)"))?
        .center();
    window
        .update(cx, |_view, window, cx| {
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Pixels(point(px(0.0), px(delta_y))),
                    modifiers: Default::default(),
                    touch_phase: TouchPhase::Moved,
                }),
                cx,
            );
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

pub(super) fn dispatch_resize(
    width: f32,
    height: f32,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    window
        .update(cx, |_view, window, _cx| {
            window.resize(size(px(width), px(height)));
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

pub(super) fn dispatch_screenshot(
    name: &str,
    window: AnyWindowHandle,
    reply: mpsc::SyncSender<DevReply>,
    cx: &mut App,
) -> Result<()> {
    let qa_dir = std::env::var_os("SOTF_QA_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("screenshots require an isolated SOTF_QA_DIR"))?;
    let output_dir = qa_dir.join("screenshots");
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("creating screenshot directory {}", output_dir.display()))?;
    let output = output_dir.join(format!("{name}.png"));

    let state = with_player_view(window, cx, |view, _cx| Ok(view.state.clone()))?;
    let viewport_size = window
        .update(cx, |_view, window, _cx| window.viewport_size())
        .map_err(|e| anyhow!("reading screenshot viewport failed: {e:#}"))?;

    // GPUI's on-screen drawable retains unchanged paint ranges, while
    // `render_to_image` starts from a cleared texture. Capture a fresh first
    // scene in an off-screen window so every unchanged region is materialized.
    let capture_window: AnyWindowHandle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(-10_000.0), px(-10_000.0)),
                    size: viewport_size,
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            move |_window, cx| cx.new(|cx| PlayerView::new_for_visual_qa(state, cx)),
        )
        .context("opening off-screen screenshot window")?
        .into();
    let initial_image = capture_window
        .update(cx, |_view, window, _cx| window.render_to_image())
        .map_err(|e| anyhow!("reading initial off-screen scene failed: {e:#}"))
        .and_then(|result| result)
        .with_context(|| format!("reading initial scene for {}", output.display()))?;
    let initial_elements = registry::snapshot_for(capture_window.window_id().as_u64());

    // Keep the hidden capture window registered until normal app shutdown.
    // Removing a freshly rendered macOS window leaves queued platform paint
    // callbacks that GPUI reports as `window not found`. These views have no
    // runtime services, remain hidden, and the QA suite creates only a small,
    // bounded number of them.

    cx.defer(move |cx| {
        let result = capture_window
            .update(cx, |_view, window, cx| {
                window.refresh();
                window.draw(cx).clear();
                let mut image = window.render_to_image()?;
                let refreshed_elements =
                    registry::snapshot_for(window.window_handle().window_id().as_u64());
                merge_missing_tracked_elements(
                    &mut image,
                    &initial_image,
                    viewport_size,
                    &initial_elements,
                    &refreshed_elements,
                );
                image.save_with_format(&output, image::ImageFormat::Png)?;
                Ok::<(), anyhow::Error>(())
            })
            .map_err(|e| anyhow!("off-screen screenshot update failed: {e:#}"))
            .and_then(|result| result)
            .with_context(|| format!("capturing screenshot {}", output.display()));
        let dev_reply = match result {
            Ok(()) => DevReply::ok(),
            Err(error) => DevReply::err(format!("{error:#}")),
        };
        let _ = reply.send(dev_reply);
    });
    Ok(())
}

pub fn merge_missing_tracked_elements(
    refreshed: &mut image::RgbaImage,
    initial: &image::RgbaImage,
    viewport_size: gpui::Size<gpui::Pixels>,
    initial_elements: &[(String, registry::TrackedElement)],
    refreshed_elements: &[(String, registry::TrackedElement)],
) {
    if refreshed.dimensions() != initial.dimensions() {
        return;
    }
    let viewport_width = f32::from(viewport_size.width);
    let viewport_height = f32::from(viewport_size.height);
    if viewport_width <= 0.0 || viewport_height <= 0.0 {
        return;
    }
    let scale_x = refreshed.width() as f32 / viewport_width;
    let scale_y = refreshed.height() as f32 / viewport_height;

    for (selector, element) in initial_elements {
        if refreshed_elements
            .iter()
            .any(|(refreshed_selector, _)| refreshed_selector == selector)
        {
            continue;
        }
        let bounds = element.bounds;
        let x_start = (f32::from(bounds.origin.x) * scale_x)
            .floor()
            .clamp(0.0, refreshed.width() as f32) as u32;
        let y_start = (f32::from(bounds.origin.y) * scale_y)
            .floor()
            .clamp(0.0, refreshed.height() as f32) as u32;
        let x_end = ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * scale_x)
            .ceil()
            .clamp(0.0, refreshed.width() as f32) as u32;
        let y_end = ((f32::from(bounds.origin.y) + f32::from(bounds.size.height)) * scale_y)
            .ceil()
            .clamp(0.0, refreshed.height() as f32) as u32;

        for y in y_start..y_end {
            for x in x_start..x_end {
                refreshed.put_pixel(x, y, *initial.get_pixel(x, y));
            }
        }
    }
}

fn accessibility_payload(window: AnyWindowHandle, cx: &mut App) -> Result<Value> {
    window
        .update(cx, |_view, window, cx| {
            let focused_element = window.focused_element_id(cx).map(|id| id.to_string());
            let snapshot = cx
                .accessibility_tree()
                .ok_or_else(|| anyhow!("accessibility tree is not initialized"))?
                .to_bridge_snapshot_for_window(window, cx);
            let nodes = snapshot
                .nodes
                .iter()
                .map(|node| {
                    serde_json::json!({
                        "element": node.element_key(),
                        "role": node.role_name,
                        "label": node.label,
                        "value": {
                            "now": node.value.now,
                            "min": node.value.min,
                            "max": node.value.max,
                            "text": node.value.text,
                        },
                        "actions": node.native_adapter_actions()
                            .iter()
                            .map(|action| action.as_str())
                            .collect::<Vec<_>>(),
                        "focusable": node.is_focusable_for_native_adapter(),
                        "focused": node.focused,
                    })
                })
                .collect::<Vec<_>>();
            Ok::<_, anyhow::Error>(serde_json::json!({
                "node_count": nodes.len(),
                "focusable_node_count": nodes
                    .iter()
                    .filter(|node| node.get("focusable") == Some(&Value::Bool(true)))
                    .count(),
                "focused_element": focused_element,
                "nodes": nodes,
            }))
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?
}

fn snapshot_payload(window: AnyWindowHandle, cx: &mut App) -> Result<Value> {
    let screen = queries::resolve("screen.focused", window, cx).unwrap_or(Value::Null);
    let input_mode = queries::resolve("input_mode", window, cx).unwrap_or(Value::Null);
    let playback = serde_json::json!({
        "volume": queries::resolve("playback.volume", window, cx).unwrap_or(Value::Null),
        "is_playing": queries::resolve("playback.is_playing", window, cx).unwrap_or(Value::Null),
        "muted": queries::resolve("playback.muted", window, cx).unwrap_or(Value::Null),
    });
    let queue = serde_json::json!({
        "length": queries::resolve("queue.length", window, cx).unwrap_or(Value::Null),
        "current_index": queries::resolve("queue.current_index", window, cx).unwrap_or(Value::Null),
    });
    let library = serde_json::json!({
        "album_count": queries::resolve("library.album_count", window, cx).unwrap_or(Value::Null),
        "track_count": queries::resolve("library.track_count", window, cx).unwrap_or(Value::Null),
    });
    let metadata_editor_open =
        queries::resolve("metadata.editor_open", window, cx).unwrap_or(Value::Null);
    let metadata_dialog_open = metadata_editor_open.as_bool() == Some(true);
    let accessibility_value = accessibility_payload(window, cx)
        .unwrap_or_else(|error| serde_json::json!({"unavailable": error.to_string()}));
    let tracked_elements = registry::snapshot_for(window.window_id().as_u64())
        .into_iter()
        .map(|(selector, element)| {
            let bounds = element.bounds;
            let width = f32::from(bounds.size.width) as f64;
            let height = f32::from(bounds.size.height) as f64;
            sotf_dev_api::TrackedElement {
                selector,
                bounds: sotf_dev_api::Rect {
                    x: f32::from(bounds.origin.x) as f64,
                    y: f32::from(bounds.origin.y) as f64,
                    width,
                    height,
                },
                visible: width > 0.0 && height > 0.0,
                enabled: element.state.enabled.unwrap_or(true),
                selected: element.state.selected.unwrap_or(false),
                expanded: element.state.expanded.unwrap_or(false),
            }
        })
        .collect::<Vec<_>>();
    let nodes = accessibility_value
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut roles = std::collections::BTreeMap::new();
    for node in &nodes {
        if let Some(role) = node.get("role").and_then(Value::as_str) {
            *roles.entry(role.to_owned()).or_default() += 1;
        }
    }
    let focused_id = accessibility_value
        .get("focused_element")
        .or_else(|| accessibility_value.get("focused_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let state = serde_json::json!({
        "screen": screen,
        "input_mode": input_mode,
        "playback": playback,
        "queue": queue,
        "library": library,
        "dialogs": {"metadata_editor": metadata_editor_open},
        "tracked_selectors": tracked_elements.iter().map(|element| &element.selector).collect::<Vec<_>>(),
        "accessibility": accessibility_value,
    });
    let mut snapshot = sotf_dev_api::Snapshot::new("desktop-gpui", 0, state)?;
    let revision = snapshot_revision(&snapshot.state_hash);
    snapshot.state_revision = revision;
    snapshot.render_revision = Some(revision);
    snapshot.accessibility_revision = Some(revision);
    snapshot.screen = snapshot
        .state
        .get("screen")
        .and_then(Value::as_str)
        .map(str::to_owned);
    snapshot.mode = snapshot
        .state
        .get("input_mode")
        .and_then(Value::as_str)
        .map(str::to_owned);
    snapshot.dialogs = if metadata_dialog_open {
        vec!["metadata_editor".into()]
    } else {
        vec![]
    };
    snapshot.tracked_elements = tracked_elements;
    snapshot.accessibility = sotf_dev_api::AccessibilitySnapshot {
        focused_id,
        node_count: nodes.len(),
        roles,
        revision,
    };
    serde_json::to_value(snapshot).map_err(anyhow::Error::from)
}

fn snapshot_revision(state_hash: &str) -> u64 {
    let revision = SNAPSHOT_REVISION.get_or_init(|| std::sync::Mutex::new((String::new(), 0)));
    let mut revision = revision
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if revision.0 != state_hash {
        revision.0 = state_hash.to_owned();
        revision.1 = revision.1.saturating_add(1);
    }
    revision.1
}

fn current_snapshot_revision() -> u64 {
    SNAPSHOT_REVISION
        .get()
        .map(|revision| {
            revision
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .1
        })
        .unwrap_or(0)
}

pub(super) fn dispatch_action(
    name: &str,
    payload: Option<Value>,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<()> {
    if matches!(name, "QaPerformanceStart" | "QaPerformanceStop") {
        let (window_id, update_frame_count, player_view) = window
            .update(cx, |any_view, window, cx| {
                let entity = any_view
                    .downcast::<PlayerView>()
                    .map_err(|_| anyhow!("root view is not PlayerView"))?;
                Ok::<_, anyhow::Error>((
                    window.window_handle().window_id(),
                    entity.read(cx).qa_update_frame_count(),
                    entity.clone(),
                ))
            })
            .map_err(|e| anyhow!("window.update failed: {e:#}"))??;

        match name {
            "QaPerformanceStart" => {
                let playback_clock_running = performance::start(window_id, update_frame_count)?;
                player_view.update(cx, |view, cx| {
                    view.qa_start_performance_playback_clock(playback_clock_running, cx);
                });
            }
            "QaPerformanceStop" => performance::stop(update_frame_count)?,
            _ => unreachable!(),
        }
        return Ok(());
    }

    if dispatch_plugin_action(name, payload.clone(), window, cx)? {
        return Ok(());
    }
    if dispatch_metadata_action(name, payload.clone(), window, cx)? {
        return Ok(());
    }

    // Resolve via the gpui action registry. Action names are namespaced
    // (e.g. `player_ui::PlayPause`); we accept either the full name or the
    // bare name and try to disambiguate by suffix match against registered
    // names. Bare names are convenient for scripts but ambiguous in theory —
    // we fail loudly when more than one action matches.
    let resolved = resolve_action_name(name, cx)?;
    let action = cx
        .build_action(&resolved, payload)
        .map_err(|e| anyhow!("build_action({resolved}) failed: {e}"))?;
    window
        .update(cx, |_view, window, cx| {
            window.dispatch_action(action, cx);
        })
        .map_err(|e| anyhow!("window.update failed: {e:#}"))?;
    Ok(())
}

fn dispatch_plugin_action(
    name: &str,
    payload: Option<Value>,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<bool> {
    use sotf_audio_player::controllers::plugin::dev_api::actions::plugin_action;
    let handled = matches!(
        name,
        "PluginAdd"
            | "PluginClear"
            | "PluginRemove"
            | "PluginToggle"
            | "PluginMoveUp"
            | "PluginMoveDown"
            | "PluginSetParam"
            | "PluginSetParamString"
            | "PluginChainSave"
            | "PluginChainLoad"
    );
    if !handled {
        return Ok(false);
    }
    with_app_state(window, cx, |state| {
        plugin_action(&mut state.app.plugin_state.graph, name, payload)
    })?;
    Ok(true)
}

fn dispatch_metadata_action(
    name: &str,
    payload: Option<Value>,
    window: AnyWindowHandle,
    cx: &mut App,
) -> Result<bool> {
    match name {
        "PluginSelectFirstUser" => {
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, cx| -> Result<()> {
                    let graph = &state.app.plugin_state.graph;
                    let plugin_idx = (0..graph.len())
                        .find(|&idx| {
                            graph
                                .get_plugin(idx)
                                .is_some_and(|plugin| !plugin.permanent)
                        })
                        .ok_or_else(|| anyhow!("no user plugin available for visual selection"))?;
                    state.app.plugin_state.selected_plugin_index = plugin_idx;
                    state.app.plugin_state.editing_plugin_index = Some(plugin_idx);
                    state.app.plugin_state.plugin_param_selection = 0;
                    cx.notify();
                    Ok(())
                })?;
                Ok(())
            })?;
            Ok(true)
        }
        "PluginSetInputChannels" => {
            let channels = payload_u32(payload.as_ref(), "channels", 2).clamp(1, 16) as usize;
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, cx| {
                    state.app.plugin_state.graph.adapt_matrix_to_input(channels);
                    cx.notify();
                });
                Ok(())
            })?;
            Ok(true)
        }
        "PlaybackSetDurationFixture" => {
            let duration_secs = payload
                .as_ref()
                .and_then(|value| value.get("duration_secs"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                .max(0.0);
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, _cx| {
                    state.app.playback.qa_duration_fixture = Some(duration_secs);
                });
                cx.notify();
                Ok(())
            })?;
            Ok(true)
        }
        "PlaybackResetQueueCursor" => {
            let requested_track = payload_u32(payload.as_ref(), "track_index", 0) as usize;
            let activate = payload
                .as_ref()
                .and_then(|value| value.get("activate"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let seek_loaded_player = payload
                .as_ref()
                .and_then(|value| value.get("seek_loaded_player"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            with_app_state(window, cx, |state| {
                let had_active_queue = state.app.playback.current_queue_index.is_some();
                let item = state
                    .app
                    .queue_state
                    .get_mut(0)
                    .ok_or_else(|| anyhow!("queue is empty"))?;
                if item.album.tracks.is_empty() {
                    return Err(anyhow!("first queue item has no tracks"));
                }
                item.current_track_index = requested_track.min(item.album.tracks.len() - 1);
                state.app.playback.current_queue_index = activate.then_some(0);
                state.app.playback.position_secs = 0.0;
                if seek_loaded_player && (activate || had_active_queue) {
                    state
                        .player
                        .seek(0.0)
                        .context("resetting loaded player cursor")?;
                }
                Ok(())
            })?;
            Ok(true)
        }
        "PlaylistReloadFixture" => {
            with_app_state(window, cx, |state| {
                let app = &mut state.app;
                let db = app
                    .library_state
                    .library
                    .get_database()
                    .ok_or_else(|| anyhow!("library database unavailable"))?;
                let mut controller = sotf_audio_player::PlaylistController::new();
                controller
                    .load_playlists(db)
                    .map_err(|error| anyhow!(error))?;
                if !controller.playlists().is_empty() {
                    controller
                        .open_playlist(db, 0)
                        .map_err(|error| anyhow!(error))?;
                }
                app.playlist.controller = controller;
                app.playlist.loaded = true;
                app.playlist.error = None;
                Ok(())
            })?;
            Ok(true)
        }
        "MetersSetFixture" => {
            let active = payload
                .as_ref()
                .and_then(|value| value.get("active"))
                .and_then(Value::as_bool)
                .unwrap_or(true);
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, cx| {
                    if active {
                        let mut fixture = sotf_audio_player::LoudnessData::new(6);
                        fixture.measurement_valid = true;
                        fixture.momentary_valid = true;
                        fixture.shortterm_valid = true;
                        fixture.integrated_valid = true;
                        fixture.sample_peak_valid = true;
                        fixture.true_peak_valid = true;
                        fixture.momentary_lufs = -18.4;
                        fixture.shortterm_lufs = -19.1;
                        fixture.integrated_lufs = -20.0;
                        fixture.peak = 0.82;
                        fixture.update_peaks(&[0.82, 0.64, 0.48, 0.31, 0.56, 0.42]);
                        fixture.update_true_peaks(&[-1.2, -2.4, -4.0, -6.8, -3.1, -4.8]);
                        let fixture = std::sync::Arc::new(fixture);
                        state.app.playback.qa_loudness_fixture = Some(fixture.clone());
                        state.app.playback.input_loudness_info = Some(fixture.clone());
                        state.app.playback.loudness_info = Some(fixture);
                    } else {
                        state.app.playback.qa_loudness_fixture = None;
                        state.app.playback.input_loudness_info = None;
                        state.app.playback.loudness_info = None;
                        state.app.level_meters.peak_hold.fill(0.0);
                    }
                    state.app.level_meters.last_channel_count = 0;
                    state.app.update_level_meter_groups();
                    state.app.update_level_meter_peak_hold();
                    state.app.ui_state.current_screen = Screen::Queue;
                    state.app.ui_state.input_mode = InputMode::Normal;
                    cx.notify();
                });
                Ok(())
            })?;
            Ok(true)
        }
        "SettingsSetTab" => {
            let tab = parse_settings_tab(payload_str(&payload, "tab")?)?;
            with_app_state(window, cx, |state| {
                state.app.ui_state.current_screen = Screen::Settings;
                state.app.ui_state.active_settings_tab = tab;
                state.app.ui_state.input_mode = InputMode::Normal;
                Ok(())
            })?;
            Ok(true)
        }
        "SettingsSetLibraryPickerFixture" => {
            let result = payload_str(&payload, "result")?;
            let qa_dir = std::env::var_os("SOTF_QA_DIR")
                .map(PathBuf::from)
                .ok_or_else(|| anyhow!("SOTF_QA_DIR is unavailable"))?;
            let seeded_library = qa_dir
                .parent()
                .ok_or_else(|| anyhow!("QA directory has no scenario parent"))?
                .join("library");
            let picker_result = match result {
                "selected" => {
                    if !seeded_library.is_dir() {
                        return Err(anyhow!(
                            "seeded QA library is unavailable: {}",
                            seeded_library.display()
                        ));
                    }
                    crate::app::state::QaLibraryPickerResult::Selected(seeded_library)
                }
                "permission_denied" => {
                    crate::app::state::QaLibraryPickerResult::PermissionDenied(seeded_library)
                }
                _ => {
                    return Err(anyhow!(
                        "library picker fixture result must be `selected` or `permission_denied`"
                    ));
                }
            };
            with_app_state(window, cx, |state| {
                state.app.settings.library.qa_picker_result = Some(picker_result);
                Ok(())
            })?;
            Ok(true)
        }
        #[cfg(feature = "tidal")]
        "SettingsSetTidalLoginFixture" => {
            let result = payload_str(&payload, "result")?;
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, _cx| {
                    state
                        .app
                        .qa_set_tidal_login_fixture(result)
                        .map_err(anyhow::Error::msg)
                })?;
                cx.notify();
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataSeedAlbum" => {
            with_app_state(window, cx, |state| {
                state.app.library_state.library.albums =
                    vec![sotf_audio_player::dev_api_fixtures::metadata_fixture_album()];
                state.app.library_state.selected_index = 0;
                state.app.library_state.invalidate_cache();
                state.app.ui_state.current_screen = Screen::Library;
                state.app.ui_state.input_mode = InputMode::Normal;
                state.app.modal.metadata_editor = None;
                Ok(())
            })?;
            Ok(true)
        }
        "HomeSeedShelves" => {
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, _cx| {
                    state.app.library_state.library.albums =
                        sotf_audio_player::dev_api_fixtures::home_fixture_albums();
                    state.app.library_state.selected_index = 0;
                    state.app.library_state.invalidate_cache();
                    state.app.library_view.loading_initial_data = false;
                    state.app.ui_state.expanded_home_sections.clear();
                    state.app.ui_state.current_screen = Screen::Home;
                    state.app.ui_state.input_mode = InputMode::Normal;
                });
                cx.notify();
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataOpenAlbumEditor" => {
            with_app_state(window, cx, |state| {
                let album = state
                    .app
                    .library_state
                    .selected_album()
                    .cloned()
                    .or_else(|| state.app.library_state.library.albums.first().cloned())
                    .ok_or_else(|| anyhow!("no album available for metadata editor"))?;
                state.app.modal.metadata_editor = Some(
                    MetadataEditorState::for_album(&album)
                        .map_err(|err| anyhow!("metadata editor unavailable: {err}"))?,
                );
                state.app.ui_state.input_mode = InputMode::MetadataEditor;
                Ok(())
            })?;
            Ok(true)
        }
        "ListeningResetGuide" => {
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, _| {
                    state.app.ui_state.current_screen = Screen::ListeningTest;
                    state.app.ui_state.input_mode = InputMode::Normal;
                    state.app.tutorial.listening_guide_open = true;
                    state.app.tutorial.listening_break_prompt_open = false;
                    state.app.tutorial.listening_break_dismissed_at = 0;
                    state
                        .app
                        .plugin_state
                        .listening_test_state
                        .eq_progress
                        .how_to_listen_completed = false;
                });
                cx.notify();
                Ok(())
            })?;
            Ok(true)
        }
        "ListeningClearSession" => {
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, _| {
                    let _ = state.app.plugin_state.leave_ab_test_runtime();
                    let listening = &mut state.app.plugin_state.listening_test_state;
                    listening.ab_test = Default::default();
                    listening.path_a = None;
                    listening.path_b = None;
                    listening.path_a_canvas = None;
                    listening.path_b_canvas = None;
                    listening.status.clear();
                    listening.surface =
                        crate::app::state::plugin::EarTrainingSurface::BlindComparison;
                    state.app.ui_state.current_screen = Screen::ListeningTest;
                    state.app.ui_state.input_mode = InputMode::Normal;
                    state.app.tutorial.listening_break_prompt_open = false;
                });
                cx.notify();
                Ok(())
            })?;
            Ok(true)
        }
        "ListeningSetBreakInterval" => {
            let trials = payload_u32(payload.as_ref(), "trials", 5).clamp(1, 100) as usize;
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, _| {
                    state.app.tutorial.listening_break_interval = trials;
                    state.app.tutorial.listening_break_prompt_open = false;
                    state.app.tutorial.listening_break_dismissed_at = 0;
                });
                cx.notify();
                Ok(())
            })?;
            Ok(true)
        }
        "ListeningCorruptMediaIdentity" => {
            with_player_view(window, cx, |view, cx| {
                view.state.update(cx, |state, _| {
                    let listening = &mut state.app.plugin_state.listening_test_state;
                    let session = listening
                        .ab_test
                        .session_mut()
                        .ok_or_else(|| anyhow!("no listening session to corrupt"))?;
                    session.setup.media.media_id = format!("sha256:{}", "0".repeat(64));
                    listening.status.clear();
                    Ok::<(), anyhow::Error>(())
                })?;
                cx.notify();
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataSetField" => {
            with_app_state(window, cx, |state| {
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                let field = payload_str(&payload, "field")?;
                let value = payload_str(&payload, "value")?;
                set_metadata_field(editor, field, value.to_string())?;
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataPreview" => {
            with_app_state(window, cx, |state| {
                let (target, patch) = {
                    let editor = state
                        .app
                        .modal
                        .metadata_editor
                        .as_ref()
                        .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                    (
                        editor.target.clone(),
                        editor
                            .patch()
                            .map_err(|err| anyhow!("invalid metadata patch: {err}"))?,
                    )
                };
                let result = state.app.library_state.preview_metadata_edit(target, patch);
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                match result {
                    Ok(preview) => {
                        editor.preview = Some(preview);
                        editor.error = None;
                    }
                    Err(err) => {
                        editor.error = Some(err.to_string());
                        return Err(anyhow!(err.to_string()));
                    }
                }
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataInjectCandidate" => {
            with_app_state(window, cx, |state| {
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                editor
                    .search_results
                    .push(metadata_candidate_from_payload(payload.as_ref()));
                editor.selected_result = editor.search_results.len().saturating_sub(1);
                editor.search_error = None;
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataImportCandidate" => {
            with_app_state(window, cx, |state| {
                let editor = state
                    .app
                    .modal
                    .metadata_editor
                    .as_mut()
                    .ok_or_else(|| anyhow!("metadata editor is not open"))?;
                let candidate = editor
                    .search_results
                    .get(editor.selected_result)
                    .cloned()
                    .ok_or_else(|| anyhow!("no metadata candidate selected"))?;
                editor.apply_candidate(candidate);
                Ok(())
            })?;
            Ok(true)
        }
        "MetadataClose" => {
            with_app_state(window, cx, |state| {
                state.app.modal.metadata_editor = None;
                state.app.ui_state.input_mode = InputMode::Normal;
                Ok(())
            })?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_settings_tab(value: &str) -> Result<SettingsTab> {
    match value {
        "Library" => Ok(SettingsTab::Library),
        "Theme" => Ok(SettingsTab::Theme),
        "Language" => Ok(SettingsTab::Language),
        "Keybindings" => Ok(SettingsTab::Keybindings),
        "AudioDevice" => Ok(SettingsTab::AudioDevice),
        "Misc" => Ok(SettingsTab::Misc),
        "Federation" => Ok(SettingsTab::Federation),
        "Servers" => Ok(SettingsTab::Servers),
        "Metadata" => Ok(SettingsTab::Metadata),
        "ReleaseChannel" => Ok(SettingsTab::ReleaseChannel),
        other => Err(anyhow!("unknown settings tab `{other}`")),
    }
}

fn payload_str<'a>(payload: &'a Option<Value>, key: &str) -> Result<&'a str> {
    payload
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("metadata action payload needs string `{key}`"))
}

fn payload_u32(payload: Option<&Value>, key: &str, default: u32) -> u32 {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(default)
}

fn payload_u8(payload: Option<&Value>, key: &str, default: u8) -> u8 {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_u64)
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(default)
}

fn set_metadata_field(editor: &mut MetadataEditorState, field: &str, value: String) -> Result<()> {
    match field {
        "title" => editor.fields.title = value,
        "artist" => editor.fields.artist = value,
        "album_artist" => editor.fields.album_artist = value,
        "year" => editor.fields.year = value,
        "genre" => editor.fields.genre = value,
        "composer" => editor.fields.composer = value,
        "disc" | "disc_number" => editor.fields.disc_number = value,
        "track" | "track_number" => editor.fields.track_number = value,
        "conductor" => editor.fields.conductor = value,
        "performer" => editor.fields.performer = value,
        "isrc" => editor.fields.isrc = value,
        "ensemble" => editor.fields.ensemble = value,
        "edition" => editor.fields.edition = value,
        other => return Err(anyhow!("unknown metadata field `{other}`")),
    }
    editor.preview = None;
    editor.error = None;
    Ok(())
}

fn payload_string(payload: Option<&Value>, key: &str, default: &str) -> Option<String> {
    payload
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| Some(default.to_string()))
}

fn metadata_candidate_from_payload(
    payload: Option<&Value>,
) -> sotf_audio_player::MetadataImportCandidate {
    sotf_audio_player::MetadataImportCandidate {
        provider_id: "musicbrainz".to_string(),
        provider_entity_id: payload
            .and_then(|value| value.get("provider_entity_id"))
            .and_then(Value::as_str)
            .unwrap_or("scenario-release")
            .to_string(),
        title: payload_string(payload, "title", "Imported Track"),
        artist: payload_string(payload, "artist", "Imported Artist"),
        album_artist: payload_string(payload, "album_artist", "Imported Artist"),
        album_title: payload_string(payload, "album_title", "Imported Album"),
        year: Some(payload_u32(payload, "year", 2024)),
        track_number: Some(payload_u32(payload, "track_number", 1)),
        disc_number: Some(payload_u32(payload, "disc_number", 1)),
        isrc: payload
            .and_then(|value| value.get("isrc"))
            .and_then(Value::as_str)
            .map(str::to_string),
        score: payload_u8(payload, "score", 96),
    }
}

pub(super) fn dispatch_request(req: &HttpRequest, tx: &mpsc::SyncSender<DevCommand>) -> String {
    let (path, query) = split_path_query(&req.path);
    let result: Result<(u16, String)> = match (req.method.as_str(), path) {
        ("POST", "/action") => post_action(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/query") => get_query(query, tx).map(|r| {
            let status = if r.value.is_ok() { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/key") => post_key(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/text") => post_text(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/input") => post_input(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/click") => post_click(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/hover") => post_hover(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/drag") => post_drag(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/scroll") => post_scroll(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/resize") => post_resize(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/screenshot") => post_screenshot(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/health") => get_health(tx).map(|r| {
            let status = if r.value.is_ok() { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/accessibility") => get_accessibility(tx).map(|r| {
            let status = if r.value.is_ok() { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("GET", "/snapshot") => get_snapshot(tx).map(|r| {
            let status = if r.value.is_ok() { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/quit") => post_quit(tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/ui-environment") => post_qa_ui_environment(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/seed") => post_qa_seed(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/recording/fake-capture") => post_qa_recording_fake_capture(&req.body, tx)
            .map(|r| {
                let status = if r.ok { 200 } else { 500 };
                (status, r.to_json())
            }),
        ("POST", "/qa/headphone/discovery-fixture") => {
            post_qa_headphone_discovery_fixture(&req.body, tx).map(|r| {
                let status = if r.ok { 200 } else { 500 };
                (status, r.to_json())
            })
        }
        ("POST", "/qa/spinorama/discovery-fixture") => {
            post_qa_spinorama_discovery_fixture(&req.body, tx).map(|r| {
                let status = if r.ok { 200 } else { 500 };
                (status, r.to_json())
            })
        }
        ("POST", "/qa/room-eq") => post_qa_room_eq(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/room-eq/ui-fixture") => post_qa_room_eq_ui_fixture(&req.body, tx).map(|r| {
            let status = if r.ok { 200 } else { 500 };
            (status, r.to_json())
        }),
        ("POST", "/qa/room-eq/export-json") => {
            post_qa_room_eq_export_json(&req.body, tx).map(|r| {
                let status = if r.value.is_ok() { 200 } else { 500 };
                (status, r.to_json())
            })
        }
        ("GET", "/elements") => Ok((200, list_elements_json())),
        _ => Err(anyhow!("unknown route: {} {}", req.method, req.path)),
    };
    match result {
        Ok((status, body)) => http_response(status, &body),
        Err(e) => http_response(400, &DevReply::err(format!("{e:#}")).to_json()),
    }
}
