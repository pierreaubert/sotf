use crate::{ShowcaseApp, GeoProjectionType};
use d3rs::geo::{
    projection::{Orthographic, Mercator, Equirectangular, Stereographic, ConicEqualArea},
    GeoPath,
};
use gpui::prelude::*;
use gpui::*;
use super::super::world_data::get_world_data;

pub fn render(app: &ShowcaseApp, cx: &mut Context<ShowcaseApp>) -> Div {
    let width = 600.0f32;
    let height = 400.0f32;

    // Use app state for rotation
    let rotation = [app.geo_rotation_lon, app.geo_rotation_lat];
    let use_large_data = app.use_large_data;
    let current_projection = app.geo_projection_type;

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_4()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::BOLD)
                .mb_4()
                .child("Versor Dragging"),
        )
        // Projection Selector
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .mb_4()
                .children(GeoProjectionType::all().into_iter().map(|proj_type| {
                    let is_selected = proj_type == current_projection;
                    let bg = if is_selected { rgb(0x007acc) } else { rgb(0xe8e8e8) };
                    let text_color = if is_selected { rgb(0xffffff) } else { rgb(0x333333) };

                    div()
                        .id(ElementId::Name(format!("proj-{}", proj_type.label()).into()))
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .bg(bg)
                        .hover(|s| s.bg(if is_selected { rgb(0x007acc) } else { rgb(0xd0d0d0) }))
                        .text_color(text_color)
                        .text_xs()
                        .child(proj_type.label())
                        .on_click(cx.listener(move |this, _, _window, _cx| {
                            this.geo_projection_type = proj_type;
                        }))
                }))
        )
        .child(
            div()
                .w(px(width))
                .h(px(height))
                .bg(rgb(0xf0f0f0))
                .relative()
                // Mouse event listeners for dragging
                .on_mouse_down(MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, _, _| {
                    this.is_dragging = true;
                    this.last_mouse_pos = Some(event.position);
                }))
                .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, _| {
                    this.is_dragging = false;
                    this.last_mouse_pos = None;
                }))
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, _| {
                    if this.is_dragging {
                        if let Some(last_pos) = this.last_mouse_pos {
                            // Convert Pixels to f32 using Into
                            let delta_x: f32 = (event.position.x - last_pos.x).into();
                            let delta_y: f32 = (event.position.y - last_pos.y).into();

                            // Simple rotation update (sensitivity could be tuned)
                            this.geo_rotation_lon += delta_x as f64 * 0.5;
                            this.geo_rotation_lat -= delta_y as f64 * 0.5;

                            // Clamp latitude to avoid flipping issue in some projections
                            this.geo_rotation_lat = this.geo_rotation_lat.max(-90.0).min(90.0);

                            this.last_mouse_pos = Some(event.position);
                        }
                    }
                }))
                .child(
                    canvas(
                        move |bounds, _, _| bounds,
                        move |bounds, _, window, _| {
                            // Macro to avoid code duplication and impl Trait issues
                            macro_rules! draw_geo {
                                ($projection:expr) => {
                                    {
                                        let geometry = get_world_data(use_large_data);
                                        let path = GeoPath::new($projection.clone());
                                        let d = path.render(&geometry);
                                        if let Some(p) = super::path_utils::parse_svg_path(&d, bounds) {
                                            window.paint_path(p, rgb(0x228822));
                                        }
                                    }
                                }
                            }

                            // Draw Sphere background only for Orthographic/Stereo/Globe-like
                            if matches!(current_projection, GeoProjectionType::Orthographic | GeoProjectionType::Stereographic) {
                                let center = bounds.origin + point(px(width/2.0), px(height/2.0));
                                let radius = px(180.0); // Approx
                                let sphere_bounds = Bounds {
                                    origin: center - point(radius, radius),
                                    size: size(radius * 2.0, radius * 2.0),
                                };
                                window.paint_quad(PaintQuad {
                                    bounds: sphere_bounds,
                                    corner_radii: Corners::all(radius),
                                    background: rgb(0xe0e0ff).into(),
                                    border_widths: Edges::all(px(1.0)),
                                    border_color: rgb(0x000000).into(),
                                    border_style: BorderStyle::default(),
                                });
                            }

                            // Instantiate and draw projection
                            match current_projection {
                                GeoProjectionType::Mercator => {
                                    let p = Mercator::new()
                                        .scale(height as f64 / 3.0)
                                        .translate(width as f64 / 2.0, height as f64 / 2.0 + 50.0)
                                        .rotate(rotation[0], 0.0, 0.0);
                                    draw_geo!(p);
                                },
                                GeoProjectionType::Orthographic => {
                                    let p = Orthographic::new()
                                        .scale(height as f64 / 2.1)
                                        .translate(width as f64 / 2.0, height as f64 / 2.0)
                                        .rotate(rotation[0], rotation[1], 0.0);
                                    draw_geo!(p);
                                },
                                GeoProjectionType::Equirectangular => {
                                     let p = Equirectangular::new()
                                        .scale(width as f64 / 360.0 * 0.9)
                                        .translate(width as f64 / 2.0, height as f64 / 2.0)
                                        .rotate(rotation[0], rotation[1], 0.0);
                                     draw_geo!(p);
                                },
                                GeoProjectionType::Stereographic => {
                                    let p = Stereographic::new()
                                        .scale(height as f64 / 2.5)
                                        .translate(width as f64 / 2.0, height as f64 / 2.0)
                                        .rotate(rotation[0], rotation[1], 0.0);
                                    draw_geo!(p);
                                },
                                GeoProjectionType::ConicEqualArea => {
                                    let p = ConicEqualArea::new()
                                        .scale(height as f64 / 5.0)
                                        .translate(width as f64 / 2.0, height as f64 / 2.0)
                                        .rotate(rotation[0], rotation[1], 0.0);
                                    draw_geo!(p);
                                },
                            };
                        }
                    )
                )
        )
}
