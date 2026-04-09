//! Import .docx files into markdown text.

use std::io::Cursor;
use std::path::Path;

/// Import a .docx file and return its content as markdown text.
pub fn import_docx(path: &Path) -> Result<String, DocxImportError> {
    let docx_file = docx::DocxFile::from_file(path)
        .map_err(|e| DocxImportError::Io(format!("{:?}", e)))?;
    let docx = docx_file
        .parse()
        .map_err(|e| DocxImportError::Parse(format!("{:?}", e)))?;
    Ok(docx_to_markdown(&docx))
}

/// Import .docx from bytes.
pub fn import_docx_bytes(bytes: &[u8]) -> Result<String, DocxImportError> {
    let cursor = Cursor::new(bytes);
    let docx_file = docx::DocxFile::from_reader(cursor)
        .map_err(|e| DocxImportError::Io(format!("{:?}", e)))?;
    let docx = docx_file
        .parse()
        .map_err(|e| DocxImportError::Parse(format!("{:?}", e)))?;
    Ok(docx_to_markdown(&docx))
}

fn docx_to_markdown(docx: &docx::Docx) -> String {
    let mut md = String::new();

    for child in &docx.document.body.content {
        match child {
            docx::document::BodyContent::Paragraph(para) => {
                convert_paragraph(para, &mut md);
            }
            docx::document::BodyContent::Table(table) => {
                convert_table(table, &mut md);
            }
        }
    }

    md.trim_end().to_string() + "\n"
}

fn convert_paragraph(para: &docx::document::Paragraph, md: &mut String) {
    // Check for heading style
    let heading_level = para
        .property
        .style_id
        .as_ref()
        .and_then(|s| extract_heading_level(&s.value));

    if let Some(level) = heading_level {
        md.push_str(&"#".repeat(level.min(6)));
        md.push(' ');
    }

    // Collect run segments with their formatting info for boundary-aware assembly.
    let mut segments: Vec<(String, bool, bool)> = Vec::new(); // (text, bold, italic)
    for content in &para.content {
        if let docx::document::ParagraphContent::Run(run) = content {
            let text = extract_run_text(run);
            if text.is_empty() {
                continue;
            }
            let is_bold = run.property.bold.is_some();
            let is_italic = run.property.italics.is_some();
            segments.push((text, is_bold, is_italic));
        }
    }

    let mut line = String::new();
    for (idx, (text, is_bold, is_italic)) in segments.iter().enumerate() {
        let is_formatted = *is_bold || *is_italic;

        if is_formatted {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                line.push_str(text);
                continue;
            }
            let prefix_ws = &text[..text.len() - text.trim_start().len()];
            let suffix_ws = &text[text.trim_end().len()..];

            line.push_str(prefix_ws);
            if *is_bold && *is_italic {
                line.push_str(&format!("***{}***", trimmed));
            } else if *is_bold {
                line.push_str(&format!("**{}**", trimmed));
            } else {
                line.push_str(&format!("*{}*", trimmed));
            }
            line.push_str(suffix_ws);
        } else {
            let mut t = text.clone();

            // If this plain run comes right before a formatted run,
            // trim trailing whitespace to avoid " **" in the output.
            let next_is_formatted =
                segments.get(idx + 1).is_some_and(|(_, b, i)| *b || *i);
            if next_is_formatted {
                t = t.trim_end().to_string();
            }

            // If this plain run comes right after a formatted run,
            // trim leading whitespace to avoid "** " in the output.
            if idx > 0 && segments.get(idx - 1).is_some_and(|(_, b, i)| *b || *i) {
                t = t.trim_start().to_string();
            }

            line.push_str(&t);
        }
    }

    // Detect list items: paragraphs starting with bullet character or number markers.
    // Word exports list items as plain paragraphs with bullet/number prefix text.
    let list_prefix = detect_list_prefix(&line);
    if let Some((marker, rest)) = list_prefix {
        md.push_str(&marker);
        md.push_str(rest);
    } else {
        md.push_str(&line);
    }

    md.push('\n');
    if heading_level.is_some() {
        md.push('\n');
    }
}

/// Detect if a line starts with a list marker pattern.
///
/// Recognizes:
/// - Bullet: lines starting with U+2022 (bullet character) or U+2023 or U+25E6
/// - Ordered: lines starting with a digit sequence followed by `.` or `)` and a space
///
/// Returns the markdown marker and the remaining text, or None if not a list item.
fn detect_list_prefix(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();

    // Bullet characters: •, ‣, ◦
    for bullet in ['\u{2022}', '\u{2023}', '\u{25E6}'] {
        if let Some(rest) = trimmed.strip_prefix(bullet) {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            return Some(("- ".to_string(), rest));
        }
    }

    // Ordered list: "1. " or "1) " patterns
    let mut chars = trimmed.char_indices();
    let mut found_digit = false;
    for (i, ch) in chars.by_ref() {
        if ch.is_ascii_digit() {
            found_digit = true;
        } else if found_digit && (ch == '.' || ch == ')') {
            let after = &trimmed[i + 1..];
            let after = after.strip_prefix(' ').unwrap_or(after);
            let num = &trimmed[..i + 1];
            return Some((format!("{} ", num), after));
        } else {
            break;
        }
    }

    None
}

fn extract_heading_level(style_id: &str) -> Option<usize> {
    let lower = style_id.to_lowercase();
    lower
        .strip_prefix("heading")
        .and_then(|rest| rest.parse::<usize>().ok())
        .filter(|&level| (1..=6).contains(&level))
}

fn extract_run_text(run: &docx::document::Run) -> String {
    let mut text = String::new();
    for content in &run.content {
        match content {
            docx::document::RunContent::Text(t) => text.push_str(&t.text),
            docx::document::RunContent::Break(_) => text.push('\n'),
        }
    }
    text
}

fn convert_table(table: &docx::document::Table, md: &mut String) {
    let mut rows: Vec<Vec<String>> = Vec::new();

    for row in &table.rows {
        let mut cells = Vec::new();
        for cell in &row.cells {
            let mut cell_text = String::new();
            for content in &cell.content {
                let docx::document::TableCellContent::Paragraph(para) = content;
                for pc in &para.content {
                    if let docx::document::ParagraphContent::Run(run) = pc {
                        cell_text.push_str(&extract_run_text(run));
                    }
                }
            }
            cells.push(cell_text.trim().to_string());
        }
        rows.push(cells);
    }

    if rows.is_empty() {
        return;
    }

    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return;
    }

    if let Some(header) = rows.first() {
        md.push('|');
        for i in 0..cols {
            md.push_str(&format!(" {} |", header.get(i).map_or("", |s| s.as_str())));
        }
        md.push('\n');
        md.push('|');
        for _ in 0..cols {
            md.push_str(" --- |");
        }
        md.push('\n');
    }

    for row in rows.iter().skip(1) {
        md.push('|');
        for i in 0..cols {
            md.push_str(&format!(" {} |", row.get(i).map_or("", |s| s.as_str())));
        }
        md.push('\n');
    }
    md.push('\n');
}

#[derive(Debug)]
pub enum DocxImportError {
    Io(String),
    Parse(String),
}

impl std::fmt::Display for DocxImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {}", msg),
            Self::Parse(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for DocxImportError {}
