use super::super::SpatialSpiderSnapshot;
#[cfg(feature = "gpu-3d")]
use d3rs::gpu3d::Lines3DState;
use gpui::*;
use sotf_plugins::speaker_config::{
    SpeakerConfig, get_speaker_config, get_speaker_config_by_channels,
};
#[cfg(feature = "gpu-3d")]
use std::cell::RefCell;
#[cfg(feature = "gpu-3d")]
use std::rc::Rc;

pub(super) fn with_alpha(c: gpui::Rgba, alpha: f32) -> gpui::Rgba {
    gpui::Rgba { a: alpha, ..c }
}

pub(super) fn blend(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    Rgba {
        r: a.r * (1.0 - t) + b.r * t,
        g: a.g * (1.0 - t) + b.g * t,
        b: a.b * (1.0 - t) + b.b * t,
        a: a.a * (1.0 - t) + b.a * t,
    }
}

#[cfg(any(feature = "gpu-3d", test))]
pub(super) fn translucent(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
}

/// Resolve the speaker layout to render against. Explicit id wins, else
/// derive from the loudness data's channel count, else `None`.
pub fn resolve_speaker_config(
    snapshot: &SpatialSpiderSnapshot,
    speaker_config_id: Option<&str>,
) -> Option<&'static SpeakerConfig> {
    speaker_config_id.and_then(get_speaker_config).or_else(|| {
        snapshot
            .loudness
            .as_ref()
            .and_then(|li| get_speaker_config_by_channels(li.true_peaks_dbtp.len()))
    })
}

/// Attach left-drag → rotate, middle-drag → pan, scroll → zoom handlers to
/// an interactive (id'd) div, mutating the supplied `Lines3DState` so the
/// next paint picks up the new camera.
#[cfg(feature = "gpu-3d")]
pub(super) fn attach_orbit_handlers(
    container: Stateful<Div>,
    state: Rc<RefCell<Lines3DState>>,
) -> Stateful<Div> {
    let s_down_l = state.clone();
    let s_down_m = state.clone();
    let s_move = state.clone();
    let s_up_l = state.clone();
    let s_up_m = state.clone();
    let s_scroll = state;

    container
        .on_mouse_down(MouseButton::Left, move |event, _window, _cx| {
            let mut st = s_down_l.borrow_mut();
            st.dragging = true;
            st.last_mouse = Some(event.position);
        })
        .on_mouse_down(MouseButton::Middle, move |event, _window, _cx| {
            let mut st = s_down_m.borrow_mut();
            st.panning = true;
            st.last_mouse = Some(event.position);
        })
        .on_mouse_move(move |event, _window, _cx| {
            let mut st = s_move.borrow_mut();
            let Some(last) = st.last_mouse else { return };
            let dx = f32::from(event.position.x - last.x);
            let dy = f32::from(event.position.y - last.y);
            if st.dragging {
                st.controls.rotate(dx, dy);
                st.update_camera();
            } else if st.panning {
                let camera = st.camera.clone();
                st.controls.pan(dx, dy, &camera);
                st.update_camera();
            }
            if st.dragging || st.panning {
                st.last_mouse = Some(event.position);
            }
        })
        .on_mouse_up(MouseButton::Left, move |_event, _window, _cx| {
            let mut st = s_up_l.borrow_mut();
            st.dragging = false;
            if !st.panning {
                st.last_mouse = None;
            }
        })
        .on_mouse_up(MouseButton::Middle, move |_event, _window, _cx| {
            let mut st = s_up_m.borrow_mut();
            st.panning = false;
            if !st.dragging {
                st.last_mouse = None;
            }
        })
        .on_scroll_wheel(move |event, _window, _cx| {
            let mut st = s_scroll.borrow_mut();
            // GPUI ScrollWheelEvent.delta is a ScrollDelta enum with Pixels
            // or Lines variants; both expose a vertical magnitude via `y`.
            // We normalise to a small unitless step suitable for OrbitControls.
            let step = match event.delta {
                ScrollDelta::Pixels(p) => f32::from(p.y) / 100.0,
                ScrollDelta::Lines(l) => l.y * 0.5,
            };
            st.controls.zoom(step);
            st.update_camera();
        })
}
