//! Export markdown to .docx format.

use comrak::nodes::{AstNode, ListType, NodeValue};
use comrak::Arena;
use docx::document::{Paragraph, Run};
use docx::formatting::{
    Bold, CharacterProperty, Italics, ParagraphProperty, ParagraphStyleId, Strike,
};
use std::path::Path;

use crate::markdown::parser::parse_markdown;

/// Export markdown text to a .docx file at the given path.
pub fn export_docx(markdown: &str, path: &Path) -> Result<(), DocxExportError> {
    let arena = Arena::new();
    let root = parse_markdown(&arena, markdown);

    let mut doc = docx::Docx::default();

    for child in root.children() {
        let data = child.data.borrow();
        let value = data.value.clone();
        drop(data);

        match value {
            NodeValue::Heading(heading) => {
                let level = heading.level;
                let text = collect_plain_text(child);
                let para = Paragraph::default()
                    .property(
                        ParagraphProperty::default().style_id(ParagraphStyleId::from(
                            format!("Heading{}", level.min(6)),
                        )),
                    )
                    .push_text(text);
                doc.document.body.push(para);
            }

            NodeValue::Paragraph => {
                let runs = collect_runs(child);
                let mut para = Paragraph::default();
                for run in runs {
                    para = para.push(run);
                }
                doc.document.body.push(para);
            }

            NodeValue::CodeBlock(code_block) => {
                for line in code_block.literal.trim_end().split('\n') {
                    let para = Paragraph::default().push_text(line.to_string());
                    doc.document.body.push(para);
                }
            }

            NodeValue::List(ref list) => {
                let ordered = list.list_type == ListType::Ordered;
                let mut index = list.start;
                for item in child.children() {
                    let text = collect_plain_text(item);
                    let marker = if ordered {
                        let m = format!("{}. ", index);
                        index += 1;
                        m
                    } else {
                        "\u{2022} ".to_string()
                    };
                    let para = Paragraph::default().push_text(format!("{}{}", marker, text));
                    doc.document.body.push(para);
                }
            }

            NodeValue::BlockQuote => {
                let text = collect_plain_text(child);
                let run = Run::default()
                    .property(CharacterProperty::default().italics(Italics::default()))
                    .push_text(text);
                let para = Paragraph::default().push(run);
                doc.document.body.push(para);
            }

            NodeValue::ThematicBreak => {
                let para = Paragraph::default().push_text(String::from("---"));
                doc.document.body.push(para);
            }

            NodeValue::Table(_) => {
                let rows = collect_table_text(child);
                for row_text in rows {
                    let para = Paragraph::default().push_text(row_text);
                    doc.document.body.push(para);
                }
            }

            _ => {}
        }
    }

    doc.write_file(path)
        .map_err(|e| DocxExportError::Build(format!("{:?}", e)))?;

    Ok(())
}

fn collect_runs<'a>(node: &'a AstNode<'a>) -> Vec<Run<'static>> {
    let mut runs = Vec::new();
    collect_runs_inner(node, false, false, &mut runs);
    runs
}

fn collect_runs_inner<'a>(
    node: &'a AstNode<'a>,
    bold: bool,
    italic: bool,
    runs: &mut Vec<Run<'static>>,
) {
    for child in node.children() {
        let data = child.data.borrow();
        let value = data.value.clone();
        drop(data);

        match value {
            NodeValue::Text(t) => {
                let mut prop = CharacterProperty::default();
                if bold {
                    prop = prop.bold(Bold::default());
                }
                if italic {
                    prop = prop.italics(Italics::default());
                }
                let run = Run::default().property(prop).push_text(t);
                runs.push(run);
            }
            NodeValue::Strong => collect_runs_inner(child, true, italic, runs),
            NodeValue::Emph => collect_runs_inner(child, bold, true, runs),
            NodeValue::Code(code) => {
                let run = Run::default().push_text(code.literal);
                runs.push(run);
            }
            NodeValue::SoftBreak => {
                let run = Run::default().push_text(String::from(" "));
                runs.push(run);
            }
            NodeValue::LineBreak => {
                let run = Run::default().push_break(None);
                runs.push(run);
            }
            NodeValue::Strikethrough => {
                let mut prop = CharacterProperty::default().strike(Strike::default());
                if bold {
                    prop = prop.bold(Bold::default());
                }
                if italic {
                    prop = prop.italics(Italics::default());
                }
                let text = collect_plain_text(child);
                let run = Run::default().property(prop).push_text(text);
                runs.push(run);
            }
            _ => collect_runs_inner(child, bold, italic, runs),
        }
    }
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

fn collect_table_text<'a>(node: &'a AstNode<'a>) -> Vec<String> {
    let mut lines = Vec::new();
    for child in node.children() {
        let data = child.data.borrow();
        let value = data.value.clone();
        drop(data);
        if let NodeValue::TableRow(_) = value {
            let mut cells = Vec::new();
            for cell_node in child.children() {
                cells.push(collect_plain_text(cell_node).trim().to_string());
            }
            lines.push(format!("| {} |", cells.join(" | ")));
        }
    }
    lines
}

#[derive(Debug)]
pub enum DocxExportError {
    Io(String),
    Build(String),
}

impl std::fmt::Display for DocxExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {}", msg),
            Self::Build(msg) => write!(f, "Build error: {}", msg),
        }
    }
}

impl std::error::Error for DocxExportError {}
