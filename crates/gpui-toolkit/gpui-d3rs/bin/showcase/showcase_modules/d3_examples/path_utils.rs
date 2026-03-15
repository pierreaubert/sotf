use gpui::*;

/// SVG Path parser to GPUI Path
///
/// Supports: M, L, H, V, C, Z (absolute) and m, l, h, v, c, z (relative)
pub fn parse_svg_path(d: &str, bounds: Bounds<Pixels>) -> Option<Path<Pixels>> {
    let mut builder = PathBuilder::fill();
    let origin = bounds.origin;

    // Tokenize: insert spaces around command letters, then split
    let mut spaced = String::with_capacity(d.len() * 2);
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            spaced.push(' ');
            spaced.push(ch);
            spaced.push(' ');
        } else if ch == ',' {
            spaced.push(' ');
        } else if ch == '-' && !spaced.ends_with(' ') && !spaced.ends_with('e') && !spaced.ends_with('E') {
            // Negative number as implicit separator (e.g. "10-5" → "10 -5")
            spaced.push(' ');
            spaced.push(ch);
        } else {
            spaced.push(ch);
        }
    }

    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut idx = 0;
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;

    while idx < tokens.len() {
        match tokens[idx] {
            "M" => {
                if let Some((x, y)) = parse_xy(&tokens, idx + 1) {
                    cx = x;
                    cy = y;
                    builder.move_to(origin + point(px(x), px(y)));
                    idx += 3;
                } else {
                    idx += 1;
                }
            }
            "m" => {
                if let Some((dx, dy)) = parse_xy(&tokens, idx + 1) {
                    cx += dx;
                    cy += dy;
                    builder.move_to(origin + point(px(cx), px(cy)));
                    idx += 3;
                } else {
                    idx += 1;
                }
            }
            "L" => {
                if let Some((x, y)) = parse_xy(&tokens, idx + 1) {
                    cx = x;
                    cy = y;
                    builder.line_to(origin + point(px(x), px(y)));
                    idx += 3;
                } else {
                    idx += 1;
                }
            }
            "l" => {
                if let Some((dx, dy)) = parse_xy(&tokens, idx + 1) {
                    cx += dx;
                    cy += dy;
                    builder.line_to(origin + point(px(cx), px(cy)));
                    idx += 3;
                } else {
                    idx += 1;
                }
            }
            "H" => {
                if let Some(x) = parse_f32(&tokens, idx + 1) {
                    cx = x;
                    builder.line_to(origin + point(px(cx), px(cy)));
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
            "h" => {
                if let Some(dx) = parse_f32(&tokens, idx + 1) {
                    cx += dx;
                    builder.line_to(origin + point(px(cx), px(cy)));
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
            "V" => {
                if let Some(y) = parse_f32(&tokens, idx + 1) {
                    cy = y;
                    builder.line_to(origin + point(px(cx), px(cy)));
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
            "v" => {
                if let Some(dy) = parse_f32(&tokens, idx + 1) {
                    cy += dy;
                    builder.line_to(origin + point(px(cx), px(cy)));
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
            "C" => {
                // Cubic bezier: C x1 y1 x2 y2 x y
                // Approximate with line segments
                if idx + 6 < tokens.len() {
                    if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                        parse_f32(&tokens, idx + 1),
                        parse_f32(&tokens, idx + 2),
                        parse_f32(&tokens, idx + 3),
                        parse_f32(&tokens, idx + 4),
                        parse_f32(&tokens, idx + 5),
                        parse_f32(&tokens, idx + 6),
                    ) {
                        // Subdivide cubic bezier into line segments
                        let steps = 16;
                        for i in 1..=steps {
                            let t = i as f32 / steps as f32;
                            let u = 1.0 - t;
                            let px = u * u * u * cx
                                + 3.0 * u * u * t * x1
                                + 3.0 * u * t * t * x2
                                + t * t * t * x;
                            let py = u * u * u * cy
                                + 3.0 * u * u * t * y1
                                + 3.0 * u * t * t * y2
                                + t * t * t * y;
                            builder.line_to(origin + point(gpui::px(px), gpui::px(py)));
                        }
                        cx = x;
                        cy = y;
                        idx += 7;
                    } else {
                        idx += 1;
                    }
                } else {
                    idx += 1;
                }
            }
            "c" => {
                // Relative cubic bezier
                if idx + 6 < tokens.len() {
                    if let (Some(dx1), Some(dy1), Some(dx2), Some(dy2), Some(dx), Some(dy)) = (
                        parse_f32(&tokens, idx + 1),
                        parse_f32(&tokens, idx + 2),
                        parse_f32(&tokens, idx + 3),
                        parse_f32(&tokens, idx + 4),
                        parse_f32(&tokens, idx + 5),
                        parse_f32(&tokens, idx + 6),
                    ) {
                        let x1 = cx + dx1;
                        let y1 = cy + dy1;
                        let x2 = cx + dx2;
                        let y2 = cy + dy2;
                        let x = cx + dx;
                        let y = cy + dy;
                        let steps = 16;
                        for i in 1..=steps {
                            let t = i as f32 / steps as f32;
                            let u = 1.0 - t;
                            let px = u * u * u * cx
                                + 3.0 * u * u * t * x1
                                + 3.0 * u * t * t * x2
                                + t * t * t * x;
                            let py = u * u * u * cy
                                + 3.0 * u * u * t * y1
                                + 3.0 * u * t * t * y2
                                + t * t * t * y;
                            builder.line_to(origin + point(gpui::px(px), gpui::px(py)));
                        }
                        cx = x;
                        cy = y;
                        idx += 7;
                    } else {
                        idx += 1;
                    }
                } else {
                    idx += 1;
                }
            }
            "Z" | "z" => {
                builder.close();
                idx += 1;
            }
            _ => idx += 1,
        }
    }

    builder.build().ok()
}

fn parse_xy(tokens: &[&str], start: usize) -> Option<(f32, f32)> {
    if start + 1 < tokens.len() {
        if let (Ok(x), Ok(y)) = (tokens[start].parse::<f32>(), tokens[start + 1].parse::<f32>()) {
            if x.is_finite() && y.is_finite() {
                return Some((x, y));
            }
        }
    }
    None
}

fn parse_f32(tokens: &[&str], idx: usize) -> Option<f32> {
    if idx < tokens.len() {
        if let Ok(v) = tokens[idx].parse::<f32>() {
            if v.is_finite() {
                return Some(v);
            }
        }
    }
    None
}
