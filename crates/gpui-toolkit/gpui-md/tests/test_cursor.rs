use gpui_md::document::EditorCursor;

#[test]
fn new_cursor_at_zero() {
    let c = EditorCursor::new();
    assert_eq!(c.position, 0);
    assert!(c.anchor.is_none());
    assert!(!c.has_selection());
}

#[test]
fn cursor_at_position() {
    let c = EditorCursor::at(42);
    assert_eq!(c.position, 42);
    assert!(!c.has_selection());
}

#[test]
fn start_and_clear_selection() {
    let mut c = EditorCursor::at(5);
    assert!(!c.has_selection());

    c.start_selection();
    assert_eq!(c.anchor, Some(5));
    // anchor == position → not really a selection yet
    assert!(!c.has_selection());

    c.position = 10;
    assert!(c.has_selection());
    assert_eq!(c.selection(), Some((5, 10)));

    c.clear_selection();
    assert!(!c.has_selection());
    assert!(c.anchor.is_none());
}

#[test]
fn selection_normalizes_order() {
    let mut c = EditorCursor::new();
    c.anchor = Some(10);
    c.position = 3;
    // selection() should return (min, max) regardless of direction
    assert_eq!(c.selection(), Some((3, 10)));
}

#[test]
fn move_to_without_extend() {
    let mut c = EditorCursor::at(5);
    c.anchor = Some(2); // existing selection
    c.move_to(10, false);
    assert_eq!(c.position, 10);
    assert!(c.anchor.is_none()); // selection cleared
}

#[test]
fn move_to_with_extend() {
    let mut c = EditorCursor::at(5);
    c.move_to(10, true);
    assert_eq!(c.position, 10);
    assert_eq!(c.anchor, Some(5)); // anchor set at original position
    assert_eq!(c.selection(), Some((5, 10)));
}

#[test]
fn move_to_extend_preserves_existing_anchor() {
    let mut c = EditorCursor::at(5);
    c.anchor = Some(2); // pre-existing anchor
    c.move_to(10, true);
    assert_eq!(c.position, 10);
    assert_eq!(c.anchor, Some(2)); // anchor unchanged
    assert_eq!(c.selection(), Some((2, 10)));
}

#[test]
fn has_selection_false_when_anchor_equals_position() {
    let mut c = EditorCursor::at(5);
    c.anchor = Some(5);
    assert!(!c.has_selection());
}
