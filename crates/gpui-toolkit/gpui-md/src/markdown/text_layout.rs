//! Text layout integration with gpui-pretext.
//!
//! Uses Knuth-Plass optimal line breaking for paragraph text
//! in the preview pane.

use gpui::*;
use gpui_pretext::{
    EngineProfile, KnuthPlassParams, PrepareOptions, TextMeasure, layout_with_lines_optimal,
    prepare_with_segments,
};

/// Proportional text measure that estimates width based on character categories.
struct ProportionalMeasure {
    base_width: f64,
}

impl TextMeasure for ProportionalMeasure {
    fn measure_width(&self, text: &str) -> f64 {
        let mut width = 0.0;
        for ch in text.chars() {
            width += match ch {
                ' ' => self.base_width * 0.35,
                'i' | 'l' | 'j' | 't' | 'f' | 'r' | '.' | ',' | ':' | ';' | '!' | '\'' => {
                    self.base_width * 0.4
                }
                'm' | 'w' | 'M' | 'W' => self.base_width * 0.85,
                'A'..='Z' => self.base_width * 0.7,
                _ => self.base_width * 0.55,
            };
        }
        width
    }
}

/// Layout paragraph text using Knuth-Plass optimal line breaking.
///
/// Returns a Vec of (line_text, line_width, is_last_line) tuples.
/// The caller can use these to render justified text.
pub fn layout_paragraph(text: &str, max_width_px: f32, font_size_px: f32) -> Vec<JustifiedLine> {
    if text.is_empty() {
        return vec![JustifiedLine {
            text: String::new(),
            width: 0.0,
            is_last: true,
        }];
    }

    let measure = ProportionalMeasure {
        base_width: font_size_px as f64,
    };
    let profile = EngineProfile::default();
    let options = PrepareOptions::default();
    let line_height = font_size_px as f64 * 1.5;
    let max_width = max_width_px as f64;

    let kp_params = KnuthPlassParams {
        line_penalty: 0.0,
        hyphen_penalty: 50.0,
        flagged_demerits: 3000.0,
        fitness_demerits: 10000.0,
        tolerance: 2.0,
        looseness_recovery: true,
    };

    let prepared = prepare_with_segments(text, &measure, &profile, &options);
    let result = layout_with_lines_optimal(&prepared, max_width, line_height, &profile, &kp_params);

    let total_lines = result.lines.len();
    result
        .lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| JustifiedLine {
            text: line.text,
            width: line.width as f32,
            is_last: i == total_lines - 1,
        })
        .collect()
}

/// A line of text after Knuth-Plass layout, ready for justified rendering.
pub struct JustifiedLine {
    pub text: String,
    pub width: f32,
    /// Last line of a paragraph — should be left-aligned, not justified.
    pub is_last: bool,
}

/// Render justified text lines as GPUI elements.
///
/// Each line except the last is stretched to fill `max_width` by distributing
/// extra space between words.
pub fn render_justified_lines(lines: &[JustifiedLine], max_width: f32) -> Vec<AnyElement> {
    let mut elements = Vec::new();

    for line in lines {
        if line.text.is_empty() {
            elements.push(div().min_h(px(10.0)).into_any_element());
            continue;
        }

        if line.is_last || line.width >= max_width * 0.95 {
            // Last line or already fills width — left-align
            elements.push(div().child(line.text.clone()).into_any_element());
        } else {
            // Justify by distributing space between words
            let words: Vec<&str> = line.text.split_whitespace().collect();
            if words.len() <= 1 {
                elements.push(div().child(line.text.clone()).into_any_element());
                continue;
            }

            let gap_count = words.len() - 1;
            let slack = max_width - line.width;
            let extra_per_gap = slack / gap_count as f32;

            let mut word_elements: Vec<AnyElement> = Vec::new();
            for (i, word) in words.iter().enumerate() {
                word_elements.push(div().child(word.to_string()).into_any_element());
                if i < gap_count {
                    // Inter-word gap with extra space for justification
                    let gap_width = extra_per_gap;
                    word_elements
                        .push(div().w(px(gap_width)).flex_shrink_0().into_any_element());
                }
            }

            elements.push(
                div()
                    .flex()
                    .flex_row()
                    .w(px(max_width))
                    .children(word_elements)
                    .into_any_element(),
            );
        }
    }

    elements
}
