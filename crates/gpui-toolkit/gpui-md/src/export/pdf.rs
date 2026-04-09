//! Export markdown to PDF format (feature-gated: pdf-export).

use comrak::nodes::{AstNode, NodeValue};
use comrak::Arena;
use genpdf::elements;
use genpdf::fonts;
use genpdf::Element as _;
use std::path::Path;

use crate::markdown::parser::parse_markdown;

/// Export markdown text to a PDF file at the given path.
pub fn export_pdf(markdown: &str, path: &Path) -> Result<(), PdfExportError> {
    let font_family = fonts::from_files("", "Liberation", None)
        .or_else(|_| {
            fonts::from_files("/usr/share/fonts/truetype/liberation", "LiberationSans", None)
        })
        .or_else(|_| fonts::from_files("/System/Library/Fonts", "Helvetica", None))
        .or_else(|_| fonts::from_files("C:\\Windows\\Fonts", "arial", None))
        .map_err(|e| {
            PdfExportError::Render(format!(
                "No fonts available for PDF export: {}",
                e
            ))
        })?;

    let mut doc = genpdf::Document::new(font_family);
    doc.set_title("Markdown Export");
    doc.set_minimal_conformance();

    let decorator = genpdf::SimplePageDecorator::new();
    doc.set_page_decorator(decorator);

    // Parse markdown
    let arena = Arena::new();
    let root = parse_markdown(&arena, markdown);

    for child in root.children() {
        let data = child.data.borrow();
        let value = data.value.clone();
        drop(data);

        match value {
            NodeValue::Heading(heading) => {
                let level = heading.level;
                let text = collect_plain_text(child);
                let size = match level {
                    1 => 24,
                    2 => 20,
                    3 => 16,
                    _ => 14,
                };
                let para = elements::Paragraph::new(text)
                    .styled(genpdf::style::Style::new().bold().with_font_size(size));
                doc.push(para);
                doc.push(elements::Break::new(0.5));
            }

            NodeValue::Paragraph => {
                let text = collect_plain_text(child);
                if !text.is_empty() {
                    doc.push(elements::Paragraph::new(text));
                    doc.push(elements::Break::new(0.3));
                }
            }

            NodeValue::CodeBlock(code_block) => {
                for line in code_block.literal.trim_end().split('\n') {
                    let para = elements::Paragraph::new(line)
                        .styled(genpdf::style::Style::new().with_font_size(10));
                    doc.push(para);
                }
                doc.push(elements::Break::new(0.3));
            }

            NodeValue::List(_) => {
                let mut list = elements::UnorderedList::new();
                for item in child.children() {
                    let text = collect_plain_text(item);
                    list.push(elements::Paragraph::new(text));
                }
                doc.push(list);
                doc.push(elements::Break::new(0.3));
            }

            NodeValue::BlockQuote => {
                let text = collect_plain_text(child);
                let para = elements::Paragraph::new(format!("> {}", text))
                    .styled(genpdf::style::Style::new().italic());
                doc.push(para);
                doc.push(elements::Break::new(0.3));
            }

            NodeValue::ThematicBreak => {
                doc.push(elements::Paragraph::new("---"));
                doc.push(elements::Break::new(0.5));
            }

            NodeValue::Table(_) => {
                let rows = collect_table_rows(child);
                if !rows.is_empty() {
                    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
                    let mut table = elements::TableLayout::new(vec![1; cols]);
                    table.set_cell_decorator(elements::FrameCellDecorator::new(true, true, false));
                    for row_data in &rows {
                        let mut row = table.row();
                        for cell_text in row_data {
                            row.push_element(elements::Paragraph::new(cell_text.as_str()));
                        }
                        row.push().ok();
                    }
                    doc.push(table);
                    doc.push(elements::Break::new(0.3));
                }
            }

            _ => {}
        }
    }

    doc.render_to_file(path)
        .map_err(|e| PdfExportError::Render(format!("{}", e)))?;

    Ok(())
}

fn collect_plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    collect_plain_text_inner(node, &mut out);
    out
}

fn collect_plain_text_inner<'a>(node: &'a AstNode<'a>, out: &mut String) {
    for child in node.children() {
        let data = child.data.borrow();
        match &data.value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::SoftBreak => out.push(' '),
            NodeValue::LineBreak => out.push('\n'),
            NodeValue::Code(code) => out.push_str(&code.literal),
            _ => {
                drop(data);
                collect_plain_text_inner(child, out);
                continue;
            }
        }
        drop(data);
    }
}

fn collect_table_rows<'a>(node: &'a AstNode<'a>) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for child in node.children() {
        let data = child.data.borrow();
        let value = data.value.clone();
        drop(data);
        if let NodeValue::TableRow(_) = value {
            let mut cells = Vec::new();
            for cell_node in child.children() {
                cells.push(collect_plain_text(cell_node).trim().to_string());
            }
            rows.push(cells);
        }
    }
    rows
}

#[derive(Debug)]
pub enum PdfExportError {
    Render(String),
}

impl std::fmt::Display for PdfExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Render(msg) => write!(f, "PDF render error: {}", msg),
        }
    }
}

impl std::error::Error for PdfExportError {}
