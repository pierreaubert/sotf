use d3rs::gpu3d::{
    Colormap as Surface3DColormap, Surface3DConfig, Surface3DElement, SurfaceData as Surface3DData,
    SurfacePlotType,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_ui_kit::Slider;

use super::SpinoramaApp;
use crate::types::Colormap;

impl SpinoramaApp {
    /// Render 3D Sphere plot - SPL of a single frequency mapped to color
    pub fn render_sphere_plot(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(ref contour_data) = self.contour_data else {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_base()
                    .text_color(rgb(0x666666))
                    .child("No contour data available for this speaker."),
            );
        };

        // 1. Data Preparation
        let freq_values = contour_data.freq.clone();
        let angle_values = contour_data.angles.clone();
        let spl_values = contour_data.spl.clone();
        let freq_count = contour_data.freq_count;
        let angle_count = contour_data.angle_count;

        // Ensure frequency index is valid
        if self.sphere_freq_idx >= freq_count {
            self.sphere_freq_idx = freq_count.saturating_sub(1);
        }
        let current_freq = freq_values[self.sphere_freq_idx];

        // 2. Generate Synthetic Data for "Beach Ball" effect
        // We want the sphere to show the SPL at `current_freq` for each Azimuth angle.
        // We replicate the SPL slice for every Elevation step so colors form vertical stripes.

        // Generate high resolution elevation grid (-90 to 90 degrees) for smooth sphere
        let min_elev = -90.0f64;
        let max_elev = 90.0f64;
        let step_elev = 5.0f64; // 5 degree steps = 37 points (sufficient for smooth look)
        let steps = ((max_elev - min_elev) / step_elev).round() as usize + 1;
        let synthetic_elevation: Vec<f64> = (0..steps)
            .map(|i| min_elev + (i as f64 * step_elev))
            .collect();
        let elev_count = synthetic_elevation.len();

        let mut z_values = Vec::with_capacity(angle_count);
        for i in 0..angle_count {
            // Get SPL at the current frequency for this angle `i`
            let data_idx = i * freq_count + self.sphere_freq_idx;
            let spl = if data_idx < spl_values.len() {
                spl_values[data_idx]
            } else {
                0.0
            };

            // Replicate this SPL value for all synthetic elevations
            let spl_column = vec![spl; elev_count];
            z_values.push(spl_column);
        }

        // 3. Configure Surface Data
        let surface_data =
            Surface3DData::from_grid(synthetic_elevation.clone(), angle_values.clone(), z_values)
                .with_log_x(false) // Linear Elevation
                .with_x_label("Elevation (deg)")
                .with_x_range(-90.0, 90.0)
                .with_x_ticks(vec![-90.0, -45.0, 0.0, 45.0, 90.0])
                .with_y_label("Azimuth (deg)")
                .with_y_range(-180.0, 180.0)
                .with_y_ticks(vec![-180.0, -120.0, -60.0, 0.0, 60.0, 120.0, 180.0])
                .with_z_label("SPL (dB)")
                .with_z_range(-40.0, 10.0)
                .with_z_ticks(vec![-40.0, -30.0, -20.0, -10.0, 0.0, 10.0]);

        // Map colormap
        let colormap = match self.contour_colormap {
            Colormap::Viridis => Surface3DColormap::Viridis,
            Colormap::Plasma => Surface3DColormap::Plasma,
            Colormap::Magma => Surface3DColormap::Inferno,
            Colormap::Inferno => Surface3DColormap::Inferno,
            Colormap::Heat => Surface3DColormap::Inferno,
            Colormap::Coolwarm => Surface3DColormap::CoolWarm,
        };

        let config = Surface3DConfig::new()
            .colormap(colormap)
            .wireframe(self.surface_wireframe)
            .background_color(1.0, 1.0, 1.0)
            .opacity(self.surface_opacity)
            .isolines(false)
            .plot_type(SurfacePlotType::Spherical)
            .camera_position(
                3.5,
                self.surface_rotation_azimuth,
                self.surface_rotation_elevation,
            );

        let surface_element =
            Surface3DElement::new(surface_data, config).with_state(self.surface_state.clone());

        // Colormap UI
        let colormaps = [
            (Colormap::Viridis, "Viridis"),
            (Colormap::Plasma, "Plasma"),
            (Colormap::Magma, "Magma"),
            (Colormap::Inferno, "Inferno"),
            (Colormap::Heat, "Heat"),
            (Colormap::Coolwarm, "Coolwarm"),
        ];

        let colormap_selector = div()
            .flex()
            .flex_row()
            .gap_2()
            .children(colormaps.iter().map(|&(cm, label)| {
                div()
                    .id(ElementId::Name(format!("cmap-{}", label).into()))
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .when(self.contour_colormap == cm, |el| {
                        el.bg(rgb(0x3b82f6)).text_color(rgb(0xffffff))
                    })
                    .when(self.contour_colormap != cm, |el| {
                        el.bg(rgb(0xe5e7eb)).text_color(rgb(0x666666))
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.contour_colormap = cm;
                        cx.notify();
                    }))
            }));

        let wireframe_toggle = div()
            .id("surface-wireframe-toggle")
            .px_3()
            .py_1()
            .rounded(px(4.0))
            .cursor_pointer()
            .when(self.surface_wireframe, |el| {
                el.bg(rgb(0x3b82f6)).text_color(rgb(0xffffff))
            })
            .when(!self.surface_wireframe, |el| {
                el.bg(rgb(0xe5e7eb)).text_color(rgb(0x666666))
            })
            .child("Wireframe")
            .on_click(cx.listener(|this, _, _window, cx| {
                this.surface_wireframe = !this.surface_wireframe;
                cx.notify();
            }));

        // Opacity slider using gpui-ui-kit Slider
        let entity = cx.entity().clone();
        let opacity_slider = Slider::new("opacity-slider")
            .value(self.surface_opacity * 100.0)
            .min(0.0)
            .max(100.0)
            .step(5.0)
            .width(120.0)
            .label("Opacity")
            .show_value(true)
            .on_change(move |value, _window, cx| {
                entity.update(cx, |this, cx| {
                    this.surface_opacity = value / 100.0;
                    cx.notify();
                });
            });

        // Frequency Overlay
        let freq_display = div()
            .absolute()
            .top(px(20.0))
            .right(px(20.0))
            .bg(gpui::hsla(0.0, 0.0, 0.0, 0.7))
            .text_color(rgb(0xffffff))
            .px_4()
            .py_2()
            .rounded_md()
            .text_lg()
            .font_weight(FontWeight::BOLD)
            .child(format!("Freq: {:.0} Hz", current_freq));

        // Interactive View
        let surface_view = div()
            .id("sphere-3d-view")
            .w(px(800.0))
            .h(px(800.0))
            .bg(rgb(0x1a1a1a))
            .relative() // For absolute positioning
            .child(surface_element)
            .child(freq_display)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    let mut state = this.surface_state.borrow_mut();
                    state.dragging = true;
                    state.last_mouse = Some(event.position);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    let mut state = this.surface_state.borrow_mut();
                    state.dragging = false;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                let mut state = this.surface_state.borrow_mut();
                if let Some(last) = state.last_mouse {
                    let delta_x: f32 = event.position.x.into();
                    let delta_y: f32 = event.position.y.into();
                    let last_x: f32 = last.x.into();
                    let last_y: f32 = last.y.into();
                    let dx = delta_x - last_x;
                    let dy = delta_y - last_y;

                    if state.dragging {
                        state.controls.rotate(dx, dy);
                        state.update_camera();
                        cx.notify();
                    }
                }
                if state.dragging {
                    state.last_mouse = Some(event.position);
                }
            }))
            .on_scroll_wheel(cx.listener(move |this, event: &ScrollWheelEvent, _, cx| {
                let delta_y = match event.delta {
                    ScrollDelta::Lines(lines) => lines.y,
                    ScrollDelta::Pixels(pixels) => {
                        let py: f32 = pixels.y.into();
                        // Simple threshold to avoid jitter
                        if py.abs() > 0.0 {
                            py.signum()
                        } else {
                            0.0
                        }
                    }
                };

                if delta_y != 0.0 {
                    let max_idx = freq_count.saturating_sub(1);
                    // Scroll Up (Positive) usually means "move down the page" or "decrease".
                    // But for values, Frame Up often means Increment.
                    // Let's standard: Scroll Up (on mouse) -> Zoom In / Increase Value.
                    // Mac 'Natural Scrolling': Swipe Up -> Content moves down -> Delta is Positive?
                    // Let's assume Delta > 0 is "Up/Increase".

                    if delta_y > 0.0 {
                        if this.sphere_freq_idx < max_idx {
                            this.sphere_freq_idx += 1;
                        }
                    } else {
                        if this.sphere_freq_idx > 0 {
                            this.sphere_freq_idx -= 1;
                        }
                    }
                    cx.notify();
                }
            }));

        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .text_2xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0x333333))
                    .child(format!(
                        "Sphere Plot - {}",
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_4()
                    .items_center()
                    .child(div().text_sm().text_color(rgb(0x666666)).child("Colormap:"))
                    .child(colormap_selector)
                    .child(wireframe_toggle)
                    .child(opacity_slider),
            )
            .child(div().flex().justify_center().child(surface_view))
    }
}
