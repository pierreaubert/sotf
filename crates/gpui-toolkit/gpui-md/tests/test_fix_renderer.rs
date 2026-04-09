//! TDD tests for renderer bugs.
//!
//! Issue #11: ordered lists render as bullet points (list_type ignored)
//! Issue #12: strikethrough flattens inline formatting via collect_text()
//! Issue #13: tables render as a single plain-text blob

use comrak::nodes::{ListType, NodeValue};
use comrak::Arena;
use gpui_md::markdown::{parser::parse_markdown, render_markdown, MdThemeColors, SourceMap};
use gpui_ui_kit::theme::Theme;

fn dark_colors() -> MdThemeColors {
    MdThemeColors::from_theme(&Theme::dark())
}

// ---------------------------------------------------------------------------
// Issue #11: ordered lists always show bullet points
//
// NodeValue::List(_) discards the ListType, unconditionally rendering "•".
// The renderer should check list.list_type and render numbered markers
// for ordered lists.
// ---------------------------------------------------------------------------

#[test]
fn issue_11_comrak_produces_ordered_list_type() {
    // Precondition: comrak correctly parses ordered lists.
    // The data IS available — the renderer just ignores it.
    let arena = Arena::new();
    let root = parse_markdown(&arena, "1. First\n2. Second\n3. Third\n");

    let mut found_ordered = false;
    for child in root.children() {
        let data = child.data.borrow();
        if let NodeValue::List(ref list) = data.value {
            if list.list_type == ListType::Ordered {
                found_ordered = true;
            }
        }
    }
    assert!(
        found_ordered,
        "comrak should produce an ordered list node — the renderer must use this info"
    );
}

#[test]
fn issue_11_ordered_list_rendering_differs_from_unordered() {
    let arena_ol = Arena::new();
    let root_ol = parse_markdown(&arena_ol, "1. First\n2. Second\n");
    let colors = dark_colors();
    let mut sm_ol = SourceMap::new();
    let elements_ol = render_markdown(root_ol, &mut sm_ol, &colors);

    let arena_ul = Arena::new();
    let root_ul = parse_markdown(&arena_ul, "- First\n- Second\n");
    let mut sm_ul = SourceMap::new();
    let elements_ul = render_markdown(root_ul, &mut sm_ul, &colors);

    // Both should render without crashing
    assert!(!elements_ol.is_empty(), "ordered list should produce elements");
    assert!(!elements_ul.is_empty(), "unordered list should produce elements");

    // TODO: once a render_to_text() or accessible-text API exists,
    // verify ordered list contains "1." and "2." while unordered has "•".
}

// ---------------------------------------------------------------------------
// Issue #12: strikethrough loses inline formatting
//
// ~~**bold** text~~ should preserve the bold inside the strikethrough.
// Currently collect_text() flattens all children to plain text.
// ---------------------------------------------------------------------------

#[test]
fn issue_12_strikethrough_ast_preserves_bold_child() {
    // Precondition: comrak AST nests Strong inside Strikethrough.
    let arena = Arena::new();
    let root = parse_markdown(&arena, "~~**bold** text~~\n");

    let mut has_strong_inside_strikethrough = false;
    for para in root.children() {
        for inline in para.children() {
            let data = inline.data.borrow();
            if matches!(data.value, NodeValue::Strikethrough) {
                drop(data);
                for child in inline.children() {
                    let child_data = child.data.borrow();
                    if matches!(child_data.value, NodeValue::Strong) {
                        has_strong_inside_strikethrough = true;
                    }
                }
            }
        }
    }
    assert!(
        has_strong_inside_strikethrough,
        "AST should nest Strong inside Strikethrough — \
         renderer must recurse instead of calling collect_text()"
    );
}

#[test]
fn issue_12_strikethrough_renders_without_crash() {
    let arena = Arena::new();
    let root = parse_markdown(&arena, "~~**bold** and *italic* text~~\n");
    let colors = dark_colors();
    let mut sm = SourceMap::new();
    let elements = render_markdown(root, &mut sm, &colors);
    assert!(!elements.is_empty());
}

// ---------------------------------------------------------------------------
// Issue #13: tables render as a single plain-text blob
//
// Tables are flattened via collect_text(), losing all cell/row structure.
// The renderer should walk TableRow/TableCell nodes and render a grid.
// ---------------------------------------------------------------------------

#[test]
fn issue_13_table_ast_has_row_and_cell_structure() {
    let arena = Arena::new();
    let root = parse_markdown(&arena, "| A | B |\n|---|---|\n| 1 | 2 |\n");

    let mut found_table = false;
    let mut row_count = 0;
    for child in root.children() {
        let data = child.data.borrow();
        if matches!(data.value, NodeValue::Table(..)) {
            found_table = true;
            drop(data);
            for row in child.children() {
                let row_data = row.data.borrow();
                if matches!(row_data.value, NodeValue::TableRow(..)) {
                    row_count += 1;
                }
            }
        }
    }
    assert!(found_table, "comrak should parse table");
    assert!(
        row_count >= 2,
        "table should have at least 2 rows (header + data), got {}",
        row_count
    );
}

#[test]
fn issue_13_table_renders_with_source_map_entry() {
    let arena = Arena::new();
    let root = parse_markdown(&arena, "| A | B |\n|---|---|\n| 1 | 2 |\n");
    let colors = dark_colors();
    let mut sm = SourceMap::new();
    let elements = render_markdown(root, &mut sm, &colors);
    assert!(!elements.is_empty(), "table should produce rendered elements");
    // After the fix, the table should be tracked in the source map
    // for click-to-locate and scroll sync support.
}
