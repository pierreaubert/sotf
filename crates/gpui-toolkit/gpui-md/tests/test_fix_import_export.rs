//! TDD tests for import/export bugs.
//!
//! Issue #28:  DOCX import drops lists, loses bold at word boundaries
//! Issue #29:  PDF and DOCX export modules duplicate the GFM parser with different options

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Issue #28: DOCX import drops list markers
//
// Word represents lists via numPr in paragraph properties. The import
// currently treats all list items as plain paragraphs.
// ---------------------------------------------------------------------------

#[test]
fn issue_28_docx_round_trip_preserves_unordered_list() {
    let md = "- Alpha\n- Beta\n- Gamma\n";
    let tmp = temp_path("unordered_list");
    gpui_md::export::docx::export_docx(md, &tmp).unwrap();
    let result = gpui_md::import::docx::import_docx(&tmp).unwrap();
    cleanup(&tmp);

    // The imported text should contain list markers
    let has_markers = result.contains("- ") || result.contains("* ") || result.contains("• ");
    assert!(
        has_markers,
        "unordered list markers should survive round-trip, got:\n{}",
        result
    );
}

#[test]
fn issue_28_docx_round_trip_preserves_ordered_list() {
    let md = "1. First\n2. Second\n3. Third\n";
    let tmp = temp_path("ordered_list");
    gpui_md::export::docx::export_docx(md, &tmp).unwrap();
    let result = gpui_md::import::docx::import_docx(&tmp).unwrap();
    cleanup(&tmp);

    let has_numbers = result.contains("1.") || result.contains("1)");
    assert!(
        has_numbers,
        "ordered list markers should survive round-trip, got:\n{}",
        result
    );
}

// ---------------------------------------------------------------------------
// Issue #28b: DOCX import produces invalid bold markers when run has
// trailing whitespace — e.g. "**bold **" instead of "**bold**"
// ---------------------------------------------------------------------------

#[test]
fn issue_28_docx_round_trip_bold_markers_are_valid() {
    let md = "Some **bold** and normal text\n";
    let tmp = temp_path("bold_ws");
    gpui_md::export::docx::export_docx(md, &tmp).unwrap();
    let result = gpui_md::import::docx::import_docx(&tmp).unwrap();
    cleanup(&tmp);

    assert!(
        !result.contains("** "),
        "bold closing marker should not have trailing space inside: {}",
        result
    );
    assert!(
        !result.contains(" **"),
        "bold opening marker should not have leading space inside: {}",
        result
    );
    // Bold content should be preserved (check both ** and __ syntax)
    assert!(
        result.contains("**bold**") || result.contains("__bold__"),
        "bold text should be preserved in round-trip: {}",
        result
    );
}

#[test]
fn issue_28_docx_round_trip_italic_markers_are_valid() {
    let md = "Some *italic* and normal text\n";
    let tmp = temp_path("italic_ws");
    gpui_md::export::docx::export_docx(md, &tmp).unwrap();
    let result = gpui_md::import::docx::import_docx(&tmp).unwrap();
    cleanup(&tmp);

    // Italic markers should not have whitespace between marker and content
    // e.g. "* italic*" or "*italic *" are invalid
    let has_bad_open = result.contains("* ") && !result.starts_with("* "); // "* " at start is a list
    let has_bad_close = result.contains(" *") && !result.ends_with(" *");
    assert!(
        !has_bad_open,
        "italic opening marker should not have trailing space inside: {}",
        result
    );
    // Note: " *" can appear legitimately in markdown; this is a best-effort check
    let _ = has_bad_close;
}

// ---------------------------------------------------------------------------
// Issue #29: export modules duplicate the GFM parser with different options
//
// The DOCX export enables footnotes but the PDF export does not.
// Both should use the shared parse_markdown() from the markdown module.
// ---------------------------------------------------------------------------

#[test]
fn issue_29_docx_export_handles_footnotes() {
    let md = "Text with a footnote[^1].\n\n[^1]: This is the footnote.\n";
    let tmp = temp_path("footnote");
    let result = gpui_md::export::docx::export_docx(md, &tmp);
    cleanup(&tmp);

    assert!(
        result.is_ok(),
        "DOCX export should not fail on footnote syntax: {:?}",
        result.err()
    );
}

#[test]
fn issue_29_docx_export_round_trips_footnote_content() {
    let md = "Text with a footnote[^1].\n\n[^1]: This is the footnote.\n";
    let tmp = temp_path("footnote_rt");
    gpui_md::export::docx::export_docx(md, &tmp).unwrap();
    let result = gpui_md::import::docx::import_docx(&tmp).unwrap();
    cleanup(&tmp);

    assert!(
        result.contains("footnote"),
        "footnote content should be preserved in round-trip: {}",
        result
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gpui_md_test_{}_{}.docx",
        name,
        std::process::id()
    ))
}

fn cleanup(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
}
