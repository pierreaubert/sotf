/// Main public API for text measurement and layout, ported from chenglou/pretext.
///
/// Two-phase approach:
/// 1. **Prepare**: Analyze and measure text once, producing a `PreparedText`.
/// 2. **Layout**: Use cached widths for fast arithmetic-only line breaking.
use unicode_segmentation::UnicodeSegmentation;

use crate::analysis::{
    analyze_text, ends_with_closing_quote, is_cjk, is_kinsoku_end, is_kinsoku_start,
    is_left_sticky_punctuation, AnalysisChunk, AnalysisProfile, SegmentBreakKind, TextAnalysis,
    WhiteSpaceMode,
};
use crate::bidi::compute_segment_levels;
use crate::line_break::{
    count_prepared_lines, layout_next_line_range, walk_prepared_lines, InternalLayoutLine,
    LineBreakCursor, PreparedLineBreakData, PreparedLineChunk,
};
use crate::measurement::{EngineProfile, MeasureCache, TextMeasure};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Opaque handle to prepared text data. Returned by [`prepare`].
#[derive(Debug, Clone)]
pub struct PreparedText {
    core: PreparedCore,
}

/// Prepared text with segment strings exposed for custom rendering.
#[derive(Debug, Clone)]
pub struct PreparedTextWithSegments {
    core: PreparedCore,
    pub segments: Vec<String>,
}

#[derive(Debug, Clone)]
struct PreparedCore {
    widths: Vec<f64>,
    line_end_fit_advances: Vec<f64>,
    line_end_paint_advances: Vec<f64>,
    kinds: Vec<SegmentBreakKind>,
    simple_line_walk_fast_path: bool,
    #[allow(dead_code)]
    seg_levels: Option<Vec<i8>>,
    breakable_widths: Vec<Option<Vec<f64>>>,
    breakable_prefix_widths: Vec<Option<Vec<f64>>>,
    discretionary_hyphen_width: f64,
    tab_stop_advance: f64,
    chunks: Vec<PreparedLineChunk>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutCursor {
    pub segment_index: usize,
    pub grapheme_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutResult {
    pub line_count: usize,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutLine {
    pub text: String,
    pub width: f64,
    pub start: LayoutCursor,
    pub end: LayoutCursor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutLineRange {
    pub width: f64,
    pub start: LayoutCursor,
    pub end: LayoutCursor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutLinesResult {
    pub line_count: usize,
    pub height: f64,
    pub lines: Vec<LayoutLine>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PrepareProfile {
    pub analysis_segments: usize,
    pub prepared_segments: usize,
    pub breakable_segments: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct PrepareOptions {
    pub white_space: WhiteSpaceMode,
}

impl Default for PrepareOptions {
    fn default() -> Self {
        Self {
            white_space: WhiteSpaceMode::Normal,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn to_line_break_data(core: &PreparedCore) -> PreparedLineBreakData {
    PreparedLineBreakData {
        widths: core.widths.clone(),
        line_end_fit_advances: core.line_end_fit_advances.clone(),
        line_end_paint_advances: core.line_end_paint_advances.clone(),
        kinds: core.kinds.clone(),
        simple_line_walk_fast_path: core.simple_line_walk_fast_path,
        breakable_widths: core.breakable_widths.clone(),
        breakable_prefix_widths: core.breakable_prefix_widths.clone(),
        discretionary_hyphen_width: core.discretionary_hyphen_width,
        tab_stop_advance: core.tab_stop_advance,
        chunks: core.chunks.clone(),
    }
}

fn to_layout_line_range(line: &InternalLayoutLine) -> LayoutLineRange {
    LayoutLineRange {
        width: line.width,
        start: LayoutCursor {
            segment_index: line.start_segment_index,
            grapheme_index: line.start_grapheme_index,
        },
        end: LayoutCursor {
            segment_index: line.end_segment_index,
            grapheme_index: line.end_grapheme_index,
        },
    }
}

fn map_analysis_chunks_to_prepared_chunks(
    chunks: &[AnalysisChunk],
    prepared_start_by_analysis: &[usize],
    prepared_end_by_analysis: &[usize],
) -> Vec<PreparedLineChunk> {
    let fallback = prepared_end_by_analysis
        .last()
        .copied()
        .unwrap_or(0);

    chunks
        .iter()
        .map(|chunk| {
            let start = if chunk.start_segment_index < prepared_start_by_analysis.len() {
                prepared_start_by_analysis[chunk.start_segment_index]
            } else {
                fallback
            };
            let end = if chunk.end_segment_index < prepared_start_by_analysis.len() {
                prepared_start_by_analysis[chunk.end_segment_index]
            } else {
                fallback
            };
            let consumed_end =
                if chunk.consumed_end_segment_index < prepared_start_by_analysis.len() {
                    prepared_start_by_analysis[chunk.consumed_end_segment_index]
                } else {
                    fallback
                };
            PreparedLineChunk {
                start_segment_index: start,
                end_segment_index: end,
                consumed_end_segment_index: consumed_end,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// measureAnalysis — core measurement
// ---------------------------------------------------------------------------

fn measure_analysis(
    analysis: &TextAnalysis,
    measure: &dyn TextMeasure,
    profile: &EngineProfile,
    include_segments: bool,
) -> (PreparedCore, Option<Vec<String>>) {
    let mut cache = MeasureCache::new();

    let discretionary_hyphen_width = cache.get_width("-", measure);
    let space_width = cache.get_width(" ", measure);
    let tab_stop_advance = space_width * 8.0;

    if analysis.is_empty() {
        return (
            PreparedCore {
                widths: Vec::new(),
                line_end_fit_advances: Vec::new(),
                line_end_paint_advances: Vec::new(),
                kinds: Vec::new(),
                simple_line_walk_fast_path: true,
                seg_levels: None,
                breakable_widths: Vec::new(),
                breakable_prefix_widths: Vec::new(),
                discretionary_hyphen_width: 0.0,
                tab_stop_advance: 0.0,
                chunks: Vec::new(),
            },
            if include_segments {
                Some(Vec::new())
            } else {
                None
            },
        );
    }

    let mut widths = Vec::new();
    let mut line_end_fit_advances = Vec::new();
    let mut line_end_paint_advances = Vec::new();
    let mut kinds = Vec::new();
    let mut simple_fast_path = analysis.chunks.len() <= 1;
    let mut seg_starts: Vec<usize> = Vec::new();
    let mut breakable_widths_out: Vec<Option<Vec<f64>>> = Vec::new();
    let mut breakable_prefix_widths_out: Vec<Option<Vec<f64>>> = Vec::new();
    let mut segments: Vec<String> = Vec::new();
    let mut prepared_start_by_analysis = vec![0usize; analysis.len()];
    let mut prepared_end_by_analysis = vec![0usize; analysis.len()];

    let push_segment = |text: &str,
                        width: f64,
                        fit_adv: f64,
                        paint_adv: f64,
                        kind: SegmentBreakKind,
                        start: usize,
                        breakable: Option<Vec<f64>>,
                        breakable_prefix: Option<Vec<f64>>,
                        widths_v: &mut Vec<f64>,
                        fit_v: &mut Vec<f64>,
                        paint_v: &mut Vec<f64>,
                        kinds_v: &mut Vec<SegmentBreakKind>,
                        starts_v: &mut Vec<usize>,
                        bw_v: &mut Vec<Option<Vec<f64>>>,
                        bpw_v: &mut Vec<Option<Vec<f64>>>,
                        segs_v: &mut Vec<String>,
                        simple: &mut bool,
                        include_segs: bool| {
        if kind != SegmentBreakKind::Text
            && kind != SegmentBreakKind::Space
            && kind != SegmentBreakKind::ZeroWidthBreak
        {
            *simple = false;
        }
        widths_v.push(width);
        fit_v.push(fit_adv);
        paint_v.push(paint_adv);
        kinds_v.push(kind);
        starts_v.push(start);
        bw_v.push(breakable);
        bpw_v.push(breakable_prefix);
        if include_segs {
            segs_v.push(text.to_string());
        }
    };

    for mi in 0..analysis.len() {
        prepared_start_by_analysis[mi] = widths.len();
        let seg_text = &analysis.texts[mi];
        let seg_word_like = analysis.is_word_like[mi];
        let seg_kind = analysis.kinds[mi];
        let seg_start = analysis.starts[mi];

        match seg_kind {
            SegmentBreakKind::SoftHyphen => {
                push_segment(
                    seg_text,
                    0.0,
                    discretionary_hyphen_width,
                    discretionary_hyphen_width,
                    seg_kind,
                    seg_start,
                    None,
                    None,
                    &mut widths,
                    &mut line_end_fit_advances,
                    &mut line_end_paint_advances,
                    &mut kinds,
                    &mut seg_starts,
                    &mut breakable_widths_out,
                    &mut breakable_prefix_widths_out,
                    &mut segments,
                    &mut simple_fast_path,
                    include_segments,
                );
                prepared_end_by_analysis[mi] = widths.len();
                continue;
            }
            SegmentBreakKind::HardBreak | SegmentBreakKind::Tab => {
                push_segment(
                    seg_text,
                    0.0,
                    0.0,
                    0.0,
                    seg_kind,
                    seg_start,
                    None,
                    None,
                    &mut widths,
                    &mut line_end_fit_advances,
                    &mut line_end_paint_advances,
                    &mut kinds,
                    &mut seg_starts,
                    &mut breakable_widths_out,
                    &mut breakable_prefix_widths_out,
                    &mut segments,
                    &mut simple_fast_path,
                    include_segments,
                );
                prepared_end_by_analysis[mi] = widths.len();
                continue;
            }
            _ => {}
        }

        // Check CJK — split into per-grapheme units
        let seg_metrics = cache.get_segment_metrics(seg_text, measure);
        if seg_kind == SegmentBreakKind::Text && seg_metrics.contains_cjk {
            let mut unit_text = String::new();
            let mut unit_start = 0usize;

            for (gi, grapheme) in seg_text.grapheme_indices(true) {
                if unit_text.is_empty() {
                    unit_text = grapheme.to_string();
                    unit_start = gi;
                    continue;
                }

                let first_char = grapheme.chars().next().unwrap_or('\0');
                if is_kinsoku_end(unit_text.chars().next().unwrap_or('\0'))
                    || is_kinsoku_start(first_char)
                    || is_left_sticky_punctuation(first_char)
                    || (profile.carry_cjk_after_closing_quote
                        && is_cjk(grapheme)
                        && ends_with_closing_quote(&unit_text))
                {
                    unit_text.push_str(grapheme);
                    continue;
                }

                let w = cache.get_width(&unit_text, measure);
                push_segment(
                    &unit_text,
                    w,
                    w,
                    w,
                    SegmentBreakKind::Text,
                    seg_start + unit_start,
                    None,
                    None,
                    &mut widths,
                    &mut line_end_fit_advances,
                    &mut line_end_paint_advances,
                    &mut kinds,
                    &mut seg_starts,
                    &mut breakable_widths_out,
                    &mut breakable_prefix_widths_out,
                    &mut segments,
                    &mut simple_fast_path,
                    include_segments,
                );

                unit_text = grapheme.to_string();
                unit_start = gi;
            }

            if !unit_text.is_empty() {
                let w = cache.get_width(&unit_text, measure);
                push_segment(
                    &unit_text,
                    w,
                    w,
                    w,
                    SegmentBreakKind::Text,
                    seg_start + unit_start,
                    None,
                    None,
                    &mut widths,
                    &mut line_end_fit_advances,
                    &mut line_end_paint_advances,
                    &mut kinds,
                    &mut seg_starts,
                    &mut breakable_widths_out,
                    &mut breakable_prefix_widths_out,
                    &mut segments,
                    &mut simple_fast_path,
                    include_segments,
                );
            }
            prepared_end_by_analysis[mi] = widths.len();
            continue;
        }

        let w = cache.get_width(seg_text, measure);
        let line_end_fit = match seg_kind {
            SegmentBreakKind::Space
            | SegmentBreakKind::PreservedSpace
            | SegmentBreakKind::ZeroWidthBreak => 0.0,
            _ => w,
        };
        let line_end_paint = match seg_kind {
            SegmentBreakKind::Space | SegmentBreakKind::ZeroWidthBreak => 0.0,
            _ => w,
        };

        let (breakable, breakable_prefix) = if seg_word_like && seg_text.len() > 1 {
            let bw = cache.get_grapheme_widths(seg_text, measure);
            let bpw = if profile.prefer_prefix_widths_for_breakable_runs {
                cache.get_grapheme_prefix_widths(seg_text, measure)
            } else {
                None
            };
            (bw, bpw)
        } else {
            (None, None)
        };

        push_segment(
            seg_text,
            w,
            line_end_fit,
            line_end_paint,
            seg_kind,
            seg_start,
            breakable,
            breakable_prefix,
            &mut widths,
            &mut line_end_fit_advances,
            &mut line_end_paint_advances,
            &mut kinds,
            &mut seg_starts,
            &mut breakable_widths_out,
            &mut breakable_prefix_widths_out,
            &mut segments,
            &mut simple_fast_path,
            include_segments,
        );
        prepared_end_by_analysis[mi] = widths.len();
    }

    let prepared_chunks = map_analysis_chunks_to_prepared_chunks(
        &analysis.chunks,
        &prepared_start_by_analysis,
        &prepared_end_by_analysis,
    );

    let seg_levels = if include_segments {
        compute_segment_levels(&analysis.normalized, &seg_starts)
    } else {
        None
    };

    let core = PreparedCore {
        widths,
        line_end_fit_advances,
        line_end_paint_advances,
        kinds,
        simple_line_walk_fast_path: simple_fast_path,
        seg_levels,
        breakable_widths: breakable_widths_out,
        breakable_prefix_widths: breakable_prefix_widths_out,
        discretionary_hyphen_width,
        tab_stop_advance,
        chunks: prepared_chunks,
    };

    let segs = if include_segments {
        Some(segments)
    } else {
        None
    };

    (core, segs)
}

// ---------------------------------------------------------------------------
// Line text materialization
// ---------------------------------------------------------------------------

fn line_has_discretionary_hyphen(
    kinds: &[SegmentBreakKind],
    start_seg: usize,
    start_graph: usize,
    end_seg: usize,
) -> bool {
    end_seg > 0
        && kinds[end_seg - 1] == SegmentBreakKind::SoftHyphen
        && !(start_seg == end_seg && start_graph > 0)
}

fn build_line_text(
    segments: &[String],
    kinds: &[SegmentBreakKind],
    start_seg: usize,
    start_graph: usize,
    end_seg: usize,
    end_graph: usize,
) -> String {
    let mut text = String::new();
    let has_hyphen = line_has_discretionary_hyphen(kinds, start_seg, start_graph, end_seg);

    for i in start_seg..end_seg {
        if kinds[i] == SegmentBreakKind::SoftHyphen || kinds[i] == SegmentBreakKind::HardBreak {
            continue;
        }
        if i == start_seg && start_graph > 0 {
            let graphemes: Vec<&str> = segments[i].graphemes(true).collect();
            for g in graphemes.iter().skip(start_graph) {
                text.push_str(g);
            }
        } else {
            text.push_str(&segments[i]);
        }
    }

    if end_graph > 0 && end_seg < segments.len() {
        if has_hyphen {
            text.push('-');
        }
        let graphemes: Vec<&str> = segments[end_seg].graphemes(true).collect();
        let skip = if start_seg == end_seg { start_graph } else { 0 };
        for g in graphemes.iter().skip(skip).take(end_graph - skip) {
            text.push_str(g);
        }
    } else if has_hyphen {
        text.push('-');
    }

    text
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Prepare text for layout. Segments and measures the text once.
///
/// The returned `PreparedText` can be used with [`layout`] for fast line counting.
pub fn prepare(
    text: &str,
    measure: &dyn TextMeasure,
    profile: &EngineProfile,
    options: &PrepareOptions,
) -> PreparedText {
    let analysis_profile = AnalysisProfile {
        carry_cjk_after_closing_quote: profile.carry_cjk_after_closing_quote,
    };
    let analysis = analyze_text(text, &analysis_profile, options.white_space);
    let (core, _) = measure_analysis(&analysis, measure, profile, false);
    PreparedText { core }
}

/// Prepare text for layout, including segment strings for rich output.
///
/// Use with [`layout_with_lines`], [`layout_next_line`], or [`walk_line_ranges`].
pub fn prepare_with_segments(
    text: &str,
    measure: &dyn TextMeasure,
    profile: &EngineProfile,
    options: &PrepareOptions,
) -> PreparedTextWithSegments {
    let analysis_profile = AnalysisProfile {
        carry_cjk_after_closing_quote: profile.carry_cjk_after_closing_quote,
    };
    let analysis = analyze_text(text, &analysis_profile, options.white_space);
    let (core, segments) = measure_analysis(&analysis, measure, profile, true);
    PreparedTextWithSegments {
        core,
        segments: segments.unwrap_or_default(),
    }
}

/// Fast layout: returns line count and total height.
pub fn layout(
    prepared: &PreparedText,
    max_width: f64,
    line_height: f64,
    profile: &EngineProfile,
) -> LayoutResult {
    let data = to_line_break_data(&prepared.core);
    let line_count = count_prepared_lines(&data, max_width, profile);
    LayoutResult {
        line_count,
        height: line_count as f64 * line_height,
    }
}

/// Layout with full line data (text content, widths, cursor positions).
pub fn layout_with_lines(
    prepared: &PreparedTextWithSegments,
    max_width: f64,
    line_height: f64,
    profile: &EngineProfile,
) -> LayoutLinesResult {
    let data = to_line_break_data(&prepared.core);

    if prepared.core.widths.is_empty() {
        return LayoutLinesResult {
            line_count: 0,
            height: 0.0,
            lines: Vec::new(),
        };
    }

    let mut lines = Vec::new();
    let segments = &prepared.segments;
    let kinds = &prepared.core.kinds;

    let line_count =
        walk_prepared_lines(&data, max_width, profile, Some(&mut |line: &InternalLayoutLine| {
            lines.push(LayoutLine {
                text: build_line_text(
                    segments,
                    kinds,
                    line.start_segment_index,
                    line.start_grapheme_index,
                    line.end_segment_index,
                    line.end_grapheme_index,
                ),
                width: line.width,
                start: LayoutCursor {
                    segment_index: line.start_segment_index,
                    grapheme_index: line.start_grapheme_index,
                },
                end: LayoutCursor {
                    segment_index: line.end_segment_index,
                    grapheme_index: line.end_grapheme_index,
                },
            });
        }));

    LayoutLinesResult {
        line_count,
        height: line_count as f64 * line_height,
        lines,
    }
}

/// Iterate line ranges with a callback (no text materialization).
pub fn walk_line_ranges(
    prepared: &PreparedTextWithSegments,
    max_width: f64,
    profile: &EngineProfile,
    mut on_line: impl FnMut(&LayoutLineRange),
) -> usize {
    let data = to_line_break_data(&prepared.core);

    if prepared.core.widths.is_empty() {
        return 0;
    }

    walk_prepared_lines(
        &data,
        max_width,
        profile,
        Some(&mut |line: &InternalLayoutLine| {
            on_line(&to_layout_line_range(line));
        }),
    )
}

/// Layout a single line starting from a cursor position.
pub fn layout_next_line(
    prepared: &PreparedTextWithSegments,
    start: LayoutCursor,
    max_width: f64,
    profile: &EngineProfile,
) -> Option<LayoutLine> {
    let data = to_line_break_data(&prepared.core);
    let cursor = LineBreakCursor {
        segment_index: start.segment_index,
        grapheme_index: start.grapheme_index,
    };

    let line = layout_next_line_range(&data, cursor, max_width, profile)?;
    let range = to_layout_line_range(&line);

    Some(LayoutLine {
        text: build_line_text(
            &prepared.segments,
            &prepared.core.kinds,
            range.start.segment_index,
            range.start.grapheme_index,
            range.end.segment_index,
            range.end.grapheme_index,
        ),
        width: range.width,
        start: range.start,
        end: range.end,
    })
}

/// Profile the prepare phase (for diagnostics).
pub fn profile_prepare(
    text: &str,
    measure: &dyn TextMeasure,
    profile: &EngineProfile,
    options: &PrepareOptions,
) -> PrepareProfile {
    let analysis_profile = AnalysisProfile {
        carry_cjk_after_closing_quote: profile.carry_cjk_after_closing_quote,
    };
    let analysis = analyze_text(text, &analysis_profile, options.white_space);
    let analysis_segments = analysis.len();
    let (core, _) = measure_analysis(&analysis, measure, profile, false);

    let breakable_segments = core
        .breakable_widths
        .iter()
        .filter(|w| w.is_some())
        .count();

    PrepareProfile {
        analysis_segments,
        prepared_segments: core.widths.len(),
        breakable_segments,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test measure: each character is 10px wide.
    struct TestMeasure;

    impl TextMeasure for TestMeasure {
        fn measure_width(&self, text: &str) -> f64 {
            text.chars().count() as f64 * 10.0
        }
    }

    #[test]
    fn test_prepare_and_layout_empty() {
        let measure = TestMeasure;
        let profile = EngineProfile::default();
        let options = PrepareOptions::default();
        let prepared = prepare("", &measure, &profile, &options);
        let result = layout(&prepared, 200.0, 20.0, &profile);
        assert_eq!(result.line_count, 0);
        assert_eq!(result.height, 0.0);
    }

    #[test]
    fn test_single_word() {
        let measure = TestMeasure;
        let profile = EngineProfile::default();
        let options = PrepareOptions::default();
        let prepared = prepare("hello", &measure, &profile, &options);
        let result = layout(&prepared, 200.0, 20.0, &profile);
        assert_eq!(result.line_count, 1);
        assert_eq!(result.height, 20.0);
    }

    #[test]
    fn test_word_wrap() {
        let measure = TestMeasure;
        let profile = EngineProfile::default();
        let options = PrepareOptions::default();
        // "hello world" = 5*10 + 1*10 + 5*10 = 110px
        let prepared = prepare("hello world", &measure, &profile, &options);
        // Width 80px should force a wrap
        let result = layout(&prepared, 80.0, 20.0, &profile);
        assert_eq!(result.line_count, 2);
        assert_eq!(result.height, 40.0);
    }

    #[test]
    fn test_layout_with_lines() {
        let measure = TestMeasure;
        let profile = EngineProfile::default();
        let options = PrepareOptions::default();
        let prepared = prepare_with_segments("hello world", &measure, &profile, &options);
        let result = layout_with_lines(&prepared, 80.0, 20.0, &profile);
        assert_eq!(result.line_count, 2);
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0].text, "hello ");
        assert_eq!(result.lines[1].text, "world");
    }

    #[test]
    fn test_layout_next_line() {
        let measure = TestMeasure;
        let profile = EngineProfile::default();
        let options = PrepareOptions::default();
        let prepared = prepare_with_segments("hello world foo", &measure, &profile, &options);

        let start = LayoutCursor {
            segment_index: 0,
            grapheme_index: 0,
        };
        let line1 = layout_next_line(&prepared, start, 80.0, &profile);
        assert!(line1.is_some());
        let line1 = line1.unwrap();
        assert_eq!(line1.text, "hello ");

        let line2 = layout_next_line(&prepared, line1.end, 80.0, &profile);
        assert!(line2.is_some());
        let line2 = line2.unwrap();
        assert_eq!(line2.text, "world ");

        let line3 = layout_next_line(&prepared, line2.end, 80.0, &profile);
        assert!(line3.is_some());
        assert_eq!(line3.unwrap().text, "foo");
    }
}
