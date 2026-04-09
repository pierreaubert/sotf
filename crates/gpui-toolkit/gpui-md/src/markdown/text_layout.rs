//! Text layout integration with gpui-pretext.
//!
//! Uses Knuth-Plass optimal line breaking for paragraph text
//! in the preview pane, with accurate GPUI text system measurement.

use std::sync::Arc;

use gpui::*;
use gpui_pretext::{
    EngineProfile, KnuthPlassParams, PrepareOptions, TextMeasure, layout_with_lines_optimal,
    prepare_with_segments,
};

/// Text measure backed by GPUI's platform text system for accurate glyph widths.
struct GpuiTextMeasure {
    text_system: Arc<WindowTextSystem>,
    font: Font,
    font_size: Pixels,
}

impl TextMeasure for GpuiTextMeasure {
    fn measure_width(&self, text: &str) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        // layout_line panics on newlines — filter them out
        let clean = if text.contains('\n') || text.contains('\r') {
            text.replace(['\n', '\r'], " ")
        } else {
            text.to_string()
        };
        let run = TextRun {
            len: clean.len(),
            font: self.font.clone(),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let layout = self.text_system.layout_line(&clean, self.font_size, &[run], None);
        let w: f32 = layout.width.into();
        w as f64
    }
}

/// Layout paragraph text using Knuth-Plass optimal line breaking
/// with accurate GPUI text system measurement.
pub fn layout_paragraph(
    text: &str,
    max_width_px: f32,
    font_size_px: f32,
    text_system: &Arc<WindowTextSystem>,
) -> Vec<JustifiedLine> {
    if text.is_empty() || max_width_px <= 0.0 {
        return vec![JustifiedLine {
            text: text.to_string(),
            width: 0.0,
            is_last: true,
        }];
    }

    let measure = GpuiTextMeasure {
        text_system: text_system.clone(),
        font: Font {
            family: SharedString::from(".SystemUIFont"),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        },
        font_size: px(font_size_px),
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
