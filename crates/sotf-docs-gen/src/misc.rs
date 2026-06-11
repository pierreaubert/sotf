use std::path::Path;

/// Escape a string so it is safe to embed inside a single markdown table cell.
///
/// Markdown pipe tables use `|` as a column separator and treat each row as a
/// single line. Backslashes also need escaping so they do not consume the
/// following character. Newlines and carriage returns are replaced with `<br>`
/// so the cell continues to render on a single row.
pub(super) fn md_cell(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            '\n' | '\r' => out.push_str("<br>"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escape a string for use inside a YAML double-quoted scalar.
///
/// We restrict ourselves to single-line strings: any newline or carriage
/// return is replaced with a space so the value stays a single-line scalar.
/// Backslashes and double quotes get backslash-escaped; control characters
/// (other than the newline replacement above) are also stripped because they
/// are not allowed in double-quoted YAML scalars without unicode escapes,
/// which we deliberately avoid here.
pub(super) fn yaml_double_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            // Strip other control characters to keep the scalar valid.
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

pub(super) fn finish_markdown(mut md: String) -> String {
    while md.ends_with("\n\n") {
        md.pop();
    }
    if !md.ends_with('\n') {
        md.push('\n');
    }
    md
}

pub(super) fn print_usage() {
    eprintln!("Usage: sotf-docs-gen [--root <workspace-root>] [--check]");
}

pub(super) fn is_project_root(dir: &Path) -> bool {
    let cargo_toml = dir.join("Cargo.toml");
    let Ok(cargo) = std::fs::read_to_string(cargo_toml) else {
        return false;
    };
    cargo.contains("[workspace]") && dir.join("site/src/content/docs").is_dir()
}

pub(super) fn write_if_changed(path: &Path, content: &str, check_only: bool) -> bool {
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == content) {
        return false;
    }
    if check_only {
        println!("  would update {}", path.display());
        return true;
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create_dir_all");
    }
    std::fs::write(path, content)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", path.display()));
    println!("  wrote {}", path.display());
    true
}
