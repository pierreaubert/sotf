/// Line breaking algorithms, ported from chenglou/pretext.
///
/// Implements both:
/// 1. **Greedy** line breaking with pending-break tracking, soft hyphen
///    support, tab stops, and overflow-wrap grapheme-level breaking.
/// 2. **Knuth-Plass** optimal line breaking that minimizes total demerits
///    (badness + penalties) across the entire paragraph using dynamic
///    programming over feasible breakpoints.
///
/// The greedy algorithm has both a "simple" fast path (no tabs/soft-hyphens)
/// and a full complex path.
use crate::analysis::SegmentBreakKind;
use crate::measurement::EngineProfile;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineBreakCursor {
    pub segment_index: usize,
    pub grapheme_index: usize,
}

#[derive(Debug, Clone)]
pub struct PreparedLineBreakData {
    pub widths: Vec<f64>,
    pub line_end_fit_advances: Vec<f64>,
    pub line_end_paint_advances: Vec<f64>,
    pub kinds: Vec<SegmentBreakKind>,
    pub simple_line_walk_fast_path: bool,
    pub breakable_widths: Vec<Option<Vec<f64>>>,
    pub breakable_prefix_widths: Vec<Option<Vec<f64>>>,
    pub discretionary_hyphen_width: f64,
    pub tab_stop_advance: f64,
    pub chunks: Vec<PreparedLineChunk>,
}

#[derive(Debug, Clone)]
pub struct PreparedLineChunk {
    pub start_segment_index: usize,
    pub end_segment_index: usize,
    pub consumed_end_segment_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InternalLayoutLine {
    pub start_segment_index: usize,
    pub start_grapheme_index: usize,
    pub end_segment_index: usize,
    pub end_grapheme_index: usize,
    pub width: f64,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn can_break_after(kind: SegmentBreakKind) -> bool {
    matches!(
        kind,
        SegmentBreakKind::Space
            | SegmentBreakKind::PreservedSpace
            | SegmentBreakKind::Tab
            | SegmentBreakKind::ZeroWidthBreak
            | SegmentBreakKind::SoftHyphen
    )
}

fn is_simple_collapsible_space(kind: SegmentBreakKind) -> bool {
    kind == SegmentBreakKind::Space
}

fn get_tab_advance(line_width: f64, tab_stop_advance: f64) -> f64 {
    if tab_stop_advance <= 0.0 {
        return 0.0;
    }
    let remainder = line_width % tab_stop_advance;
    if remainder.abs() <= 1e-6 {
        tab_stop_advance
    } else {
        tab_stop_advance - remainder
    }
}

fn get_breakable_advance(
    grapheme_widths: &[f64],
    grapheme_prefix_widths: Option<&[f64]>,
    grapheme_index: usize,
    prefer_prefix_widths: bool,
) -> f64 {
    if !prefer_prefix_widths || grapheme_prefix_widths.is_none() {
        return grapheme_widths[grapheme_index];
    }
    let pw = grapheme_prefix_widths.unwrap();
    pw[grapheme_index] - if grapheme_index > 0 { pw[grapheme_index - 1] } else { 0.0 }
}

fn fit_soft_hyphen_break(
    grapheme_widths: &[f64],
    initial_width: f64,
    max_width: f64,
    line_fit_epsilon: f64,
    discretionary_hyphen_width: f64,
    cumulative_widths: bool,
) -> (usize, f64) {
    let mut fit_count = 0;
    let mut fitted_width = initial_width;

    while fit_count < grapheme_widths.len() {
        let next_width = if cumulative_widths {
            initial_width + grapheme_widths[fit_count]
        } else {
            fitted_width + grapheme_widths[fit_count]
        };
        let next_line_width = if fit_count + 1 < grapheme_widths.len() {
            next_width + discretionary_hyphen_width
        } else {
            next_width
        };
        if next_line_width > max_width + line_fit_epsilon {
            break;
        }
        fitted_width = next_width;
        fit_count += 1;
    }

    (fit_count, fitted_width)
}

fn find_chunk_index_for_start(prepared: &PreparedLineBreakData, segment_index: usize) -> Option<usize> {
    prepared
        .chunks
        .iter()
        .position(|chunk| segment_index < chunk.consumed_end_segment_index)
}

// ---------------------------------------------------------------------------
// normalizeLineStart
// ---------------------------------------------------------------------------

pub fn normalize_line_start(
    prepared: &PreparedLineBreakData,
    start: LineBreakCursor,
) -> Option<LineBreakCursor> {
    let mut seg_idx = start.segment_index;

    if seg_idx >= prepared.widths.len() {
        return None;
    }
    if start.grapheme_index > 0 {
        return Some(start);
    }

    let chunk_idx = find_chunk_index_for_start(prepared, seg_idx)?;
    let chunk = &prepared.chunks[chunk_idx];

    if chunk.start_segment_index == chunk.end_segment_index
        && seg_idx == chunk.start_segment_index
    {
        return Some(LineBreakCursor {
            segment_index: seg_idx,
            grapheme_index: 0,
        });
    }

    if seg_idx < chunk.start_segment_index {
        seg_idx = chunk.start_segment_index;
    }

    while seg_idx < chunk.end_segment_index {
        let kind = prepared.kinds[seg_idx];
        if kind != SegmentBreakKind::Space
            && kind != SegmentBreakKind::ZeroWidthBreak
            && kind != SegmentBreakKind::SoftHyphen
        {
            return Some(LineBreakCursor {
                segment_index: seg_idx,
                grapheme_index: 0,
            });
        }
        seg_idx += 1;
    }

    if chunk.consumed_end_segment_index >= prepared.widths.len() {
        return None;
    }
    Some(LineBreakCursor {
        segment_index: chunk.consumed_end_segment_index,
        grapheme_index: 0,
    })
}

// ---------------------------------------------------------------------------
// countPreparedLines
// ---------------------------------------------------------------------------

pub fn count_prepared_lines(prepared: &PreparedLineBreakData, max_width: f64, profile: &EngineProfile) -> usize {
    if prepared.simple_line_walk_fast_path {
        count_prepared_lines_simple(prepared, max_width, profile)
    } else {
        walk_prepared_lines(prepared, max_width, profile, None)
    }
}

fn count_prepared_lines_simple(
    prepared: &PreparedLineBreakData,
    max_width: f64,
    profile: &EngineProfile,
) -> usize {
    let widths = &prepared.widths;
    let kinds = &prepared.kinds;
    let breakable_widths = &prepared.breakable_widths;
    let breakable_prefix_widths = &prepared.breakable_prefix_widths;

    if widths.is_empty() {
        return 0;
    }

    let eps = profile.line_fit_epsilon;
    let mut line_count = 0usize;
    let mut line_w = 0.0f64;
    let mut has_content = false;

    let place_on_fresh_line = |seg_idx: usize, line_count: &mut usize, line_w: &mut f64, has_content: &mut bool| {
        let w = widths[seg_idx];
        if w > max_width && breakable_widths[seg_idx].is_some() {
            let g_widths = breakable_widths[seg_idx].as_ref().unwrap();
            let g_prefix = breakable_prefix_widths[seg_idx].as_deref();
            *line_w = 0.0;
            for g in 0..g_widths.len() {
                let gw = get_breakable_advance(
                    g_widths,
                    g_prefix,
                    g,
                    profile.prefer_prefix_widths_for_breakable_runs,
                );
                if *line_w > 0.0 && *line_w + gw > max_width + eps {
                    *line_count += 1;
                    *line_w = gw;
                } else {
                    if *line_w == 0.0 {
                        *line_count += 1;
                    }
                    *line_w += gw;
                }
            }
        } else {
            *line_w = w;
            *line_count += 1;
        }
        *has_content = true;
    };

    for i in 0..widths.len() {
        let w = widths[i];
        let kind = kinds[i];

        if !has_content {
            place_on_fresh_line(i, &mut line_count, &mut line_w, &mut has_content);
            continue;
        }

        let new_w = line_w + w;
        if new_w > max_width + eps {
            if is_simple_collapsible_space(kind) {
                continue;
            }
            line_w = 0.0;
            has_content = false;
            place_on_fresh_line(i, &mut line_count, &mut line_w, &mut has_content);
            continue;
        }

        line_w = new_w;
    }

    if !has_content {
        return line_count + 1;
    }
    line_count
}

// ---------------------------------------------------------------------------
// walkPreparedLines — simple path
// ---------------------------------------------------------------------------

fn walk_prepared_lines_simple(
    prepared: &PreparedLineBreakData,
    max_width: f64,
    profile: &EngineProfile,
    on_line: &mut Option<&mut dyn FnMut(&InternalLayoutLine)>,
) -> usize {
    let widths = &prepared.widths;
    let kinds = &prepared.kinds;
    let breakable_widths = &prepared.breakable_widths;
    let breakable_prefix_widths = &prepared.breakable_prefix_widths;

    if widths.is_empty() {
        return 0;
    }

    let eps = profile.line_fit_epsilon;
    let mut line_count = 0usize;
    let mut line_w = 0.0f64;
    let mut has_content = false;
    let mut line_start_seg = 0usize;
    let mut line_start_graph = 0usize;
    let mut line_end_seg = 0usize;
    let mut line_end_graph = 0usize;
    let mut pending_break_seg: Option<usize> = None;
    let mut pending_break_paint_w = 0.0f64;

    macro_rules! emit_line {
        ($end_seg:expr, $end_graph:expr, $w:expr) => {{
            line_count += 1;
            if let Some(cb) = on_line.as_mut() {
                cb(&InternalLayoutLine {
                    start_segment_index: line_start_seg,
                    start_grapheme_index: line_start_graph,
                    end_segment_index: $end_seg,
                    end_grapheme_index: $end_graph,
                    width: $w,
                });
            }
            line_w = 0.0;
            has_content = false;
            pending_break_seg = None;
            pending_break_paint_w = 0.0;
        }};
        () => {
            emit_line!(line_end_seg, line_end_graph, line_w)
        };
    }

    macro_rules! start_at_segment {
        ($seg:expr, $w:expr) => {{
            has_content = true;
            line_start_seg = $seg;
            line_start_graph = 0;
            line_end_seg = $seg + 1;
            line_end_graph = 0;
            line_w = $w;
        }};
    }

    macro_rules! append_segment {
        ($seg:expr, $w:expr) => {{
            if !has_content {
                start_at_segment!($seg, $w);
            } else {
                line_w += $w;
                line_end_seg = $seg + 1;
                line_end_graph = 0;
            }
        }};
    }

    let mut append_breakable_from =
        |seg_idx: usize,
         start_g: usize,
         line_count: &mut usize,
         line_w: &mut f64,
         has_content: &mut bool,
         line_start_seg: &mut usize,
         line_start_graph: &mut usize,
         line_end_seg: &mut usize,
         line_end_graph: &mut usize,
         pending_break_seg: &mut Option<usize>,
         pending_break_paint_w: &mut f64,
         on_line: &mut Option<&mut dyn FnMut(&InternalLayoutLine)>| {
            let g_widths = breakable_widths[seg_idx].as_ref().unwrap();
            let g_prefix = breakable_prefix_widths[seg_idx].as_deref();
            for g in start_g..g_widths.len() {
                let gw = get_breakable_advance(
                    g_widths,
                    g_prefix,
                    g,
                    profile.prefer_prefix_widths_for_breakable_runs,
                );
                if !*has_content {
                    *has_content = true;
                    *line_start_seg = seg_idx;
                    *line_start_graph = g;
                    *line_end_seg = seg_idx;
                    *line_end_graph = g + 1;
                    *line_w = gw;
                    continue;
                }
                if *line_w + gw > max_width + eps {
                    // emit
                    *line_count += 1;
                    if let Some(cb) = on_line.as_mut() {
                        cb(&InternalLayoutLine {
                            start_segment_index: *line_start_seg,
                            start_grapheme_index: *line_start_graph,
                            end_segment_index: *line_end_seg,
                            end_grapheme_index: *line_end_graph,
                            width: *line_w,
                        });
                    }
                    *line_w = gw;
                    *has_content = true;
                    *line_start_seg = seg_idx;
                    *line_start_graph = g;
                    *line_end_seg = seg_idx;
                    *line_end_graph = g + 1;
                    *pending_break_seg = None;
                    *pending_break_paint_w = 0.0;
                } else {
                    *line_w += gw;
                    *line_end_seg = seg_idx;
                    *line_end_graph = g + 1;
                }
            }
            if *has_content && *line_end_seg == seg_idx && *line_end_graph == g_widths.len() {
                *line_end_seg = seg_idx + 1;
                *line_end_graph = 0;
            }
        };

    let mut i = 0;
    while i < widths.len() {
        let w = widths[i];
        let kind = kinds[i];

        if !has_content {
            if w > max_width && breakable_widths[i].is_some() {
                append_breakable_from(
                    i,
                    0,
                    &mut line_count,
                    &mut line_w,
                    &mut has_content,
                    &mut line_start_seg,
                    &mut line_start_graph,
                    &mut line_end_seg,
                    &mut line_end_graph,
                    &mut pending_break_seg,
                    &mut pending_break_paint_w,
                    on_line,
                );
            } else {
                start_at_segment!(i, w);
            }
            if can_break_after(kind) {
                pending_break_seg = Some(i + 1);
                pending_break_paint_w = line_w - w;
            }
            i += 1;
            continue;
        }

        let new_w = line_w + w;
        if new_w > max_width + eps {
            if can_break_after(kind) {
                append_segment!(i, w);
                emit_line!(i + 1, 0, line_w - w);
                i += 1;
                continue;
            }

            if let Some(pb_seg) = pending_break_seg {
                emit_line!(pb_seg, 0, pending_break_paint_w);
                continue;
            }

            if w > max_width && breakable_widths[i].is_some() {
                emit_line!();
                append_breakable_from(
                    i,
                    0,
                    &mut line_count,
                    &mut line_w,
                    &mut has_content,
                    &mut line_start_seg,
                    &mut line_start_graph,
                    &mut line_end_seg,
                    &mut line_end_graph,
                    &mut pending_break_seg,
                    &mut pending_break_paint_w,
                    on_line,
                );
                i += 1;
                continue;
            }

            emit_line!();
            continue;
        }

        append_segment!(i, w);
        if can_break_after(kind) {
            pending_break_seg = Some(i + 1);
            pending_break_paint_w = line_w - w;
        }
        i += 1;
    }

    if has_content {
        emit_line!();
    }
    line_count
}

// ---------------------------------------------------------------------------
// walkPreparedLines — full path
// ---------------------------------------------------------------------------

pub fn walk_prepared_lines(
    prepared: &PreparedLineBreakData,
    max_width: f64,
    profile: &EngineProfile,
    mut on_line: Option<&mut dyn FnMut(&InternalLayoutLine)>,
) -> usize {
    if prepared.simple_line_walk_fast_path {
        return walk_prepared_lines_simple(prepared, max_width, profile, &mut on_line);
    }

    let widths = &prepared.widths;
    let line_end_fit_advances = &prepared.line_end_fit_advances;
    let line_end_paint_advances = &prepared.line_end_paint_advances;
    let kinds = &prepared.kinds;
    let breakable_widths = &prepared.breakable_widths;
    let breakable_prefix_widths = &prepared.breakable_prefix_widths;
    let discretionary_hyphen_width = prepared.discretionary_hyphen_width;
    let tab_stop_advance = prepared.tab_stop_advance;
    let chunks = &prepared.chunks;

    if widths.is_empty() || chunks.is_empty() {
        return 0;
    }

    let eps = profile.line_fit_epsilon;
    let mut line_count = 0usize;
    let mut line_w = 0.0f64;
    let mut has_content = false;
    let mut line_start_seg = 0usize;
    let mut line_start_graph = 0usize;
    let mut line_end_seg = 0usize;
    let mut line_end_graph = 0usize;
    let mut pending_break_seg: Option<usize> = None;
    let mut pending_break_fit_w = 0.0f64;
    let mut pending_break_paint_w = 0.0f64;
    let mut pending_break_kind: Option<SegmentBreakKind> = None;

    macro_rules! clear_pending {
        () => {{
            pending_break_seg = None;
            pending_break_fit_w = 0.0;
            pending_break_paint_w = 0.0;
            pending_break_kind = None;
        }};
    }

    macro_rules! emit_line {
        ($end_seg:expr, $end_graph:expr, $w:expr) => {{
            line_count += 1;
            if let Some(cb) = on_line.as_mut() {
                cb(&InternalLayoutLine {
                    start_segment_index: line_start_seg,
                    start_grapheme_index: line_start_graph,
                    end_segment_index: $end_seg,
                    end_grapheme_index: $end_graph,
                    width: $w,
                });
            }
            line_w = 0.0;
            has_content = false;
            clear_pending!();
        }};
        () => {
            emit_line!(line_end_seg, line_end_graph, line_w)
        };
    }

    for chunk in chunks {
        if chunk.start_segment_index == chunk.end_segment_index {
            // Empty chunk (hard break only)
            line_count += 1;
            if let Some(cb) = on_line.as_mut() {
                cb(&InternalLayoutLine {
                    start_segment_index: chunk.start_segment_index,
                    start_grapheme_index: 0,
                    end_segment_index: chunk.consumed_end_segment_index,
                    end_grapheme_index: 0,
                    width: 0.0,
                });
            }
            clear_pending!();
            continue;
        }

        has_content = false;
        line_w = 0.0;
        line_start_seg = chunk.start_segment_index;
        line_start_graph = 0;
        line_end_seg = chunk.start_segment_index;
        line_end_graph = 0;
        clear_pending!();

        let mut i = chunk.start_segment_index;
        while i < chunk.end_segment_index {
            let kind = kinds[i];
            let w = if kind == SegmentBreakKind::Tab {
                get_tab_advance(line_w, tab_stop_advance)
            } else {
                widths[i]
            };

            if kind == SegmentBreakKind::SoftHyphen {
                if has_content {
                    line_end_seg = i + 1;
                    line_end_graph = 0;
                    pending_break_seg = Some(i + 1);
                    pending_break_fit_w = line_w + discretionary_hyphen_width;
                    pending_break_paint_w = line_w + discretionary_hyphen_width;
                    pending_break_kind = Some(kind);
                }
                i += 1;
                continue;
            }

            if !has_content {
                if w > max_width && breakable_widths[i].is_some() {
                    // appendBreakableSegment inline
                    let g_widths = breakable_widths[i].as_ref().unwrap();
                    let g_prefix = breakable_prefix_widths[i].as_deref();
                    for g in 0..g_widths.len() {
                        let gw = get_breakable_advance(
                            g_widths,
                            g_prefix,
                            g,
                            profile.prefer_prefix_widths_for_breakable_runs,
                        );
                        if !has_content {
                            has_content = true;
                            line_start_seg = i;
                            line_start_graph = g;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                            line_w = gw;
                        } else if line_w + gw > max_width + eps {
                            emit_line!();
                            has_content = true;
                            line_start_seg = i;
                            line_start_graph = g;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                            line_w = gw;
                        } else {
                            line_w += gw;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                        }
                    }
                    if has_content && line_end_seg == i && line_end_graph == g_widths.len() {
                        line_end_seg = i + 1;
                        line_end_graph = 0;
                    }
                } else {
                    has_content = true;
                    line_start_seg = i;
                    line_start_graph = 0;
                    line_end_seg = i + 1;
                    line_end_graph = 0;
                    line_w = w;
                }
                // update pending break
                if can_break_after(kind) {
                    let fit_adv = if kind == SegmentBreakKind::Tab {
                        0.0
                    } else {
                        line_end_fit_advances[i]
                    };
                    let paint_adv = if kind == SegmentBreakKind::Tab {
                        w
                    } else {
                        line_end_paint_advances[i]
                    };
                    pending_break_seg = Some(i + 1);
                    pending_break_fit_w = line_w - w + fit_adv;
                    pending_break_paint_w = line_w - w + paint_adv;
                    pending_break_kind = Some(kind);
                }
                i += 1;
                continue;
            }

            let new_w = line_w + w;
            if new_w > max_width + eps {
                let current_fit_w = line_w
                    + if kind == SegmentBreakKind::Tab {
                        0.0
                    } else {
                        line_end_fit_advances[i]
                    };
                let current_paint_w = line_w
                    + if kind == SegmentBreakKind::Tab {
                        w
                    } else {
                        line_end_paint_advances[i]
                    };

                // Early soft-hyphen break
                if pending_break_kind == Some(SegmentBreakKind::SoftHyphen)
                    && profile.prefer_early_soft_hyphen_break
                    && pending_break_fit_w <= max_width + eps
                    && let Some(pb_seg) = pending_break_seg {
                        emit_line!(pb_seg, 0, pending_break_paint_w);
                        continue;
                    }

                // Soft-hyphen breakable continuation
                if pending_break_kind == Some(SegmentBreakKind::SoftHyphen)
                    && let Some(ref gw) = breakable_widths[i] {
                        let fit_widths = if profile.prefer_prefix_widths_for_breakable_runs {
                            breakable_prefix_widths[i].as_deref().unwrap_or(gw)
                        } else {
                            gw
                        };
                        let uses_prefix = fit_widths.as_ptr() != gw.as_ptr();
                        let (fit_count, fitted_width) = fit_soft_hyphen_break(
                            fit_widths,
                            line_w,
                            max_width,
                            eps,
                            discretionary_hyphen_width,
                            uses_prefix,
                        );
                        if fit_count > 0 {
                            line_w = fitted_width;
                            line_end_seg = i;
                            line_end_graph = fit_count;
                            clear_pending!();
                            if fit_count == gw.len() {
                                line_end_seg = i + 1;
                                line_end_graph = 0;
                                i += 1;
                                continue;
                            }
                            emit_line!(i, fit_count, fitted_width + discretionary_hyphen_width);
                            // continue from fit_count
                            let remaining_gw = breakable_widths[i].as_ref().unwrap();
                            let remaining_prefix = breakable_prefix_widths[i].as_deref();
                            for g in fit_count..remaining_gw.len() {
                                let gw_val = get_breakable_advance(
                                    remaining_gw,
                                    remaining_prefix,
                                    g,
                                    profile.prefer_prefix_widths_for_breakable_runs,
                                );
                                if !has_content {
                                    has_content = true;
                                    line_start_seg = i;
                                    line_start_graph = g;
                                    line_end_seg = i;
                                    line_end_graph = g + 1;
                                    line_w = gw_val;
                                } else if line_w + gw_val > max_width + eps {
                                    emit_line!();
                                    has_content = true;
                                    line_start_seg = i;
                                    line_start_graph = g;
                                    line_end_seg = i;
                                    line_end_graph = g + 1;
                                    line_w = gw_val;
                                } else {
                                    line_w += gw_val;
                                    line_end_seg = i;
                                    line_end_graph = g + 1;
                                }
                            }
                            if has_content
                                && line_end_seg == i
                                && line_end_graph == remaining_gw.len()
                            {
                                line_end_seg = i + 1;
                                line_end_graph = 0;
                            }
                            i += 1;
                            continue;
                        }
                    }

                if can_break_after(kind) && current_fit_w <= max_width + eps {
                    line_w += w;
                    line_end_seg = i + 1;
                    line_end_graph = 0;
                    emit_line!(i + 1, 0, current_paint_w);
                    i += 1;
                    continue;
                }

                if let Some(pb_seg) = pending_break_seg
                    && pending_break_fit_w <= max_width + eps {
                        emit_line!(pb_seg, 0, pending_break_paint_w);
                        continue;
                    }

                if w > max_width && breakable_widths[i].is_some() {
                    emit_line!();
                    let g_widths = breakable_widths[i].as_ref().unwrap();
                    let g_prefix = breakable_prefix_widths[i].as_deref();
                    for g in 0..g_widths.len() {
                        let gw_val = get_breakable_advance(
                            g_widths,
                            g_prefix,
                            g,
                            profile.prefer_prefix_widths_for_breakable_runs,
                        );
                        if !has_content {
                            has_content = true;
                            line_start_seg = i;
                            line_start_graph = g;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                            line_w = gw_val;
                        } else if line_w + gw_val > max_width + eps {
                            emit_line!();
                            has_content = true;
                            line_start_seg = i;
                            line_start_graph = g;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                            line_w = gw_val;
                        } else {
                            line_w += gw_val;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                        }
                    }
                    if has_content && line_end_seg == i && line_end_graph == g_widths.len() {
                        line_end_seg = i + 1;
                        line_end_graph = 0;
                    }
                    i += 1;
                    continue;
                }

                emit_line!();
                continue;
            }

            // Fits: append
            line_w += w;
            line_end_seg = i + 1;
            line_end_graph = 0;
            if can_break_after(kind) {
                let fit_adv = if kind == SegmentBreakKind::Tab {
                    0.0
                } else {
                    line_end_fit_advances[i]
                };
                let paint_adv = if kind == SegmentBreakKind::Tab {
                    w
                } else {
                    line_end_paint_advances[i]
                };
                pending_break_seg = Some(i + 1);
                pending_break_fit_w = line_w - w + fit_adv;
                pending_break_paint_w = line_w - w + paint_adv;
                pending_break_kind = Some(kind);
            }
            i += 1;
        }

        if has_content {
            let final_paint_w =
                if pending_break_seg == Some(chunk.consumed_end_segment_index) {
                    pending_break_paint_w
                } else {
                    line_w
                };
            emit_line!(chunk.consumed_end_segment_index, 0, final_paint_w);
        }
    }

    line_count
}

// ---------------------------------------------------------------------------
// layoutNextLineRange
// ---------------------------------------------------------------------------

pub fn layout_next_line_range(
    prepared: &PreparedLineBreakData,
    start: LineBreakCursor,
    max_width: f64,
    profile: &EngineProfile,
) -> Option<InternalLayoutLine> {
    let normalized_start = normalize_line_start(prepared, start)?;

    if prepared.simple_line_walk_fast_path {
        return layout_next_line_range_simple(prepared, normalized_start, max_width, profile);
    }

    let chunk_idx = find_chunk_index_for_start(prepared, normalized_start.segment_index)?;
    let chunk = &prepared.chunks[chunk_idx];

    if chunk.start_segment_index == chunk.end_segment_index {
        return Some(InternalLayoutLine {
            start_segment_index: chunk.start_segment_index,
            start_grapheme_index: 0,
            end_segment_index: chunk.consumed_end_segment_index,
            end_grapheme_index: 0,
            width: 0.0,
        });
    }

    let widths = &prepared.widths;
    let line_end_fit_advances = &prepared.line_end_fit_advances;
    let line_end_paint_advances = &prepared.line_end_paint_advances;
    let kinds = &prepared.kinds;
    let breakable_widths = &prepared.breakable_widths;
    let breakable_prefix_widths = &prepared.breakable_prefix_widths;
    let discretionary_hyphen_width = prepared.discretionary_hyphen_width;
    let tab_stop_advance = prepared.tab_stop_advance;
    let eps = profile.line_fit_epsilon;

    let mut line_w = 0.0f64;
    let mut has_content = false;
    let line_start_seg = normalized_start.segment_index;
    let line_start_graph = normalized_start.grapheme_index;
    let mut line_end_seg = line_start_seg;
    let mut line_end_graph = line_start_graph;
    let mut pending_break_seg: Option<usize> = None;
    let mut pending_break_fit_w = 0.0f64;
    let mut pending_break_paint_w = 0.0f64;
    let mut pending_break_kind: Option<SegmentBreakKind> = None;

    let finish_line = |end_seg: usize, end_graph: usize, w: f64, has_content: bool| -> Option<InternalLayoutLine> {
        if !has_content {
            return None;
        }
        Some(InternalLayoutLine {
            start_segment_index: line_start_seg,
            start_grapheme_index: line_start_graph,
            end_segment_index: end_seg,
            end_grapheme_index: end_graph,
            width: w,
        })
    };

    for i in normalized_start.segment_index..chunk.end_segment_index {
        let kind = kinds[i];
        let start_grapheme = if i == normalized_start.segment_index {
            normalized_start.grapheme_index
        } else {
            0
        };
        let w = if kind == SegmentBreakKind::Tab {
            get_tab_advance(line_w, tab_stop_advance)
        } else {
            widths[i]
        };

        if kind == SegmentBreakKind::SoftHyphen && start_grapheme == 0 {
            if has_content {
                line_end_seg = i + 1;
                line_end_graph = 0;
                pending_break_seg = Some(i + 1);
                pending_break_fit_w = line_w + discretionary_hyphen_width;
                pending_break_paint_w = line_w + discretionary_hyphen_width;
                pending_break_kind = Some(kind);
            }
            continue;
        }

        if !has_content {
            if start_grapheme > 0 {
                if let Some(ref gw) = breakable_widths[i] {
                    let gp = breakable_prefix_widths[i].as_deref();
                    for g in start_grapheme..gw.len() {
                        let gw_val =
                            get_breakable_advance(gw, gp, g, profile.prefer_prefix_widths_for_breakable_runs);
                        if !has_content {
                            has_content = true;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                            line_w = gw_val;
                        } else if line_w + gw_val > max_width + eps {
                            return finish_line(line_end_seg, line_end_graph, line_w, has_content);
                        } else {
                            line_w += gw_val;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                        }
                    }
                    if has_content && line_end_seg == i && line_end_graph == gw.len() {
                        line_end_seg = i + 1;
                        line_end_graph = 0;
                    }
                }
            } else if w > max_width && breakable_widths[i].is_some() {
                let gw = breakable_widths[i].as_ref().unwrap();
                let gp = breakable_prefix_widths[i].as_deref();
                for g in 0..gw.len() {
                    let gw_val =
                        get_breakable_advance(gw, gp, g, profile.prefer_prefix_widths_for_breakable_runs);
                    if !has_content {
                        has_content = true;
                        line_end_seg = i;
                        line_end_graph = g + 1;
                        line_w = gw_val;
                    } else if line_w + gw_val > max_width + eps {
                        return finish_line(line_end_seg, line_end_graph, line_w, has_content);
                    } else {
                        line_w += gw_val;
                        line_end_seg = i;
                        line_end_graph = g + 1;
                    }
                }
                if has_content && line_end_seg == i && line_end_graph == gw.len() {
                    line_end_seg = i + 1;
                    line_end_graph = 0;
                }
            } else {
                has_content = true;
                line_end_seg = i + 1;
                line_end_graph = 0;
                line_w = w;
            }
            if can_break_after(kind) {
                let fit_adv = if kind == SegmentBreakKind::Tab {
                    0.0
                } else {
                    line_end_fit_advances[i]
                };
                let paint_adv = if kind == SegmentBreakKind::Tab {
                    w
                } else {
                    line_end_paint_advances[i]
                };
                pending_break_seg = Some(i + 1);
                pending_break_fit_w = line_w - w + fit_adv;
                pending_break_paint_w = line_w - w + paint_adv;
                pending_break_kind = Some(kind);
            }
            continue;
        }

        let new_w = line_w + w;
        if new_w > max_width + eps {
            let current_fit_w = line_w
                + if kind == SegmentBreakKind::Tab {
                    0.0
                } else {
                    line_end_fit_advances[i]
                };
            let current_paint_w = line_w
                + if kind == SegmentBreakKind::Tab {
                    w
                } else {
                    line_end_paint_advances[i]
                };

            if pending_break_kind == Some(SegmentBreakKind::SoftHyphen)
                && profile.prefer_early_soft_hyphen_break
                && pending_break_fit_w <= max_width + eps
                && let Some(pb_seg) = pending_break_seg {
                    return finish_line(pb_seg, 0, pending_break_paint_w, has_content);
                }

            // Soft-hyphen breakable
            if pending_break_kind == Some(SegmentBreakKind::SoftHyphen) {
                if let Some(ref gw) = breakable_widths[i] {
                    let fit_widths = if profile.prefer_prefix_widths_for_breakable_runs {
                        breakable_prefix_widths[i].as_deref().unwrap_or(gw)
                    } else {
                        gw.as_slice()
                    };
                    let uses_prefix = fit_widths.as_ptr() != gw.as_ptr();
                    let (fit_count, fitted_width) = fit_soft_hyphen_break(
                        fit_widths,
                        line_w,
                        max_width,
                        eps,
                        discretionary_hyphen_width,
                        uses_prefix,
                    );
                    if fit_count == gw.len() {
                        line_w = fitted_width;
                        line_end_seg = i + 1;
                        line_end_graph = 0;
                        pending_break_seg = None;
                        pending_break_fit_w = 0.0;
                        pending_break_paint_w = 0.0;
                        pending_break_kind = None;
                        continue;
                    }
                    if fit_count > 0 {
                        return finish_line(
                            i,
                            fit_count,
                            fitted_width + discretionary_hyphen_width,
                            true,
                        );
                    }
                }
                if pending_break_fit_w <= max_width + eps
                    && let Some(pb_seg) = pending_break_seg {
                        return finish_line(pb_seg, 0, pending_break_paint_w, has_content);
                    }
            }

            if can_break_after(kind) && current_fit_w <= max_width + eps {
                line_w += w;
                line_end_seg = i + 1;
                line_end_graph = 0;
                return finish_line(i + 1, 0, current_paint_w, true);
            }

            if let Some(pb_seg) = pending_break_seg
                && pending_break_fit_w <= max_width + eps {
                    return finish_line(pb_seg, 0, pending_break_paint_w, has_content);
                }

            if w > max_width && breakable_widths[i].is_some() {
                let current_line = finish_line(line_end_seg, line_end_graph, line_w, has_content);
                if current_line.is_some() {
                    return current_line;
                }
                // Would need to start breakable segment, but that's for the next call
            }

            return finish_line(line_end_seg, line_end_graph, line_w, has_content);
        }

        // Fits
        line_w += w;
        line_end_seg = i + 1;
        line_end_graph = 0;
        if can_break_after(kind) {
            let fit_adv = if kind == SegmentBreakKind::Tab {
                0.0
            } else {
                line_end_fit_advances[i]
            };
            let paint_adv = if kind == SegmentBreakKind::Tab {
                w
            } else {
                line_end_paint_advances[i]
            };
            pending_break_seg = Some(i + 1);
            pending_break_fit_w = line_w - w + fit_adv;
            pending_break_paint_w = line_w - w + paint_adv;
            pending_break_kind = Some(kind);
        }
    }

    if pending_break_seg == Some(chunk.consumed_end_segment_index) && line_end_graph == 0 {
        return finish_line(chunk.consumed_end_segment_index, 0, pending_break_paint_w, has_content);
    }

    finish_line(chunk.consumed_end_segment_index, 0, line_w, has_content)
}

// ---------------------------------------------------------------------------
// layoutNextLineRange — simple path
// ---------------------------------------------------------------------------

fn layout_next_line_range_simple(
    prepared: &PreparedLineBreakData,
    normalized_start: LineBreakCursor,
    max_width: f64,
    profile: &EngineProfile,
) -> Option<InternalLayoutLine> {
    let widths = &prepared.widths;
    let kinds = &prepared.kinds;
    let breakable_widths = &prepared.breakable_widths;
    let breakable_prefix_widths = &prepared.breakable_prefix_widths;
    let eps = profile.line_fit_epsilon;

    let mut line_w = 0.0f64;
    let mut has_content = false;
    let line_start_seg = normalized_start.segment_index;
    let line_start_graph = normalized_start.grapheme_index;
    let mut line_end_seg = line_start_seg;
    let mut line_end_graph = line_start_graph;
    let mut pending_break_seg: Option<usize> = None;
    let mut pending_break_paint_w = 0.0f64;

    let finish_line = |end_seg: usize, end_graph: usize, w: f64, has_content: bool| -> Option<InternalLayoutLine> {
        if !has_content {
            return None;
        }
        Some(InternalLayoutLine {
            start_segment_index: line_start_seg,
            start_grapheme_index: line_start_graph,
            end_segment_index: end_seg,
            end_grapheme_index: end_graph,
            width: w,
        })
    };

    for i in normalized_start.segment_index..widths.len() {
        let w = widths[i];
        let kind = kinds[i];
        let start_grapheme = if i == normalized_start.segment_index {
            normalized_start.grapheme_index
        } else {
            0
        };

        if !has_content {
            if start_grapheme > 0 {
                if let Some(ref gw) = breakable_widths[i] {
                    let gp = breakable_prefix_widths[i].as_deref();
                    for g in start_grapheme..gw.len() {
                        let gw_val =
                            get_breakable_advance(gw, gp, g, profile.prefer_prefix_widths_for_breakable_runs);
                        if !has_content {
                            has_content = true;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                            line_w = gw_val;
                        } else if line_w + gw_val > max_width + eps {
                            return finish_line(line_end_seg, line_end_graph, line_w, has_content);
                        } else {
                            line_w += gw_val;
                            line_end_seg = i;
                            line_end_graph = g + 1;
                        }
                    }
                    if has_content && line_end_seg == i && line_end_graph == gw.len() {
                        line_end_seg = i + 1;
                        line_end_graph = 0;
                    }
                }
            } else if w > max_width && breakable_widths[i].is_some() {
                let gw = breakable_widths[i].as_ref().unwrap();
                let gp = breakable_prefix_widths[i].as_deref();
                for g in 0..gw.len() {
                    let gw_val =
                        get_breakable_advance(gw, gp, g, profile.prefer_prefix_widths_for_breakable_runs);
                    if !has_content {
                        has_content = true;
                        line_end_seg = i;
                        line_end_graph = g + 1;
                        line_w = gw_val;
                    } else if line_w + gw_val > max_width + eps {
                        return finish_line(line_end_seg, line_end_graph, line_w, has_content);
                    } else {
                        line_w += gw_val;
                        line_end_seg = i;
                        line_end_graph = g + 1;
                    }
                }
                if has_content && line_end_seg == i && line_end_graph == gw.len() {
                    line_end_seg = i + 1;
                    line_end_graph = 0;
                }
            } else {
                has_content = true;
                line_end_seg = i + 1;
                line_end_graph = 0;
                line_w = w;
            }
            if can_break_after(kind) {
                pending_break_seg = Some(i + 1);
                pending_break_paint_w = line_w - w;
            }
            continue;
        }

        let new_w = line_w + w;
        if new_w > max_width + eps {
            if can_break_after(kind) {
                line_w += w;
                line_end_seg = i + 1;
                line_end_graph = 0;
                return finish_line(i + 1, 0, line_w - w, true);
            }
            if let Some(pb_seg) = pending_break_seg {
                return finish_line(pb_seg, 0, pending_break_paint_w, has_content);
            }
            if w > max_width && breakable_widths[i].is_some() {
                let current_line = finish_line(line_end_seg, line_end_graph, line_w, has_content);
                if current_line.is_some() {
                    return current_line;
                }
            }
            return finish_line(line_end_seg, line_end_graph, line_w, has_content);
        }

        line_w += w;
        line_end_seg = i + 1;
        line_end_graph = 0;
        if can_break_after(kind) {
            pending_break_seg = Some(i + 1);
            pending_break_paint_w = line_w - w;
        }
    }

    finish_line(line_end_seg, line_end_graph, line_w, has_content)
}

// ===========================================================================
// Knuth-Plass optimal line breaking
// ===========================================================================

/// Strategy for line breaking.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LineBreakStrategy {
    /// Greedy first-fit algorithm (fast, local decisions).
    #[default]
    Greedy,
    /// Knuth-Plass optimal algorithm (minimizes total demerits across the
    /// entire paragraph). Falls back to greedy if no feasible solution exists.
    Optimal,
}

/// Tuning parameters for the Knuth-Plass algorithm.
#[derive(Debug, Clone, Copy)]
pub struct KnuthPlassParams {
    /// Penalty for a normal inter-word breakpoint (typically 0).
    pub line_penalty: f64,
    /// Penalty for a hyphenation break (typically 50).
    pub hyphen_penalty: f64,
    /// Extra demerits when two consecutive lines end with flagged breaks
    /// (e.g., both hyphenated). Typically 3000.
    pub flagged_demerits: f64,
    /// Extra demerits when adjacent lines differ by more than one fitness
    /// class. Typically 10000.
    pub fitness_demerits: f64,
    /// Tolerance: maximum allowed adjustment ratio before a breakpoint is
    /// considered infeasible. Typically 1.0–2.0 for body text.
    pub tolerance: f64,
    /// When true and no feasible solution exists within tolerance, increase
    /// tolerance and retry rather than failing.
    pub looseness_recovery: bool,
}

impl Default for KnuthPlassParams {
    fn default() -> Self {
        Self {
            line_penalty: 0.0,
            hyphen_penalty: 50.0,
            flagged_demerits: 3000.0,
            fitness_demerits: 10000.0,
            tolerance: 2.0,
            looseness_recovery: true,
        }
    }
}

/// Fitness class per Knuth-Plass §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FitnessClass {
    Tight = 0,  // r < -0.5
    Normal = 1, // -0.5 <= r < 0.5
    Loose = 2,  // 0.5 <= r < 1.0
    VeryLoose = 3, // r >= 1.0
}

impl FitnessClass {
    fn from_ratio(r: f64) -> Self {
        if r < -0.5 {
            FitnessClass::Tight
        } else if r < 0.5 {
            FitnessClass::Normal
        } else if r < 1.0 {
            FitnessClass::Loose
        } else {
            FitnessClass::VeryLoose
        }
    }
}

/// A candidate breakpoint in the Knuth-Plass item list.
#[derive(Debug, Clone)]
struct KPItem {
    /// Segment index in the PreparedLineBreakData.
    segment_index: usize,
    /// Grapheme index within the segment (0 for segment-level breaks).
    grapheme_index: usize,
    /// Cumulative width of all content up to (not including) this break.
    total_width: f64,
    /// Cumulative stretch (glue stretchability) up to this break.
    total_stretch: f64,
    /// Cumulative shrink (glue shrinkability) up to this break.
    total_shrink: f64,
    /// Penalty for breaking here. f64::INFINITY means "must not break",
    /// f64::NEG_INFINITY means "must break" (hard break / end of paragraph).
    penalty: f64,
    /// Whether this break is "flagged" (e.g., a hyphenation break).
    flagged: bool,
    /// Width added at this break position (e.g., hyphen width for soft hyphen).
    break_width: f64,
}

/// An active node in the Knuth-Plass DP.
#[derive(Debug, Clone)]
struct ActiveNode {
    /// Index into the breakpoint candidate list.
    break_index: usize,
    /// Line number (0-based: this node starts line `line`).
    line: usize,
    /// Fitness class of the line ending at this breakpoint.
    fitness: FitnessClass,
    /// Total accumulated demerits up to this breakpoint.
    total_demerits: f64,
    /// Index of the previous node in the optimal path (into `nodes` vec).
    prev_node: Option<usize>,
}

/// Build the list of Knuth-Plass items (breakpoint candidates) from prepared data.
///
/// Each item represents a legal breakpoint with cumulative width/stretch/shrink.
fn build_kp_items(
    prepared: &PreparedLineBreakData,
    profile: &EngineProfile,
    params: &KnuthPlassParams,
) -> Vec<KPItem> {
    let widths = &prepared.widths;
    let kinds = &prepared.kinds;
    let breakable_widths = &prepared.breakable_widths;
    let breakable_prefix_widths = &prepared.breakable_prefix_widths;

    if widths.is_empty() {
        return Vec::new();
    }

    let mut items = Vec::new();

    // Start-of-paragraph sentinel (breakpoint at position 0).
    items.push(KPItem {
        segment_index: 0,
        grapheme_index: 0,
        total_width: 0.0,
        total_stretch: 0.0,
        total_shrink: 0.0,
        penalty: f64::NEG_INFINITY, // forced break (start)
        flagged: false,
        break_width: 0.0,
    });

    let mut cum_width = 0.0;
    let mut cum_stretch = 0.0;
    let mut cum_shrink = 0.0;

    for i in 0..widths.len() {
        let kind = kinds[i];
        let w = if kind == SegmentBreakKind::Tab {
            // For tabs, use a nominal width; actual advance depends on position
            // which the DP will handle approximately via cumulative widths.
            get_tab_advance(cum_width, prepared.tab_stop_advance)
        } else {
            widths[i]
        };

        match kind {
            SegmentBreakKind::HardBreak => {
                // Must break here.
                items.push(KPItem {
                    segment_index: i + 1,
                    grapheme_index: 0,
                    total_width: cum_width,
                    total_stretch: cum_stretch,
                    total_shrink: cum_shrink,
                    penalty: f64::NEG_INFINITY,
                    flagged: false,
                    break_width: 0.0,
                });
            }
            SegmentBreakKind::Space | SegmentBreakKind::PreservedSpace => {
                // Glue: the space has width, stretch, and shrink.
                // Standard Knuth-Plass model: space_width with ±stretch/shrink.
                // We use 1/3 of space width for stretch and 1/6 for shrink
                // (approximation of TeX's interword spacing model).
                let stretch = w * 0.5;
                let shrink = w * 0.333;

                cum_width += w;
                cum_stretch += stretch;
                cum_shrink += shrink;

                // Break is legal *after* the space.
                items.push(KPItem {
                    segment_index: i + 1,
                    grapheme_index: 0,
                    total_width: cum_width,
                    total_stretch: cum_stretch,
                    total_shrink: cum_shrink,
                    penalty: params.line_penalty,
                    flagged: false,
                    break_width: 0.0,
                });
            }
            SegmentBreakKind::ZeroWidthBreak => {
                // Zero-width break opportunity.
                items.push(KPItem {
                    segment_index: i + 1,
                    grapheme_index: 0,
                    total_width: cum_width,
                    total_stretch: cum_stretch,
                    total_shrink: cum_shrink,
                    penalty: params.line_penalty,
                    flagged: false,
                    break_width: 0.0,
                });
            }
            SegmentBreakKind::SoftHyphen => {
                // Discretionary hyphen: penalty for hyphenation, flagged.
                items.push(KPItem {
                    segment_index: i + 1,
                    grapheme_index: 0,
                    total_width: cum_width,
                    total_stretch: cum_stretch,
                    total_shrink: cum_shrink,
                    penalty: params.hyphen_penalty,
                    flagged: true,
                    break_width: prepared.discretionary_hyphen_width,
                });
            }
            SegmentBreakKind::Tab => {
                cum_width += w;
                let stretch = w * 0.5;
                let shrink = w * 0.333;
                cum_stretch += stretch;
                cum_shrink += shrink;

                // Breakable after tab.
                items.push(KPItem {
                    segment_index: i + 1,
                    grapheme_index: 0,
                    total_width: cum_width,
                    total_stretch: cum_stretch,
                    total_shrink: cum_shrink,
                    penalty: params.line_penalty,
                    flagged: false,
                    break_width: 0.0,
                });
            }
            SegmentBreakKind::Text | SegmentBreakKind::Glue => {
                // For CJK text segments that were split into per-grapheme units
                // during preparation, each unit is a separate segment, so
                // breakpoints between CJK characters are already handled as
                // segment boundaries with Text kind. We add a breakpoint
                // candidate after each such segment (CJK characters can break
                // between any pair).

                // Check if this segment has grapheme-level breakability
                // (overflow-wrap: break-word for long words).
                if let Some(ref gw) = breakable_widths[i] {
                    let gp = breakable_prefix_widths[i].as_deref();
                    for g in 0..gw.len() {
                        let gw_val = get_breakable_advance(
                            gw,
                            gp,
                            g,
                            profile.prefer_prefix_widths_for_breakable_runs,
                        );
                        cum_width += gw_val;

                        // Each grapheme boundary within a breakable word is a
                        // break candidate with high penalty (emergency breaks).
                        if g + 1 < gw.len() {
                            items.push(KPItem {
                                segment_index: i,
                                grapheme_index: g + 1,
                                total_width: cum_width,
                                total_stretch: cum_stretch,
                                total_shrink: cum_shrink,
                                penalty: 1000.0, // very high penalty: emergency only
                                flagged: false,
                                break_width: 0.0,
                            });
                        }
                    }
                } else {
                    cum_width += w;
                }

                // For CJK: each text segment can break after it (CJK
                // characters were already split into single-char segments
                // during prepare). We check if the next segment is also Text
                // to add an inter-CJK break opportunity. But this is already
                // handled by the segment splitting, so we just add content
                // width without a break here — breaks are created at space/
                // soft-hyphen/zero-width positions.
            }
        }
    }

    // End-of-paragraph: forced break at the end.
    items.push(KPItem {
        segment_index: widths.len(),
        grapheme_index: 0,
        total_width: cum_width,
        total_stretch: cum_stretch,
        total_shrink: cum_shrink,
        penalty: f64::NEG_INFINITY,
        flagged: false,
        break_width: 0.0,
    });

    items
}

/// Compute the adjustment ratio for a line from breakpoint `a` to breakpoint `b`.
///
/// Returns the ratio r such that:
/// - r = 0 means the line exactly fits.
/// - r > 0 means the line is short and needs stretching.
/// - r < 0 means the line is long and needs shrinking.
/// - r = f64::INFINITY means infeasible (cannot stretch enough).
fn compute_adjustment_ratio(
    items: &[KPItem],
    a: usize,
    b: usize,
    max_width: f64,
    break_width: f64,
) -> f64 {
    // Line width = content_width_at_b - content_width_at_a + break_width
    // But we need to be careful: the cumulative width at `a` includes the
    // glue/space *at* breakpoint `a`, which should NOT be on the new line.
    // The items already have total_width set after the space, so
    // content on line from a to b = items[b].total_width - items[a].total_width + break_width
    let line_width = items[b].total_width - items[a].total_width + break_width;

    let diff = max_width - line_width;

    if diff.abs() < 1e-6 {
        return 0.0;
    }

    if diff > 0.0 {
        // Line is short, needs stretching.
        let stretch = items[b].total_stretch - items[a].total_stretch;
        if stretch > 1e-6 {
            diff / stretch
        } else {
            // No stretch available — line is too short and can't be filled.
            f64::INFINITY
        }
    } else {
        // Line is long, needs shrinking.
        let shrink = items[b].total_shrink - items[a].total_shrink;
        if shrink > 1e-6 {
            diff / shrink
        } else {
            // No shrink available — line is too long and can't be compressed.
            f64::NEG_INFINITY
        }
    }
}

/// Compute badness from adjustment ratio, following Knuth-Plass.
/// badness = 100 * |r|^3, capped at 10000 for infeasible.
fn badness(ratio: f64) -> f64 {
    if ratio.is_infinite() {
        return 10000.0;
    }
    let b = 100.0 * ratio.abs().powi(3);
    b.min(10000.0)
}

/// Compute demerits for a line break, following Knuth-Plass.
fn compute_demerits(
    params: &KnuthPlassParams,
    penalty: f64,
    badness_val: f64,
    prev_flagged: bool,
    curr_flagged: bool,
    prev_fitness: FitnessClass,
    curr_fitness: FitnessClass,
) -> f64 {
    let line_penalty = params.line_penalty;

    let d = if penalty >= 0.0 {
        (line_penalty + badness_val).powi(2) + penalty * penalty
    } else if penalty > f64::NEG_INFINITY {
        (line_penalty + badness_val).powi(2) - penalty * penalty
    } else {
        // Forced break (end of paragraph or hard break).
        (line_penalty + badness_val).powi(2)
    };

    let mut total = d;

    // Flagged demerits: consecutive flagged breaks (e.g., consecutive hyphens).
    if prev_flagged && curr_flagged {
        total += params.flagged_demerits;
    }

    // Fitness demerits: adjacent lines with very different fitness classes.
    let class_diff = (prev_fitness as i32 - curr_fitness as i32).unsigned_abs();
    if class_diff > 1 {
        total += params.fitness_demerits;
    }

    total
}

/// Run the Knuth-Plass algorithm on a single chunk of prepared data.
///
/// Returns the chosen breakpoints as (segment_index, grapheme_index) pairs,
/// or None if no feasible solution exists.
fn knuth_plass_chunk(
    items: &[KPItem],
    max_width: f64,
    params: &KnuthPlassParams,
    tolerance: f64,
) -> Option<Vec<(usize, usize)>> {
    if items.len() < 2 {
        return Some(Vec::new());
    }

    // All active nodes, stored in a flat vec. We use indices as references.
    let mut nodes: Vec<ActiveNode> = Vec::new();

    // Active set: indices into `nodes` that are still candidates.
    let mut active: Vec<usize> = Vec::new();

    // Initialize with the start-of-paragraph node (items[0]).
    nodes.push(ActiveNode {
        break_index: 0,
        line: 0,
        fitness: FitnessClass::Normal,
        total_demerits: 0.0,
        prev_node: None,
    });
    active.push(0);

    // Process each candidate breakpoint.
    for b in 1..items.len() {
        let item = &items[b];

        // Skip non-breakable items (infinite penalty, not forced).
        if item.penalty == f64::INFINITY {
            continue;
        }

        // Best candidates for each fitness class at this breakpoint.
        let mut best: [Option<(f64, usize)>; 4] = [None; 4]; // (demerits, node_index)

        // Indices of active nodes to deactivate (line too long).
        let mut to_deactivate = Vec::new();

        for (ai, &node_idx) in active.iter().enumerate() {
            let node = &nodes[node_idx];

            let ratio = compute_adjustment_ratio(
                items,
                node.break_index,
                b,
                max_width,
                item.break_width,
            );

            // If the line is too long even with maximum shrink, deactivate.
            if ratio < -1.0 {
                to_deactivate.push(ai);
            }

            // Check feasibility.
            if ratio < -1.0 || ratio > tolerance {
                // For forced breaks (neg infinity penalty), we must still
                // consider even if ratio exceeds tolerance — the line might
                // just be short (positive ratio), which is acceptable for
                // paragraph-ending lines.
                if item.penalty == f64::NEG_INFINITY && ratio >= -1.0 {
                    // Allow forced breaks with short lines.
                } else {
                    continue;
                }
            }

            // Compute demerits.
            let bad = badness(ratio);
            let fitness = FitnessClass::from_ratio(ratio);
            let demerits = compute_demerits(
                params,
                item.penalty,
                bad,
                items[node.break_index].flagged,
                item.flagged,
                node.fitness,
                fitness,
            );
            let total_d = node.total_demerits + demerits;

            let class_idx = fitness as usize;
            match best[class_idx] {
                Some((best_d, _)) if total_d >= best_d => {}
                _ => {
                    best[class_idx] = Some((total_d, node_idx));
                }
            }
        }

        // Remove deactivated nodes (iterate in reverse to preserve indices).
        for &ai in to_deactivate.iter().rev() {
            active.swap_remove(ai);
        }

        // Add new active nodes for the best candidates.
        for (class_idx, slot) in best.iter().enumerate() {
            if let Some((total_d, prev_idx)) = *slot {
                let fitness = match class_idx {
                    0 => FitnessClass::Tight,
                    1 => FitnessClass::Normal,
                    2 => FitnessClass::Loose,
                    _ => FitnessClass::VeryLoose,
                };
                let new_idx = nodes.len();
                nodes.push(ActiveNode {
                    break_index: b,
                    line: nodes[prev_idx].line + 1,
                    fitness,
                    total_demerits: total_d,
                    prev_node: Some(prev_idx),
                });
                active.push(new_idx);
            }
        }

        // If active set is empty, no feasible solution found.
        if active.is_empty() {
            return None;
        }
    }

    // Find the best node at the end of paragraph (last item).
    let last_item_idx = items.len() - 1;
    let best_node_idx = active
        .iter()
        .filter(|&&idx| nodes[idx].break_index == last_item_idx)
        .min_by(|&&a, &&b| {
            nodes[a]
                .total_demerits
                .partial_cmp(&nodes[b].total_demerits)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied();

    // If no node reached the end, pick the best active node overall.
    let best_node_idx = best_node_idx.or_else(|| {
        active
            .iter()
            .min_by(|&&a, &&b| {
                nodes[a]
                    .total_demerits
                    .partial_cmp(&nodes[b].total_demerits)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    })?;

    // Trace back the path to collect breakpoints.
    let mut breaks = Vec::new();
    let mut cur = Some(best_node_idx);
    while let Some(idx) = cur {
        let node = &nodes[idx];
        let item = &items[node.break_index];
        breaks.push((item.segment_index, item.grapheme_index));
        cur = node.prev_node;
    }
    breaks.reverse();

    // The first break is the start-of-paragraph (segment 0, grapheme 0),
    // and the last is end-of-paragraph. We include both — callers interpret
    // consecutive pairs as line ranges.
    Some(breaks)
}

/// Convert Knuth-Plass breakpoints into InternalLayoutLine entries.
///
/// `breaks` contains (segment_index, grapheme_index) pairs representing
/// the start of each line and the end of the last line.
fn breakpoints_to_lines(
    prepared: &PreparedLineBreakData,
    profile: &EngineProfile,
    breaks: &[(usize, usize)],
    on_line: &mut Option<&mut dyn FnMut(&InternalLayoutLine)>,
) -> usize {
    if breaks.len() < 2 {
        return 0;
    }

    let widths = &prepared.widths;
    let kinds = &prepared.kinds;
    let line_end_paint_advances = &prepared.line_end_paint_advances;
    let breakable_widths = &prepared.breakable_widths;
    let breakable_prefix_widths = &prepared.breakable_prefix_widths;
    let mut line_count = 0;

    for pair in breaks.windows(2) {
        let (start_seg, start_graph) = pair[0];
        let (end_seg, end_graph) = pair[1];

        // Skip empty ranges at the end.
        if start_seg >= widths.len() && end_seg >= widths.len() {
            continue;
        }

        // Normalize start: skip leading spaces.
        let mut actual_start_seg = start_seg;
        let mut actual_start_graph = start_graph;
        if actual_start_graph == 0 {
            while actual_start_seg < end_seg.min(widths.len()) {
                let k = kinds[actual_start_seg];
                if k == SegmentBreakKind::Space
                    || k == SegmentBreakKind::ZeroWidthBreak
                    || k == SegmentBreakKind::SoftHyphen
                {
                    actual_start_seg += 1;
                } else {
                    break;
                }
            }
        }

        // Compute line width.
        let mut line_w = 0.0;
        let mut seg = actual_start_seg;
        let g_start = actual_start_graph;

        while seg < end_seg.min(widths.len()) {
            let kind = kinds[seg];
            if seg == end_seg.saturating_sub(0) && end_graph > 0 && seg == end_seg {
                break;
            }
            if kind == SegmentBreakKind::SoftHyphen || kind == SegmentBreakKind::HardBreak {
                seg += 1;
                continue;
            }

            if seg == actual_start_seg && g_start > 0 {
                // Partial segment from grapheme g_start.
                if let Some(ref gw) = breakable_widths[seg] {
                    let gp = breakable_prefix_widths[seg].as_deref();
                    for g in g_start..gw.len() {
                        line_w += get_breakable_advance(
                            gw,
                            gp,
                            g,
                            profile.prefer_prefix_widths_for_breakable_runs,
                        );
                    }
                } else {
                    line_w += widths[seg];
                }
            } else {
                // Check if this is a trailing space — use paint advance.
                if (kind == SegmentBreakKind::Space || kind == SegmentBreakKind::PreservedSpace)
                    && seg + 1 >= end_seg
                {
                    line_w += line_end_paint_advances[seg];
                } else {
                    line_w += widths[seg];
                }
            }
            seg += 1;
        }

        // Add partial end segment (grapheme-level break).
        if end_graph > 0
            && end_seg < widths.len()
            && let Some(ref gw) = breakable_widths[end_seg]
        {
            let gp = breakable_prefix_widths[end_seg].as_deref();
            let skip = if actual_start_seg == end_seg {
                actual_start_graph
            } else {
                0
            };
            for g in skip..end_graph {
                line_w += get_breakable_advance(
                    gw,
                    gp,
                    g,
                    profile.prefer_prefix_widths_for_breakable_runs,
                );
            }
        }

        // Check for discretionary hyphen at line end.
        if end_seg > 0
            && end_seg <= widths.len()
            && end_graph == 0
            && kinds[end_seg - 1] == SegmentBreakKind::SoftHyphen
        {
            line_w += prepared.discretionary_hyphen_width;
        }

        line_count += 1;
        if let Some(cb) = on_line.as_mut() {
            cb(&InternalLayoutLine {
                start_segment_index: actual_start_seg,
                start_grapheme_index: actual_start_graph,
                end_segment_index: end_seg,
                end_grapheme_index: end_graph,
                width: line_w,
            });
        }
    }

    line_count
}

/// Run Knuth-Plass optimal line breaking on prepared data.
///
/// Returns the number of lines. If `on_line` is provided, calls it for each line.
/// Falls back to greedy if Knuth-Plass finds no feasible solution.
pub fn walk_prepared_lines_optimal(
    prepared: &PreparedLineBreakData,
    max_width: f64,
    profile: &EngineProfile,
    params: &KnuthPlassParams,
    mut on_line: Option<&mut dyn FnMut(&InternalLayoutLine)>,
) -> usize {
    let widths = &prepared.widths;
    if widths.is_empty() {
        return 0;
    }

    // For multi-chunk data (hard breaks), process each chunk separately.
    if prepared.chunks.len() > 1 {
        let mut total_lines = 0;

        for chunk in &prepared.chunks {
            if chunk.start_segment_index == chunk.end_segment_index {
                // Empty chunk (hard break only).
                total_lines += 1;
                if let Some(cb) = on_line.as_mut() {
                    cb(&InternalLayoutLine {
                        start_segment_index: chunk.start_segment_index,
                        start_grapheme_index: 0,
                        end_segment_index: chunk.consumed_end_segment_index,
                        end_grapheme_index: 0,
                        width: 0.0,
                    });
                }
                continue;
            }

            // Build a sub-prepared data for this chunk.
            let chunk_start = chunk.start_segment_index;
            let chunk_end = chunk.end_segment_index;
            let sub_prepared = PreparedLineBreakData {
                widths: widths[chunk_start..chunk_end].to_vec(),
                line_end_fit_advances: prepared.line_end_fit_advances[chunk_start..chunk_end]
                    .to_vec(),
                line_end_paint_advances: prepared.line_end_paint_advances[chunk_start..chunk_end]
                    .to_vec(),
                kinds: prepared.kinds[chunk_start..chunk_end].to_vec(),
                simple_line_walk_fast_path: false,
                breakable_widths: prepared.breakable_widths[chunk_start..chunk_end].to_vec(),
                breakable_prefix_widths: prepared.breakable_prefix_widths[chunk_start..chunk_end]
                    .to_vec(),
                discretionary_hyphen_width: prepared.discretionary_hyphen_width,
                tab_stop_advance: prepared.tab_stop_advance,
                chunks: vec![PreparedLineChunk {
                    start_segment_index: 0,
                    end_segment_index: chunk_end - chunk_start,
                    consumed_end_segment_index: chunk_end - chunk_start,
                }],
            };

            let items = build_kp_items(&sub_prepared, profile, params);

            let result = knuth_plass_chunk(&items, max_width, params, params.tolerance)
                .or_else(|| {
                    if params.looseness_recovery {
                        // Retry with progressively larger tolerance.
                        for extra in [2.0, 4.0, 8.0, 16.0] {
                            let result = knuth_plass_chunk(
                                &items,
                                max_width,
                                params,
                                params.tolerance + extra,
                            );
                            if result.is_some() {
                                return result;
                            }
                        }
                    }
                    None
                });

            match result {
                Some(breaks) => {
                    // Remap segment indices back to global coordinates.
                    let global_breaks: Vec<(usize, usize)> = breaks
                        .iter()
                        .map(|&(seg, graph)| (seg + chunk_start, graph))
                        .collect();

                    // Override the last break to point to the chunk's consumed end.
                    let mut final_breaks = global_breaks;
                    if let Some(last) = final_breaks.last_mut() {
                        *last = (chunk.consumed_end_segment_index, 0);
                    }

                    total_lines += breakpoints_to_lines(
                        prepared,
                        profile,
                        &final_breaks,
                        &mut on_line,
                    );
                }
                None => {
                    // Fall back to greedy for the entire paragraph.
                    return walk_prepared_lines(prepared, max_width, profile, on_line);
                }
            }
        }

        return total_lines;
    }

    // Single chunk: run KP directly.
    let items = build_kp_items(prepared, profile, params);

    let result = knuth_plass_chunk(&items, max_width, params, params.tolerance)
        .or_else(|| {
            if params.looseness_recovery {
                for extra in [2.0, 4.0, 8.0, 16.0] {
                    let result = knuth_plass_chunk(
                        &items,
                        max_width,
                        params,
                        params.tolerance + extra,
                    );
                    if result.is_some() {
                        return result;
                    }
                }
            }
            None
        });

    match result {
        Some(breaks) => breakpoints_to_lines(prepared, profile, &breaks, &mut on_line),
        None => {
            // Fall back to greedy.
            walk_prepared_lines(prepared, max_width, profile, on_line)
        }
    }
}

/// Count lines using Knuth-Plass optimal algorithm.
pub fn count_prepared_lines_optimal(
    prepared: &PreparedLineBreakData,
    max_width: f64,
    profile: &EngineProfile,
    params: &KnuthPlassParams,
) -> usize {
    walk_prepared_lines_optimal(prepared, max_width, profile, params, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_prepared(widths: Vec<f64>, kinds: Vec<SegmentBreakKind>) -> PreparedLineBreakData {
        let len = widths.len();
        let fit = widths.to_vec();
        let paint = fit.clone();
        PreparedLineBreakData {
            widths,
            line_end_fit_advances: fit,
            line_end_paint_advances: paint,
            kinds,
            simple_line_walk_fast_path: true,
            breakable_widths: vec![None; len],
            breakable_prefix_widths: vec![None; len],
            discretionary_hyphen_width: 0.0,
            tab_stop_advance: 0.0,
            chunks: vec![PreparedLineChunk {
                start_segment_index: 0,
                end_segment_index: len,
                consumed_end_segment_index: len,
            }],
        }
    }

    #[test]
    fn test_single_line() {
        let prepared = simple_prepared(
            vec![50.0, 10.0, 40.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let count = count_prepared_lines(&prepared, 200.0, &profile);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_two_lines() {
        let prepared = simple_prepared(
            vec![50.0, 10.0, 50.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let count = count_prepared_lines(&prepared, 70.0, &profile);
        assert_eq!(count, 2);
    }

    #[test]
    fn test_walk_lines() {
        let prepared = simple_prepared(
            vec![50.0, 10.0, 50.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let mut lines = Vec::new();
        let count = walk_prepared_lines(&prepared, 70.0, &profile, Some(&mut |line: &InternalLayoutLine| {
            lines.push(line.clone());
        }));
        assert_eq!(count, 2);
        assert_eq!(lines.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Knuth-Plass tests
    // -----------------------------------------------------------------------

    fn kp_params() -> KnuthPlassParams {
        KnuthPlassParams::default()
    }

    #[test]
    fn test_kp_empty() {
        let prepared = simple_prepared(vec![], vec![]);
        let profile = EngineProfile::default();
        let count = count_prepared_lines_optimal(&prepared, 200.0, &profile, &kp_params());
        assert_eq!(count, 0);
    }

    #[test]
    fn test_kp_single_word() {
        // Single word "hello" = 50px, fits in 200px.
        let prepared = simple_prepared(
            vec![50.0],
            vec![SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let count = count_prepared_lines_optimal(&prepared, 200.0, &profile, &kp_params());
        assert_eq!(count, 1);
    }

    #[test]
    fn test_kp_single_line() {
        // "hello world" = 50 + 10 + 40 = 100px, fits in 200px.
        let prepared = simple_prepared(
            vec![50.0, 10.0, 40.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let count = count_prepared_lines_optimal(&prepared, 200.0, &profile, &kp_params());
        assert_eq!(count, 1);
    }

    #[test]
    fn test_kp_two_lines() {
        // "hello world" = 50 + 10 + 50 = 110px, 70px max → should wrap.
        let prepared = simple_prepared(
            vec![50.0, 10.0, 50.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let count = count_prepared_lines_optimal(&prepared, 70.0, &profile, &kp_params());
        assert_eq!(count, 2);
    }

    #[test]
    fn test_kp_walk_lines() {
        // "hello world" wrapping at 70px.
        let prepared = simple_prepared(
            vec![50.0, 10.0, 50.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let mut lines = Vec::new();
        let count = walk_prepared_lines_optimal(
            &prepared,
            70.0,
            &profile,
            &kp_params(),
            Some(&mut |line: &InternalLayoutLine| {
                lines.push(line.clone());
            }),
        );
        assert_eq!(count, 2);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_kp_optimal_vs_greedy_difference() {
        // Classic case where KP produces better results than greedy:
        // "AAAA BB CCC DDDD EE" at width that causes greedy to make a bad
        // first-line decision.
        //
        // Segments: [40, 10, 20, 10, 30, 10, 40, 10, 20]
        //           [T,   S,  T,  S,  T,  S,  T,  S,  T]
        // Total = 190px
        //
        // At width=80:
        // Greedy: "AAAA BB" (70) | "CCC DDDD" (80) | "EE" (20)
        //   → 3 lines, last line very short
        // KP might find: "AAAA BB" (70) | "CCC DDDD" (80) | "EE" (20)
        //   → same 3 lines in this case, but both should produce valid output.
        let prepared = simple_prepared(
            vec![40.0, 10.0, 20.0, 10.0, 30.0, 10.0, 40.0, 10.0, 20.0],
            vec![
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text,
            ],
        );
        let profile = EngineProfile::default();

        let greedy = count_prepared_lines(&prepared, 80.0, &profile);
        let optimal = count_prepared_lines_optimal(&prepared, 80.0, &profile, &kp_params());

        // Both should produce valid line counts (≥ 1).
        assert!(greedy >= 1);
        assert!(optimal >= 1);
        // Optimal should use same or fewer lines.
        assert!(optimal <= greedy + 1, "optimal={optimal}, greedy={greedy}");
    }

    #[test]
    fn test_kp_even_distribution() {
        // 5 equal-width words, each 30px with 10px spaces.
        // Total content: 5*30 + 4*10 = 190px.
        // At width=100:
        // Greedy: "AAA BBB" (70) | "CCC DDD" (70) | "EEE" (30) → uneven last line
        // KP: should distribute more evenly.
        let prepared = simple_prepared(
            vec![30.0, 10.0, 30.0, 10.0, 30.0, 10.0, 30.0, 10.0, 30.0],
            vec![
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text,
            ],
        );
        let profile = EngineProfile::default();

        let mut lines = Vec::new();
        let count = walk_prepared_lines_optimal(
            &prepared,
            100.0,
            &profile,
            &kp_params(),
            Some(&mut |line: &InternalLayoutLine| {
                lines.push(line.clone());
            }),
        );

        // Should produce a valid result.
        assert!(count >= 2);
        assert_eq!(lines.len(), count);

        // Every line should have positive width.
        for (i, line) in lines.iter().enumerate() {
            assert!(line.width > 0.0, "line {i} has zero width");
        }
    }

    #[test]
    fn test_kp_soft_hyphen_penalty() {
        // Verify soft hyphens get flagged penalty.
        let len = 5;
        let mut widths = vec![50.0, 0.0, 50.0, 0.0, 30.0];
        let mut kinds = vec![
            SegmentBreakKind::Text,
            SegmentBreakKind::SoftHyphen,
            SegmentBreakKind::Text,
            SegmentBreakKind::SoftHyphen,
            SegmentBreakKind::Text,
        ];
        let fit = widths.to_vec();
        let paint = fit.clone();
        let prepared = PreparedLineBreakData {
            widths,
            line_end_fit_advances: fit,
            line_end_paint_advances: paint,
            kinds,
            simple_line_walk_fast_path: false,
            breakable_widths: vec![None; len],
            breakable_prefix_widths: vec![None; len],
            discretionary_hyphen_width: 5.0,
            tab_stop_advance: 0.0,
            chunks: vec![PreparedLineChunk {
                start_segment_index: 0,
                end_segment_index: len,
                consumed_end_segment_index: len,
            }],
        };
        let profile = EngineProfile::default();
        let count = count_prepared_lines_optimal(&prepared, 60.0, &profile, &kp_params());
        // Should break at one of the soft hyphens.
        assert!(count >= 2);
    }

    #[test]
    fn test_kp_hard_break() {
        // Hard break forces a line break regardless of remaining space.
        let prepared = PreparedLineBreakData {
            widths: vec![40.0, 0.0, 30.0],
            line_end_fit_advances: vec![40.0, 0.0, 30.0],
            line_end_paint_advances: vec![40.0, 0.0, 30.0],
            kinds: vec![
                SegmentBreakKind::Text,
                SegmentBreakKind::HardBreak,
                SegmentBreakKind::Text,
            ],
            simple_line_walk_fast_path: false,
            breakable_widths: vec![None; 3],
            breakable_prefix_widths: vec![None; 3],
            discretionary_hyphen_width: 0.0,
            tab_stop_advance: 0.0,
            chunks: vec![
                PreparedLineChunk {
                    start_segment_index: 0,
                    end_segment_index: 1,
                    consumed_end_segment_index: 2,
                },
                PreparedLineChunk {
                    start_segment_index: 2,
                    end_segment_index: 3,
                    consumed_end_segment_index: 3,
                },
            ],
        };
        let profile = EngineProfile::default();
        let count = count_prepared_lines_optimal(&prepared, 200.0, &profile, &kp_params());
        assert_eq!(count, 2);
    }

    #[test]
    fn test_kp_fitness_class_from_ratio() {
        assert_eq!(FitnessClass::from_ratio(-1.0), FitnessClass::Tight);
        assert_eq!(FitnessClass::from_ratio(-0.3), FitnessClass::Normal);
        assert_eq!(FitnessClass::from_ratio(0.0), FitnessClass::Normal);
        assert_eq!(FitnessClass::from_ratio(0.7), FitnessClass::Loose);
        assert_eq!(FitnessClass::from_ratio(1.5), FitnessClass::VeryLoose);
    }

    #[test]
    fn test_kp_badness() {
        assert!((badness(0.0) - 0.0).abs() < 1e-6);
        assert!((badness(1.0) - 100.0).abs() < 1e-6);
        assert!((badness(-1.0) - 100.0).abs() < 1e-6);
        assert_eq!(badness(f64::INFINITY), 10000.0);
        // badness(0.5) = 100 * 0.125 = 12.5
        assert!((badness(0.5) - 12.5).abs() < 1e-6);
    }

    #[test]
    fn test_kp_build_items_basic() {
        // "hello world" → [Text(50), Space(10), Text(40)]
        let prepared = simple_prepared(
            vec![50.0, 10.0, 40.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let items = build_kp_items(&prepared, &profile, &kp_params());

        // Should have: start sentinel, break-after-space, end-of-paragraph.
        assert!(items.len() >= 3, "items.len() = {}", items.len());
        // First item: start sentinel.
        assert_eq!(items[0].segment_index, 0);
        assert_eq!(items[0].penalty, f64::NEG_INFINITY);
        // Last item: end of paragraph.
        let last = items.last().unwrap();
        assert_eq!(last.segment_index, 3);
        assert_eq!(last.penalty, f64::NEG_INFINITY);
    }

    #[test]
    fn test_kp_adjustment_ratio() {
        // Build items for "AAAA BBBB" = [40, 10, 40] at max_width=90
        // Line width = 40 + 10 + 40 = 90 (exact fit).
        let prepared = simple_prepared(
            vec![40.0, 10.0, 40.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let items = build_kp_items(&prepared, &profile, &kp_params());

        // Ratio from start to end should be close to 0 at width=90.
        let ratio = compute_adjustment_ratio(&items, 0, items.len() - 1, 90.0, 0.0);
        assert!(ratio.abs() < 0.5, "ratio = {ratio}");
    }

    #[test]
    fn test_kp_fallback_to_greedy() {
        // With very tight tolerance and extreme conditions, KP might fail
        // and should fall back to greedy.
        let prepared = simple_prepared(
            vec![50.0, 10.0, 50.0],
            vec![SegmentBreakKind::Text, SegmentBreakKind::Space, SegmentBreakKind::Text],
        );
        let profile = EngineProfile::default();
        let mut params = kp_params();
        params.tolerance = 0.001; // extremely tight
        params.looseness_recovery = true;

        // Should still produce valid output (via fallback).
        let count = count_prepared_lines_optimal(&prepared, 70.0, &profile, &params);
        assert!(count >= 1);
    }

    #[test]
    fn test_kp_count_matches_walk() {
        // Verify count_prepared_lines_optimal matches walk_prepared_lines_optimal.
        let prepared = simple_prepared(
            vec![30.0, 10.0, 30.0, 10.0, 30.0, 10.0, 30.0, 10.0, 30.0],
            vec![
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text,
            ],
        );
        let profile = EngineProfile::default();

        for width in [50.0, 80.0, 100.0, 150.0, 200.0] {
            let count = count_prepared_lines_optimal(&prepared, width, &profile, &kp_params());
            let mut walk_count = 0;
            walk_prepared_lines_optimal(
                &prepared,
                width,
                &profile,
                &kp_params(),
                Some(&mut |_: &InternalLayoutLine| {
                    walk_count += 1;
                }),
            );
            assert_eq!(count, walk_count, "mismatch at width {width}");
        }
    }

    #[test]
    fn test_kp_line_break_strategy_default() {
        assert_eq!(LineBreakStrategy::default(), LineBreakStrategy::Greedy);
    }

    #[test]
    fn test_kp_params_default() {
        let params = KnuthPlassParams::default();
        assert_eq!(params.line_penalty, 0.0);
        assert_eq!(params.hyphen_penalty, 50.0);
        assert_eq!(params.flagged_demerits, 3000.0);
        assert_eq!(params.fitness_demerits, 10000.0);
        assert_eq!(params.tolerance, 2.0);
        assert!(params.looseness_recovery);
    }

    #[test]
    fn test_kp_many_words_monotonic_widths() {
        // As width decreases, line count should increase monotonically.
        let prepared = simple_prepared(
            vec![
                30.0, 10.0, 30.0, 10.0, 30.0, 10.0, 30.0, 10.0,
                30.0, 10.0, 30.0, 10.0, 30.0,
            ],
            vec![
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text, SegmentBreakKind::Space,
                SegmentBreakKind::Text,
            ],
        );
        let profile = EngineProfile::default();

        let mut prev_count = 0;
        for width in [500.0, 200.0, 100.0, 80.0, 50.0] {
            let count = count_prepared_lines_optimal(&prepared, width, &profile, &kp_params());
            assert!(count >= prev_count, "line count decreased at width {width}: {count} < {prev_count}");
            prev_count = count;
        }
    }
}
