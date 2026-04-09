use comrak::nodes::{AstNode, ListType, NodeValue};
use gpui::*;

use super::source_map::{SourceMap, SourceSpan};
use super::theme_colors::MdThemeColors;

const BODY_FONT_SIZE: f32 = 15.0;

/// Render a comrak AST into GPUI elements with inline styling (bold, italic, code).
pub fn render_markdown<'a>(
    root: &'a AstNode<'a>,
    source_map: &mut SourceMap,
    colors: &MdThemeColors,
) -> Vec<AnyElement> {
    let mut elements = Vec::new();
    let mut block_counter: usize = 0;

    for child in root.children() {
        if let Some(el) = render_node(child, source_map, &mut block_counter, colors) {
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
) -> Option<AnyElement> {
    let data = node.data.borrow();
    let sourcepos = data.sourcepos;
    let value = data.value.clone();
    drop(data);

    match value {
        NodeValue::Paragraph => {
            let block_id = next_id(counter);
            source_map.insert(block_id.clone(), SourceSpan::from_comrak(sourcepos));

            let inline_elements = render_inlines(node, colors);

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .mb_2()
                    .text_size(px(BODY_FONT_SIZE))
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

            let inline_elements = render_inlines(node, colors);

            Some(
                div()
                    .id(ElementId::Name(block_id.into()))
                    .mt_4()
                    .mb_2()
                    .text_size(px(font_size))
                    .text_color(colors.text)
                    .font_weight(FontWeight::BOLD)
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .children(inline_elements)
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
                    .child(literal)
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
                    .rounded_sm()
                    .overflow_hidden()
                    .text_size(px(BODY_FONT_SIZE))
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
