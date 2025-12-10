use d3rs::geo::{
    ConicEqualArea, Equirectangular, GeoPath, Graticule, Mercator, Orthographic, Projection,
    Stereographic, Rotation,
};
use gpui::*;

use super::ShowcaseApp;
use crate::GeoProjectionType;
use super::world_data::world_continents;

/// Famous cities with their coordinates
const CITIES: &[(&str, f64, f64)] = &[
    ("New York", -74.0, 40.7),
    ("London", -0.1, 51.5),
    ("Paris", 2.3, 48.9),
    ("Tokyo", 139.7, 35.7),
    ("Sydney", 151.2, -33.9),
    ("Rio de Janeiro", -43.2, -22.9),
    ("Cairo", 31.2, 30.0),
    ("Moscow", 37.6, 55.8),
    ("Mumbai", 72.9, 19.1),
    ("Beijing", 116.4, 39.9),
];

pub fn render(app: &ShowcaseApp, cx: &mut Context<ShowcaseApp>) -> Div {
    let current_projection = app.geo_projection_type;
    let rotation_lon = app.geo_rotation_lon;
    let rotation_lat = app.geo_rotation_lat;

    // Map dimensions
    let map_width = 800.0_f64;
    let map_height = 500.0_f64;
    let center_x = map_width / 2.0;
    let center_y = map_height / 2.0;

    div()
        .flex()
        .flex_col()
        .gap_6()
        .size_full()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Geographic Projections Demo"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0x666666))
                .max_w(px(700.0))
                .child("The d3-geo module provides geographic projections for mapping spherical coordinates (longitude, latitude) to planar coordinates (x, y). Select a projection below to see how it transforms the globe."),
        )
        // Projection selector
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Select Projection:"),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .children(GeoProjectionType::all().into_iter().map(|proj_type| {
                            let is_selected = proj_type == current_projection;
                            let bg = if is_selected {
                                rgb(0x007acc)
                            } else {
                                rgb(0xe8e8e8)
                            };
                            let text_color = if is_selected {
                                rgb(0xffffff)
                            } else {
                                rgb(0x333333)
                            };

                            div()
                                .id(ElementId::Name(proj_type.label().into()))
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .cursor_pointer()
                                .bg(bg)
                                .hover(|s| s.bg(if is_selected { rgb(0x007acc) } else { rgb(0xd0d0d0) }))
                                .text_color(text_color)
                                .text_sm()
                                .child(proj_type.label())
                                .on_click(cx.listener(move |this, _, _window, _cx| {
                                    this.geo_projection_type = proj_type;
                                }))
                        })),
                ),
        )
        // Map visualization
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{} Projection", current_projection.label())),
                )
                .child(
                    div()
                        .w(px(map_width as f32))
                        .h(px(map_height as f32))
                        .relative()
                        .bg(rgb(0xe8f4fc))
                        .border_1()
                        .border_color(rgb(0xcccccc))
                        .rounded_lg()
                        .overflow_hidden()
                        .child(
                            canvas(
                                move |bounds, _, _| bounds,
                                move |bounds, _, _, _| {
                                    let scale = match current_projection {
                                        GeoProjectionType::Mercator => map_height / 3.0,
                                        GeoProjectionType::Equirectangular => map_width / 360.0 * 0.9,
                                        GeoProjectionType::Orthographic => map_height / 2.5,
                                        GeoProjectionType::Stereographic => map_height / 4.0,
                                        GeoProjectionType::ConicEqualArea => map_height / 5.0,
                                    };

                                    let rotation = Rotation::new().angles(rotation_lon, rotation_lat, 0.0);

                                    // Render simplified world
                                    let continents = world_continents();
                                    
                                    // Helper to render path
                                    let render_path = |path_str: String, fill: Option<Rgba>, stroke: Option<Rgba>, width: f32| {
                                        // Decide whether to fill or stroke based on args (simplistic)
                                        let mut path_builder = if fill.is_some() {
                                            PathBuilder::fill()
                                        } else {
                                            PathBuilder::stroke(px(width))
                                        };
                                        
                                        // Simple SVG path parser
                                        let mut chars = path_str.chars().peekable();
                                        let read_coord = |chars: &mut std::iter::Peekable<std::str::Chars>| -> Option<f32> {
                                            // Skip separators
                                            while let Some(&c) = chars.peek() {
                                                if c == ',' || c == ' ' {
                                                    chars.next();
                                                } else {
                                                    break;
                                                }
                                            }
                                            
                                            let mut s = String::new();
                                            // Read sign
                                            if let Some(&c) = chars.peek() {
                                                if c == '-' {
                                                    s.push(chars.next().unwrap());
                                                }
                                            }
                                            
                                            // Read number
                                            let mut has_dot = false;
                                            while let Some(&c) = chars.peek() {
                                                if c.is_ascii_digit() {
                                                    s.push(chars.next().unwrap());
                                                } else if c == '.' && !has_dot {
                                                    has_dot = true;
                                                    s.push(chars.next().unwrap());
                                                } else {
                                                    break;
                                                }
                                            }
                                            
                                            if s.is_empty() { 
                                                None 
                                            } else { 
                                                s.parse::<f32>().ok().filter(|v| v.is_finite())
                                            }
                                        };
                                        
                                        while let Some(&cmd) = chars.peek() {
                                            if cmd.is_ascii_alphabetic() {
                                                chars.next();
                                                match cmd {
                                                    'M' => {
                                                        if let (Some(x), Some(y)) = (read_coord(&mut chars), read_coord(&mut chars)) {
                                                            path_builder.move_to(bounds.origin + point(px(x), px(y)));
                                                        }
                                                    }
                                                    'L' => {
                                                        if let (Some(x), Some(y)) = (read_coord(&mut chars), read_coord(&mut chars)) {
                                                            path_builder.line_to(bounds.origin + point(px(x), px(y)));
                                                        }
                                                    }
                                                    'm' => { // Relative move (treated as absolute for now heavily simplifed)
                                                         if let (Some(x), Some(y)) = (read_coord(&mut chars), read_coord(&mut chars)) {
                                                            path_builder.move_to(bounds.origin + point(px(x), px(y)));
                                                        }
                                                    }
                                                    'Z' | 'z' => {
                                                        path_builder.close();
                                                    }
                                                    _ => {} 
                                                }
                                            } else {
                                                // Implicit command (usually L after M)
                                                // For this simple demo, we assume implicit L
                                                 if let (Some(x), Some(y)) = (read_coord(&mut chars), read_coord(&mut chars)) {
                                                    path_builder.line_to(bounds.origin + point(px(x), px(y)));
                                                } else {
                                                    chars.next(); // Skip unknown
                                                }
                                            }
                                        }
                                        
                                        if let Ok(path) = path_builder.build() {
                                            // Fill
                                            // Scene::paint_path is not directly available, need window.paint_path or similar
                                            // Wait, canvas closure gives (bounds, state, window, cx) usually?
                                            // No, gpui::canvas signature: prepaint: (bounds, &mut Window, &mut App), paint: (bounds, state, &mut Window, &mut App)
                                        }
                                    };
                                }
                            )
                        )
                        // Canvas overlay for map
                        .child(
                            canvas(
                                move |bounds, _, _| bounds,
                                move |bounds, _, window, _| {
                                    let scale = match current_projection {
                                        GeoProjectionType::Mercator => map_height / 3.0,
                                        GeoProjectionType::Equirectangular => map_width / 360.0 * 0.9,
                                        GeoProjectionType::Orthographic => map_height / 2.5,
                                        GeoProjectionType::Stereographic => map_height / 4.0,
                                        GeoProjectionType::ConicEqualArea => map_height / 3.5,
                                    };

                                    let rotation = Rotation::new().angles(rotation_lon, rotation_lat, 0.0);

                                    // Helper for Rendering
                                    // Since we deal with Traits, we need to dispatch manually until d3rs supports dynamic dispatch better or we use an enum
                                    // We only use this block to generate string, so we can discard the projection after.
                                    // Wait, we generate a STRING here? No, this block is actually UNUSED in the final code I wrote previously?
                                    // Looking at the file content, lines 236-304 seem to be the `match` block for `let path_str = ...`.
                                    // But I commented out/replaced the usage of `path_str` with direct rendering below.
                                    // Ah, I see "Helper for Rendering" block around line 236.
                                    // If this block is unused or redundant, I should remove it.
                                    // The code I wrote replaces `path_str` generation with `continents_svg` and `grid_svg` generation blocks.
                                    // Let me double check if I left the old block in.
                                    // If `path_str` is unused, I should remove it to fix the error and cleanup.
                                    
                                    // Looking at lines 316+ in previous `replace_file_content`, I see `// 1. Draw Continents (Fill)`.
                                    // So the previous block `let path_str = match ...` is likely still there and causing the ownership error even if unused?
                                    // Let's remove the redundant block if it exists.
                                    
                                    // Actually, looking at the error message line numbers: 249, 264, 276...
                                    // These correspond to the FIRST match block.
                                    // I will remove this block entirely as it seems I intended to replace it with the separated rendering blocks but maybe I didn't delete it?
                                    // Or maybe I intended to keep it?
                                    // Let's replace the whole section with just the separated rendering blocks.
                                    
                                     // Re-instantiate projection to separate draws
                                     
                                     // 1. Draw Continents (Fill)
                                     {
                                         let continents_svg = match current_projection {
                                              GeoProjectionType::Mercator => { let p = Mercator::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&world_continents()) },
                                              GeoProjectionType::Equirectangular => { let p = Equirectangular::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&world_continents()) },
                                              GeoProjectionType::Orthographic => { let p = Orthographic::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&world_continents()) },
                                              GeoProjectionType::Stereographic => { let p = Stereographic::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&world_continents()) },
                                              GeoProjectionType::ConicEqualArea => { let p = ConicEqualArea::new().scale(scale).translate(center_x, center_y).center(0.0, 30.0).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&world_continents()) },
                                         };
                                         
                                         // VERY simple M/L parser for demo purposes
                                         // In production, use a real SVG path parser or d3rs should output Path events directly
                                         // Use PathBuilder::fill() for filled shapes
                                         let mut builder = PathBuilder::fill();
                                         let tokens = continents_svg.replace("M", " M ").replace("L", " L ").replace("Z", " Z ").replace("z", " Z ");
                                         let parts: Vec<&str> = tokens.split_whitespace().collect();
                                         let mut i = 0;
                                         while i < parts.len() {
                                             match parts[i] {
                                                 "M" => {
                                                     if i+1 < parts.len() {
                                                         let coords: Vec<&str> = parts[i+1].split(',').collect();
                                                         if coords.len() == 2 {
                                                             if let (Ok(x), Ok(y)) = (coords[0].parse::<f32>(), coords[1].parse::<f32>()) {
                                                                 builder.move_to(bounds.origin + point(px(x), px(y)));
                                                             }
                                                         }
                                                         i += 2;
                                                     } else { i += 1; }
                                                 },
                                                 "L" => {
                                                     if i+1 < parts.len() {
                                                         let coords: Vec<&str> = parts[i+1].split(',').collect();
                                                         if coords.len() == 2 {
                                                             if let (Ok(x), Ok(y)) = (coords[0].parse::<f32>(), coords[1].parse::<f32>()) {
                                                                 builder.line_to(bounds.origin + point(px(x), px(y)));
                                                             }
                                                         }
                                                         i += 2;
                                                     } else { i += 1; }
                                                 },
                                                 "Z" => {
                                                     builder.close();
                                                     i += 1;
                                                 },
                                                 _ => i += 1,
                                             }
                                         }
                                         
                                         if let Ok(path) = builder.build() {
                                             // paint_path takes (path, color) in recent GPUI versions?
                                             // Looking at the error: unexpected argument #3 of type `gpui::Rgba`
                                             // It seems paint_path expects 2 arguments: self and path? No, method signature is `paint_path(&mut self, path, color)`.
                                             // Let's try passing just the path and color (fill color).
                                             window.paint_path(path, rgb(0xd6e4ff));
                                         }
                                     }

                                    // 2. Draw Graticule
                                    let graticule = Graticule::new().step([30.0, 30.0]);
                                    for line in graticule.lines() {
                                        let grid_svg = match current_projection {
                                              GeoProjectionType::Mercator => { let p = Mercator::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&d3rs::geo::GeoJsonGeometry::LineString(line)) },
                                              GeoProjectionType::Equirectangular => { let p = Equirectangular::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&d3rs::geo::GeoJsonGeometry::LineString(line)) },
                                              GeoProjectionType::Orthographic => { let p = Orthographic::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&d3rs::geo::GeoJsonGeometry::LineString(line)) },
                                              GeoProjectionType::Stereographic => { let p = Stereographic::new().scale(scale).translate(center_x, center_y).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&d3rs::geo::GeoJsonGeometry::LineString(line)) },
                                              GeoProjectionType::ConicEqualArea => { let p = ConicEqualArea::new().scale(scale).translate(center_x, center_y).center(0.0, 30.0).rotate(rotation.lambda, rotation.phi, rotation.gamma); GeoPath::new(p).render(&d3rs::geo::GeoJsonGeometry::LineString(line)) },
                                         };

                                        // Use PathBuilder::stroke() for lines
                                        let mut builder = PathBuilder::stroke(px(1.0));
                                         let tokens = grid_svg.replace("M", " M ").replace("L", " L ");
                                         let parts: Vec<&str> = tokens.split_whitespace().collect();
                                         let mut i = 0;
                                         while i < parts.len() {
                                              match parts[i] {
                                                 "M" => {
                                                     if i+1 < parts.len() {
                                                         let coords: Vec<&str> = parts[i+1].split(',').collect();
                                                         if coords.len() == 2 {
                                                             if let (Ok(x), Ok(y)) = (coords[0].parse::<f32>(), coords[1].parse::<f32>()) {
                                                                 builder.move_to(bounds.origin + point(px(x), px(y)));
                                                             }
                                                         }
                                                         i += 2;
                                                     } else { i += 1; }
                                                 },
                                                 "L" => {
                                                     if i+1 < parts.len() {
                                                         let coords: Vec<&str> = parts[i+1].split(',').collect();
                                                         if coords.len() == 2 {
                                                             if let (Ok(x), Ok(y)) = (coords[0].parse::<f32>(), coords[1].parse::<f32>()) {
                                                                 builder.line_to(bounds.origin + point(px(x), px(y)));
                                                             }
                                                         }
                                                         i += 2;
                                                     } else { i += 1; }
                                                 },
                                                 _ => i += 1,
                                             }
                                         }
                                         // Don't close grid lines
                                         if let Ok(path) = builder.build() {
                                            window.paint_path(path, rgba(0x00000033));
                                         }
                                    }
                                }
                            )
                        )
                        // Render city markers (kept as overlay divs for easy text handling)
                        .children(render_cities(
                            current_projection,
                            map_width,
                            map_height,
                            center_x,
                            center_y,
                            rotation_lon,
                            rotation_lat,
                        )),
                ),
        )
        // Rotation controls
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Rotation Controls:"),
                )
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(div().text_sm().child("Longitude:"))
                                .child(
                                    div()
                                        .id("lon-minus")
                                        .px_3()
                                        .py_1()
                                        .bg(rgb(0xe8e8e8))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0xd0d0d0)))
                                        .child("-30°")
                                        .on_click(cx.listener(|this, _, _window, _cx| {
                                            this.geo_rotation_lon -= 30.0;
                                        })),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .w(px(60.0))
                                        .text_center()
                                        .child(format!("{:.0}°", rotation_lon)),
                                )
                                .child(
                                    div()
                                        .id("lon-plus")
                                        .px_3()
                                        .py_1()
                                        .bg(rgb(0xe8e8e8))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0xd0d0d0)))
                                        .child("+30°")
                                        .on_click(cx.listener(|this, _, _window, _cx| {
                                            this.geo_rotation_lon += 30.0;
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(div().text_sm().child("Latitude:"))
                                .child(
                                    div()
                                        .id("lat-minus")
                                        .px_3()
                                        .py_1()
                                        .bg(rgb(0xe8e8e8))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0xd0d0d0)))
                                        .child("-15°")
                                        .on_click(cx.listener(|this, _, _window, _cx| {
                                            this.geo_rotation_lat = (this.geo_rotation_lat - 15.0).max(-60.0);
                                        })),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .w(px(60.0))
                                        .text_center()
                                        .child(format!("{:.0}°", rotation_lat)),
                                )
                                .child(
                                    div()
                                        .id("lat-plus")
                                        .px_3()
                                        .py_1()
                                        .bg(rgb(0xe8e8e8))
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(rgb(0xd0d0d0)))
                                        .child("+15°")
                                        .on_click(cx.listener(|this, _, _window, _cx| {
                                            this.geo_rotation_lat = (this.geo_rotation_lat + 15.0).min(60.0);
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .id("reset-rotation")
                                .px_3()
                                .py_1()
                                .bg(rgb(0x007acc))
                                .text_color(rgb(0xffffff))
                                .rounded_md()
                                .cursor_pointer()
                                .hover(|s| s.bg(rgb(0x005a9e)))
                                .child("Reset")
                                .on_click(cx.listener(|this, _, _window, _cx| {
                                    this.geo_rotation_lon = 0.0;
                                    this.geo_rotation_lat = 0.0;
                                })),
                        ),
                ),
        )
        // Legend
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .mt_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Legend:"),
                )
                .child(
                    div()
                        .flex()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().w(px(16.0)).h(px(2.0)).bg(rgba(0x00000033))) // Updated color to match
                                .child(div().text_sm().text_color(rgb(0x666666)).child("Graticule (30° grid)")),
                        )
                         .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().w(px(16.0)).h(px(16.0)).bg(rgb(0xd6e4ff)).border_1().border_color(rgb(0x3399ff)))
                                .child(div().text_sm().text_color(rgb(0x666666)).child("Continents")),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(rgb(0xd62728)))
                                .child(div().text_sm().text_color(rgb(0x666666)).child("Cities")),
                        ),
                ),
        )
        // Projection descriptions (kept)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_4()
                .bg(rgb(0xf5f5f5))
                .rounded_lg()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Projection Properties:"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x333333))
                        .child(projection_description(current_projection)),
                ),
        )
}

/// Project a point using the selected projection type
fn project_point(
    lon: f64,
    lat: f64,
    proj_type: GeoProjectionType,
    map_width: f64,
    map_height: f64,
    center_x: f64,
    center_y: f64,
    rotation_lon: f64,
    rotation_lat: f64,
) -> Option<(f64, f64)> {
    // Apply rotation to the longitude/latitude
    // NOTE: d3rs::geo::rotate is a trait method, but here we do it manually or via Rotation helper if accessible.
    // However, the previous project_point implementation did simple addition, which is WRONG for latitude.
    // The previous implementation was:
    // let rotated_lon = lon + rotation_lon;
    // let rotated_lat = lat + rotation_lat;
    
    // Now we use the proper Rotation helper from d3rs
    let rot = Rotation::new().angles(rotation_lon, rotation_lat, 0.0);
    let (rotated_lon, rotated_lat) = rot.rotate(lon, lat);

    // Check if point is visible (especially for azimuthal projections)
    match proj_type {
        GeoProjectionType::Orthographic | GeoProjectionType::Stereographic => {
            // Simple visibility check for azimuthal projections
            // This is a rough approximation. True check uses clipping.
            // For Orthographic, clip if cos(c) < 0 where c is distance from center.
            // Here we just check if it's "behind" the globe broadly.
            // A point is visible if dot product of normal and view vector > 0.
            // For simplicity, let's trust d3rs projection might return values, but we need to filter NaNs.
            
            // The previous check was:
            // let lon_diff = rotated_lon.to_radians().cos();
            // let lat_cos = rotated_lat.to_radians().cos();
            // if lon_diff * lat_cos < 0.0 { return None; }
        }
        _ => {}
    }

    let scale = match proj_type {
        GeoProjectionType::Mercator => map_height / 3.0,
        GeoProjectionType::Equirectangular => map_width / 360.0 * 0.9,
        GeoProjectionType::Orthographic => map_height / 2.5,
        GeoProjectionType::Stereographic => map_height / 4.0,
        GeoProjectionType::ConicEqualArea => map_height / 3.5,
    };

    let (x, y) = match proj_type {
        GeoProjectionType::Mercator => {
            let proj = Mercator::new().scale(scale).translate(center_x, center_y);
            // We already rotated the point, so we project directly? 
            // NO, `projejct` expects unrotated if the projection itself handles rotation.
            // But here we rotated MANUALLY above. So we project the rotated coords.
            proj.project(rotated_lon, rotated_lat)
        }
        GeoProjectionType::Equirectangular => {
            let proj = Equirectangular::new().scale(scale).translate(center_x, center_y);
            proj.project(rotated_lon, rotated_lat)
        }
        GeoProjectionType::Orthographic => {
            let proj = Orthographic::new().scale(scale).translate(center_x, center_y);
            proj.project(rotated_lon, rotated_lat)
        }
        GeoProjectionType::Stereographic => {
            let proj = Stereographic::new().scale(scale).translate(center_x, center_y);
            proj.project(rotated_lon, rotated_lat)
        }
        GeoProjectionType::ConicEqualArea => {
            let proj = ConicEqualArea::new().scale(scale).translate(center_x, center_y).center(0.0, 30.0);
            proj.project(rotated_lon, rotated_lat)
        }
    };

    // Check bounds
    if x.is_finite() && y.is_finite() {
        // Relax strict bounds check to allow points slightly off-canvas (clipping handles it)
        Some((x, y))
    } else {
        None
    }
}

/// Render city markers
fn render_cities(
    proj_type: GeoProjectionType,
    map_width: f64,
    map_height: f64,
    center_x: f64,
    center_y: f64,
    rotation_lon: f64,
    rotation_lat: f64,
) -> Vec<Div> {
    let mut elements = Vec::new();

    for &(name, lon, lat) in CITIES {
        if let Some((x, y)) = project_point(
            lon,
            lat,
            proj_type,
            map_width,
            map_height,
            center_x,
            center_y,
            rotation_lon,
            rotation_lat,
        ) {
            
            // Basic visibility check for Orthographic (hide points behind globe)
            if matches!(proj_type, GeoProjectionType::Orthographic) {
                // If distance from center > radius (approx), hide
                let dx = x - center_x;
                let dy = y - center_y;
                let r = map_height / 2.5; 
                if dx*dx + dy*dy > r*r + 1.0 { // tolerance
                    continue;
                }
            }

            // City dot
            elements.push(
                div()
                    .absolute()
                    .left(px(x as f32 - 4.0))
                    .top(px(y as f32 - 4.0))
                    .w(px(8.0))
                    .h(px(8.0))
                    .rounded_full()
                    .bg(rgb(0xd62728))
                    .border_1()
                    .border_color(rgb(0xffffff)),
            );
            // City label
            elements.push(
                div()
                    .absolute()
                    .left(px(x as f32 + 6.0))
                    .top(px(y as f32 - 6.0))
                    .text_xs()
                    .text_color(rgb(0x333333))
                    .bg(rgba(0xffffffcc))
                    .px_1()
                    .rounded(px(2.0))
                    .child(name),
            );
        }
    }

    elements
}

/// Get description for a projection
fn projection_description(proj_type: GeoProjectionType) -> &'static str {
    match proj_type {
        GeoProjectionType::Mercator =>
            "Mercator: A conformal cylindrical projection that preserves angles and shapes locally. \
             Used for navigation and web maps. Distorts size near the poles.",
        GeoProjectionType::Equirectangular =>
            "Equirectangular (Plate Carrée): The simplest projection that maps longitude and latitude \
             directly to x and y. Preserves neither area nor shape, but is easy to compute.",
        GeoProjectionType::Orthographic =>
            "Orthographic: Shows the Earth as seen from space. An azimuthal projection that can only \
             display one hemisphere at a time. Useful for visualizing the globe.",
        GeoProjectionType::Stereographic =>
            "Stereographic: A conformal azimuthal projection. Preserves angles and local shapes. \
             Used in crystallography and complex analysis.",
        GeoProjectionType::ConicEqualArea =>
            "Conic Equal-Area (Albers): An equal-area projection using two standard parallels. \
             Excellent for regions with large east-west extent like the United States.",
    }
}
