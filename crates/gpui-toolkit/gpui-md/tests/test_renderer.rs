use comrak::Arena;
use gpui_md::markdown::parser::parse_markdown;

/// Verify that comrak parses inline formatting correctly.
#[test]
fn comrak_parses_bold_and_italic() {
    let arena = Arena::new();
    let text = "test *italic* **bold**\n";
    let root = parse_markdown(&arena, text);

    let mut found_emph = false;
    let mut found_strong = false;

    for child in root.children() {
        let data = child.data.borrow();
        if let comrak::nodes::NodeValue::Paragraph = data.value {
            drop(data);
            for inline in child.children() {
                let idata = inline.data.borrow();
                match &idata.value {
                    comrak::nodes::NodeValue::Emph => found_emph = true,
                    comrak::nodes::NodeValue::Strong => found_strong = true,
                    _ => {}
                }
            }
        }
    }

    assert!(found_emph, "comrak should parse *italic* as Emph node");
    assert!(found_strong, "comrak should parse **bold** as Strong node");
}

/// Verify render_markdown produces elements (doesn't panic).
#[test]
fn render_markdown_produces_elements() {
    let arena = Arena::new();
    let text = "test *italic* **bold** `code`\n";
    let root = parse_markdown(&arena, text);

    let mut source_map = gpui_md::markdown::SourceMap::new();
    let colors = gpui_md::markdown::MdThemeColors::from_theme(
        &gpui_ui_kit::theme::Theme::dark(),
    );

    let elements = gpui_md::markdown::render_markdown(root, &mut source_map, &colors, 0.0, None, 15.0);
    assert!(!elements.is_empty(), "Should produce at least one element for the paragraph");
}

/// Verify the inline node count is correct — each inline fragment should produce a separate element.
#[test]
fn render_inline_fragments_count() {
    let arena = Arena::new();
    // "hello *world* end" → Text("hello "), Emph(Text("world")), Text(" end")
    let text = "hello *world* end\n";
    let root = parse_markdown(&arena, text);

    // Count inline children of the paragraph
    let para = root.children().next().unwrap();
    let data = para.data.borrow();
    assert!(matches!(data.value, comrak::nodes::NodeValue::Paragraph));
    drop(data);

    let inline_count = para.children().count();
    // comrak produces: Text("hello "), Emph, Text(" end") = 3 inline nodes
    // (Emph contains Text("world") as a child)
    assert_eq!(inline_count, 3, "Expected 3 inline nodes: text, emph, text");
}

/// Verify bold+italic nesting works.
#[test]
fn comrak_parses_bold_italic_nesting() {
    let arena = Arena::new();
    let text = "***bold italic***\n";
    let root = parse_markdown(&arena, text);

    let para = root.children().next().unwrap();
    let first_inline = para.children().next().unwrap();
    let data = first_inline.data.borrow();
    // comrak wraps ***text*** as either Emph(Strong(Text)) or Strong(Emph(Text))
    let is_emph = matches!(data.value, comrak::nodes::NodeValue::Emph);
    let is_strong = matches!(data.value, comrak::nodes::NodeValue::Strong);
    assert!(is_emph || is_strong, "Should be Emph or Strong for ***text***");
}
