use std::sync::Arc;

use comrak::nodes::{AstNode, ListType, NodeValue};
use gpui::*;

use super::source_map::{SourceMap, SourceSpan};
use super::text_layout::layout_paragraph;
use super::theme_colors::MdThemeColors;

/// Render a comrak AST into GPUI elements with inline styling (bold, italic, code).
///
/// When `max_width_px` is provided (> 0), paragraph text is justified using
/// Knuth-Plass optimal line breaking with accurate font measurement.
pub fn render_markdown<'a>(
    root: &'a AstNode<'a>,
    source_map: &mut SourceMap,
    colors: &MdThemeColors,
    max_width_px: f32,
    text_system: Option<&Arc<WindowTextSystem>>,
    font_size: f32,
) -> Vec<AnyElement> {
    let mut elements = Vec::new();
    let mut block_counter: usize = 0;

    for child in root.children() {
        if let Some(el) = render_node(child, source_map, &mut block_counter, colors, max_width_px, text_system, font_size) {
            elements.push(el);
        }
    }

    elements
}

fn render_node<'a>(
    node: &'a AstNode<'a>,
    source_map: &mut SourceMap,
    counter: &mut usize,
    colors: &MdThemeColors,
    max_width_px: f32,
    text_system: Option<&Arc<WindowTextSystem>>,
    font_size: f32,
) -> Option<AnyElement> {
    let data = node.data.borrow();
    let sourcepos = data.sourcepos;
    let value = data.value.clone();
    drop(data);

    match value {
        NodeValue::Paragraph => {
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            // Collect styled runs from inline children
            let runs = collect_styled_runs(node, colors);

            // Use Knuth-Plass justification when width and text system are available
            if max_width_px > 0.0 && let Some(ts) = text_system {
                let elements = render_justified_styled_paragraph(
                    &runs,
                    max_width_px,
                    font_size,
                    colors,
                    ts,
                );
                return Some(
                    div()
                        .id(ElementId::Name(block_id.into()))
                        .mb_4()
                        .w_full()
                        .overflow_hidden()
                        .text_size(px(font_size))
                        .text_color(colors.text)
                        .children(elements)
                        .into_any_element(),
                );
            }

            // Fallback: flex-wrap layout when width is unknown
            let inline_elements = render_styled_runs_flat(&runs);
            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .mb_4()
                    .w_full()
                    .overflow_hidden()
                    .text_size(px(font_size))
                    .text_color(colors.text)
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .children(inline_elements)
                    .into_any_element(),
            )
        }

        NodeValue::Heading(ref heading) => {
            let level = heading.level;
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            let font_size = match level {
                1 => 28.0_f32,
                2 => 24.0,
                3 => 20.0,
                4 => 18.0,
                _ => 16.0,
            };

            // Collect heading text as a single string so GPUI wraps it naturally
            let heading_text = collect_text(node);

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .mt_4()
                    .mb_2()
                    .w_full()
                    .overflow_hidden()
                    .text_size(px(font_size))
                    .text_color(colors.text)
                    .font_weight(FontWeight::BOLD)
                    .child(heading_text)
                    .into_any_element(),
            )
        }

        NodeValue::CodeBlock(ref code_block) => {
            let literal = code_block.literal.clone();
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .my_2()
                    .p_3()
                    .rounded_md()
                    .bg(colors.code_block_bg)
                    .text_size(px(13.0))
                    .text_color(colors.text)
                    .font_family("monospace")
                    .w_full()
                    .overflow_x_scroll()
                    .child(div().whitespace_nowrap().child(literal))
                    .into_any_element(),
            )
        }

        NodeValue::List(ref list) => {
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            let is_ordered = list.list_type == ListType::Ordered;
            let start_num = list.start;

            let mut items = Vec::new();
            for (i, item) in node.children().enumerate() {
                let item_inlines = render_inlines_recursive(item, colors);

                let marker: String = if is_ordered {
                    format!("{}.", start_num + i)
                } else {
                    "\u{2022}".to_string()
                };

                items.push(
                    div()
                        .flex()
                        .flex_row()
                        .w_full()
                        .mb_1()
                        .child(
                            div()
                                .w(px(24.0))
                                .flex_shrink_0()
                                .text_color(colors.text_muted)
                                .child(marker),
                        )
                        .child(
                            div()
                                .flex_grow()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .text_color(colors.text)
                                .children(item_inlines),
                        )
                        .into_any_element(),
                );
            }

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .my_2()
                    .children(items)
                    .into_any_element(),
            )
        }

        NodeValue::BlockQuote => {
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            let inline_elements = render_inlines_recursive(node, colors);

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .my_2()
                    .pl_4()
                    .w_full()
                    .overflow_hidden()
                    .border_l_2()
                    .border_color(colors.border)
                    .text_color(colors.text_muted)
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .children(inline_elements)
                    .into_any_element(),
            )
        }

        NodeValue::ThematicBreak => {
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .my_4()
                    .h(px(1.0))
                    .bg(colors.hr)
                    .into_any_element(),
            )
        }

        NodeValue::Table(_) => {
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            let mut rows = Vec::new();
            let mut body_row_count = 0usize;
            for row_node in node.children() {
                let row_data = row_node.data.borrow();
                let is_table_row = matches!(row_data.value, NodeValue::TableRow(..));
                let is_header = if let NodeValue::TableRow(header) = &row_data.value {
                    *header
                } else {
                    false
                };
                drop(row_data);

                if !is_table_row {
                    continue;
                }

                let mut cells = Vec::new();
                for cell_node in row_node.children() {
                    let cell_inlines = render_inlines(cell_node, colors);
                    let mut cell_el = div()
                        .px_2()
                        .py_1()
                        .border_1()
                        .border_color(colors.border)
                        .flex_1()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .text_color(colors.text)
                        .children(cell_inlines);

                    if is_header {
                        cell_el = cell_el.font_weight(FontWeight::BOLD);
                    }

                    cells.push(cell_el.into_any_element());
                }

                let mut row_el = div().flex().flex_row().children(cells);
                if is_header {
                    row_el = row_el.bg(colors.table_header_bg);
                } else {
                    if body_row_count.is_multiple_of(2) {
                        row_el = row_el.bg(colors.table_bg);
                    }
                    body_row_count += 1;
                }

                rows.push(row_el.into_any_element());
            }

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .my_2()
                    .w_full()
                    .rounded_sm()
                    .overflow_x_scroll()
                    .text_size(px(font_size))
                    .children(rows)
                    .into_any_element(),
            )
        }

        _ => None,
    }
}

/// Render inline nodes of a block-level node (paragraph, heading) into styled GPUI elements.
/// Handles: Text, Strong (bold), Emph (italic), Code, Link, Strikethrough, SoftBreak, LineBreak.
fn render_inlines<'a>(node: &'a AstNode<'a>, colors: &MdThemeColors) -> Vec<AnyElement> {
    let mut elements = Vec::new();
    for child in node.children() {
        render_inline_node(child, colors, false, false, &mut elements);
    }
    elements
}

/// Like render_inlines but walks through intermediate nodes (e.g., ListItem → Paragraph → inlines).
fn render_inlines_recursive<'a>(node: &'a AstNode<'a>, colors: &MdThemeColors) -> Vec<AnyElement> {
    let mut elements = Vec::new();
    collect_inlines_deep(node, colors, false, false, &mut elements);
    elements
}

fn collect_inlines_deep<'a>(
    node: &'a AstNode<'a>,
    colors: &MdThemeColors,
    bold: bool,
    italic: bool,
    elements: &mut Vec<AnyElement>,
) {
    for child in node.children() {
        let data = child.data.borrow();
        let value = data.value.clone();
        drop(data);

        match value {
            NodeValue::Paragraph => {
                // Descend into paragraph to get its inline children
                for inline in child.children() {
                    render_inline_node(inline, colors, bold, italic, elements);
                }
            }
            _ => {
                render_inline_node(child, colors, bold, italic, elements);
            }
        }
    }
}

fn render_inline_node<'a>(
    node: &'a AstNode<'a>,
    colors: &MdThemeColors,
    bold: bool,
    italic: bool,
    elements: &mut Vec<AnyElement>,
) {
    let data = node.data.borrow();
    let value = data.value.clone();
    drop(data);

    match value {
        NodeValue::Text(t) => {
            let mut el = div().child(t);
            if bold {
                el = el.font_weight(FontWeight::BOLD);
            }
            if italic {
                el = el.italic();
            }
            elements.push(el.into_any_element());
        }

        NodeValue::Strong => {
            for child in node.children() {
                render_inline_node(child, colors, true, italic, elements);
            }
        }

        NodeValue::Emph => {
            for child in node.children() {
                render_inline_node(child, colors, bold, true, elements);
            }
        }

        NodeValue::Code(code) => {
            elements.push(
                div()
                    .px_1()
                    .rounded_sm()
                    .bg(colors.code_block_bg)
                    .font_family("monospace")
                    .text_size(px(13.0))
                    .child(code.literal)
                    .into_any_element(),
            );
        }

        NodeValue::Link(link) => {
            let mut link_elements = Vec::new();
            for child in node.children() {
                render_inline_node(child, colors, bold, italic, &mut link_elements);
            }
            // Wrap link children in a colored container
            let link_text = if link_elements.is_empty() {
                div().child(link.url.clone()).into_any_element()
            } else {
                div()
                    .flex()
                    .flex_row()
                    .children(link_elements)
                    .into_any_element()
            };
            elements.push(
                div()
                    .text_color(colors.syn_link)
                    .child(link_text)
                    .into_any_element(),
            );
        }

        NodeValue::Strikethrough => {
            let mut strike_elements = Vec::new();
            for child in node.children() {
                render_inline_node(child, colors, bold, italic, &mut strike_elements);
            }
            elements.push(
                div()
                    .line_through()
                    .flex()
                    .flex_row()
                    .children(strike_elements)
                    .into_any_element(),
            );
        }

        NodeValue::SoftBreak => {
            elements.push(div().child(" ").into_any_element());
        }

        NodeValue::LineBreak => {
            // Force a line break by ending the current flex row
            elements.push(div().w_full().into_any_element());
        }

        NodeValue::Image(image) => {
            let alt = collect_text(node);
            let display = if alt.is_empty() {
                image.url.clone()
            } else {
                alt
            };
            elements.push(
                div()
                    .text_color(colors.text_muted)
                    .italic()
                    .child(format!("[Image: {}]", display))
                    .into_any_element(),
            );
        }

        _ => {
            // For unhandled inline nodes, try to extract text
            for child in node.children() {
                render_inline_node(child, colors, bold, italic, elements);
            }
        }
    }
}

fn next_id(counter: &mut usize) -> String {
    let id = format!("md-block-{}", *counter);
    *counter += 1;
    id
}

/// Recursively collect all text from a node and its children (plain text, no formatting).
fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    collect_text_inner(node, &mut out);
    out
}

fn collect_text_inner<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for child in node.children() {
        let data = child.data.borrow();
        match &data.value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::SoftBreak => out.push(' '),
            NodeValue::LineBreak => out.push('\n'),
            NodeValue::Code(code) => {
                out.push('`');
                out.push_str(&code.literal);
                out.push('`');
            }
            NodeValue::Link(link) => {
                let url = link.url.clone();
                drop(data);
                let inner = collect_text(child);
                if inner.is_empty() {
                    out.push_str(&url);
                } else {
                    out.push_str(&inner);
                }
                continue;
            }
            _ => {
                drop(data);
                collect_text_inner(child, out);
                continue;
            }
        }
        drop(data);
    }
}

// ---- Styled runs for Knuth-Plass justified paragraphs ----

/// A text run with inline formatting metadata.
#[derive(Clone)]
struct StyledRun {
    text: String,
    bold: bool,
    italic: bool,
    color: Option<Rgba>,
    bg: Option<Rgba>,
    is_code: bool,
    is_strikethrough: bool,
}

/// Walk inline children and build a flat list of styled runs.
fn collect_styled_runs<'a>(node: &'a AstNode<'a>, colors: &MdThemeColors) -> Vec<StyledRun> {
    let mut runs = Vec::new();
    for child in node.children() {
        collect_styled_run(child, colors, false, false, None, false, &mut runs);
    }
    runs
}

fn collect_styled_run<'a>(
    node: &'a AstNode<'a>,
    colors: &MdThemeColors,
    bold: bool,
    italic: bool,
    color: Option<Rgba>,
    strikethrough: bool,
    runs: &mut Vec<StyledRun>,
) {
    let data = node.data.borrow();
    let value = data.value.clone();
    drop(data);

    match value {
        NodeValue::Text(t) => {
            runs.push(StyledRun {
                text: t,
                bold,
                italic,
                color,
                bg: None,
                is_code: false,
                is_strikethrough: strikethrough,
            });
        }
        NodeValue::Strong => {
            for child in node.children() {
                collect_styled_run(child, colors, true, italic, color, strikethrough, runs);
            }
        }
        NodeValue::Emph => {
            for child in node.children() {
                collect_styled_run(child, colors, bold, true, color, strikethrough, runs);
            }
        }
        NodeValue::Code(code) => {
            runs.push(StyledRun {
                text: code.literal,
                bold,
                italic,
                color: None,
                bg: Some(colors.code_block_bg),
                is_code: true,
                is_strikethrough: strikethrough,
            });
        }
        NodeValue::Link(link) => {
            let inner = collect_text(node);
            let text = if inner.is_empty() {
                link.url.clone()
            } else {
                inner
            };
            runs.push(StyledRun {
                text,
                bold,
                italic,
                color: Some(colors.syn_link),
                bg: None,
                is_code: false,
                is_strikethrough: strikethrough,
            });
        }
        NodeValue::Strikethrough => {
            for child in node.children() {
                collect_styled_run(child, colors, bold, italic, color, true, runs);
            }
        }
        NodeValue::SoftBreak => {
            runs.push(StyledRun {
                text: " ".to_string(),
                bold,
                italic,
                color,
                bg: None,
                is_code: false,
                is_strikethrough: strikethrough,
            });
        }
        NodeValue::Image(image) => {
            let alt = collect_text(node);
            let display = if alt.is_empty() {
                image.url.clone()
            } else {
                alt
            };
            runs.push(StyledRun {
                text: format!("[Image: {}]", display),
                bold: false,
                italic: true,
                color: Some(colors.text_muted),
                bg: None,
                is_code: false,
                is_strikethrough: false,
            });
        }
        _ => {
            for child in node.children() {
                collect_styled_run(child, colors, bold, italic, color, strikethrough, runs);
            }
        }
    }
}

/// Render styled runs as flat div elements (fallback when width is unknown).
fn render_styled_runs_flat(runs: &[StyledRun]) -> Vec<AnyElement> {
    runs.iter().map(|run| render_single_run(run)).collect()
}

/// Render a single styled run as a GPUI element.
fn render_single_run(run: &StyledRun) -> AnyElement {
    let mut el = div();
    if run.is_code {
        el = el
            .px_1()
            .rounded_sm()
            .font_family("monospace")
            .text_size(px(13.0));
    }
    if let Some(bg) = run.bg {
        el = el.bg(bg);
    }
    if let Some(color) = run.color {
        el = el.text_color(color);
    }
    if run.bold {
        el = el.font_weight(FontWeight::BOLD);
    }
    if run.italic {
        el = el.italic();
    }
    if run.is_strikethrough {
        el = el.line_through();
    }
    el.child(run.text.clone()).into_any_element()
}

/// Render a paragraph with Knuth-Plass justified line breaking.
///
/// Collects all inline text, runs KP optimal layout, splits styled runs at
/// line boundaries, and distributes extra space between words on non-last lines.
fn render_justified_styled_paragraph(
    runs: &[StyledRun],
    max_width_px: f32,
    font_size_px: f32,
    _colors: &MdThemeColors,
    text_system: &Arc<WindowTextSystem>,
) -> Vec<AnyElement> {
    // Build flat plain text for KP
    let flat_text: String = runs.iter().map(|r| r.text.as_str()).collect();
    if flat_text.trim().is_empty() {
        return vec![div().min_h(px(10.0)).into_any_element()];
    }

    // Run KP layout with accurate font measurement
    let lines = layout_paragraph(&flat_text, max_width_px, font_size_px, text_system);

    // Split styled runs at KP line boundaries and render each line
    let flat_chars: Vec<char> = flat_text.chars().collect();
    let mut char_cursor = 0usize;
    let mut run_idx = 0usize;
    let mut offset_in_run = 0usize;

    let total_lines = lines.len();
    let mut elements = Vec::new();

    for (line_i, justified_line) in lines.iter().enumerate() {
        let line_text = &justified_line.text;
        let is_last = line_i == total_lines - 1;

        // Skip the inter-line space that KP consumed (one space between lines)
        if line_i > 0
            && char_cursor < flat_chars.len()
            && flat_chars[char_cursor] == ' '
        {
            advance_cursor(runs, &mut run_idx, &mut offset_in_run, 1);
            char_cursor += 1;
        }

        // Collect styled runs for this line's characters
        let line_char_count = line_text.chars().count();
        let line_runs = extract_runs_for_chars(
            runs,
            &mut run_idx,
            &mut offset_in_run,
            line_char_count,
        );
        char_cursor += line_char_count;

        // Render this line
        if is_last || justified_line.width >= max_width_px * 0.85 {
            // Left-align last line or lines that already fill the width
            let run_elements: Vec<AnyElement> =
                line_runs.iter().map(|r| render_single_run(r)).collect();
            elements.push(
                div()
                    .flex()
                    .flex_row()
                    .children(run_elements)
                    .into_any_element(),
            );
        } else {
            // Justify: split into words and distribute extra space
            let word_groups = split_runs_into_words(&line_runs);
            if word_groups.len() <= 1 {
                let run_elements: Vec<AnyElement> =
                    line_runs.iter().map(|r| render_single_run(r)).collect();
                elements.push(
                    div()
                        .flex()
                        .flex_row()
                        .children(run_elements)
                        .into_any_element(),
                );
            } else {
                let gap_count = word_groups.len() - 1;
                let slack = max_width_px - justified_line.width;
                let extra_per_gap = slack / gap_count as f32;

                let mut word_elements: Vec<AnyElement> = Vec::new();
                for (i, word_runs) in word_groups.iter().enumerate() {
                    for run in word_runs {
                        word_elements.push(render_single_run(run));
                    }
                    if i < gap_count {
                        word_elements.push(
                            div()
                                .w(px(extra_per_gap))
                                .flex_shrink_0()
                                .into_any_element(),
                        );
                    }
                }
                elements.push(
                    div()
                        .flex()
                        .flex_row()
                        .w(px(max_width_px))
                        .children(word_elements)
                        .into_any_element(),
                );
            }
        }
    }

    elements
}

/// Advance the run cursor by `n` characters.
fn advance_cursor(runs: &[StyledRun], run_idx: &mut usize, offset: &mut usize, mut n: usize) {
    while n > 0 && *run_idx < runs.len() {
        let run_chars = runs[*run_idx].text.chars().count();
        let remaining = run_chars - *offset;
        if n >= remaining {
            n -= remaining;
            *run_idx += 1;
            *offset = 0;
        } else {
            *offset += n;
            n = 0;
        }
    }
}

/// Extract styled runs covering the next `char_count` characters from the run list.
fn extract_runs_for_chars(
    runs: &[StyledRun],
    run_idx: &mut usize,
    offset: &mut usize,
    mut char_count: usize,
) -> Vec<StyledRun> {
    let mut result = Vec::new();
    while char_count > 0 && *run_idx < runs.len() {
        let run = &runs[*run_idx];
        let run_chars = run.text.chars().count();
        let remaining = run_chars - *offset;
        let take = char_count.min(remaining);

        let text: String = run.text.chars().skip(*offset).take(take).collect();
        result.push(StyledRun {
            text,
            bold: run.bold,
            italic: run.italic,
            color: run.color,
            bg: run.bg,
            is_code: run.is_code,
            is_strikethrough: run.is_strikethrough,
        });

        char_count -= take;
        *offset += take;
        if *offset >= run_chars {
            *run_idx += 1;
            *offset = 0;
        }
    }
    result
}

/// Split a line's styled runs into word groups (separated by spaces).
fn split_runs_into_words(runs: &[StyledRun]) -> Vec<Vec<StyledRun>> {
    let mut groups: Vec<Vec<StyledRun>> = Vec::new();
    let mut current_group: Vec<StyledRun> = Vec::new();

    for run in runs {
        // Split this run's text at spaces
        let parts: Vec<&str> = run.text.split(' ').collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 && !current_group.is_empty() {
                // Space boundary — start a new word group
                groups.push(std::mem::take(&mut current_group));
            }
            if !part.is_empty() {
                current_group.push(StyledRun {
                    text: part.to_string(),
                    bold: run.bold,
                    italic: run.italic,
                    color: run.color,
                    bg: run.bg,
                    is_code: run.is_code,
                    is_strikethrough: run.is_strikethrough,
                });
            }
        }
    }

    if !current_group.is_empty() {
        groups.push(current_group);
    }

    groups
}
