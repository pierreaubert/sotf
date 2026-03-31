/// Line breaking algorithm, ported from chenglou/pretext.
///
/// Implements greedy line breaking with pending-break tracking, soft hyphen
/// support, tab stops, and overflow-wrap grapheme-level breaking. Has both
/// a "simple" fast path (no tabs/soft-hyphens) and a full complex path.
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
}
