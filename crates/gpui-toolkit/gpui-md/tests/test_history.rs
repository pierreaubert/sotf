use gpui_md::document::EditHistory;
use ropey::Rope;

#[test]
fn empty_history() {
    let h = EditHistory::default();
    assert!(!h.can_undo());
    assert!(!h.can_redo());
}

#[test]
fn undo_single() {
    let mut h = EditHistory::default();
    let snap = Rope::from_str("original");
    h.push_undo(snap, 0);
    assert!(h.can_undo());

    let current = Rope::from_str("modified");
    let (restored, _cursor) = h.undo(current, 0).unwrap();
    assert_eq!(restored.to_string(), "original");
    assert!(!h.can_undo());
}

#[test]
fn redo_after_undo() {
    let mut h = EditHistory::default();
    h.push_undo(Rope::from_str("v1"), 0);
    let (restored, _) = h.undo(Rope::from_str("v2"), 0).unwrap();
    assert_eq!(restored.to_string(), "v1");
    assert!(h.can_redo());

    let (re_restored, _) = h.redo(Rope::from_str("v1"), 0).unwrap();
    assert_eq!(re_restored.to_string(), "v2");
    assert!(!h.can_redo());
}

#[test]
fn push_clears_redo() {
    let mut h = EditHistory::default();
    h.push_undo(Rope::from_str("v1"), 0);
    h.undo(Rope::from_str("v2"), 0);
    assert!(h.can_redo());

    h.push_undo(Rope::from_str("v3"), 0); // new edit clears redo stack
    assert!(!h.can_redo());
}

#[test]
fn multiple_undos() {
    let mut h = EditHistory::default();
    h.push_undo(Rope::from_str("v1"), 0);
    h.push_undo(Rope::from_str("v2"), 0);
    h.push_undo(Rope::from_str("v3"), 0);

    let (r, _) = h.undo(Rope::from_str("v4"), 0).unwrap();
    assert_eq!(r.to_string(), "v3");
    let (r, _) = h.undo(r, 0).unwrap();
    assert_eq!(r.to_string(), "v2");
    let (r, _) = h.undo(r, 0).unwrap();
    assert_eq!(r.to_string(), "v1");
    assert!(!h.can_undo());
}

#[test]
fn undo_on_empty_returns_none() {
    let mut h = EditHistory::default();
    assert!(h.undo(Rope::from_str("current"), 0).is_none());
}

#[test]
fn redo_on_empty_returns_none() {
    let mut h = EditHistory::default();
    assert!(h.redo(Rope::from_str("current"), 0).is_none());
}

#[test]
fn max_history_respected() {
    let mut h = EditHistory::new(3);
    h.push_undo(Rope::from_str("v1"), 0);
    h.push_undo(Rope::from_str("v2"), 0);
    h.push_undo(Rope::from_str("v3"), 0);
    h.push_undo(Rope::from_str("v4"), 0); // should evict v1

    let (r, _) = h.undo(Rope::from_str("v5"), 0).unwrap();
    assert_eq!(r.to_string(), "v4");
    let (r, _) = h.undo(r, 0).unwrap();
    assert_eq!(r.to_string(), "v3");
    let (r, _) = h.undo(r, 0).unwrap();
    assert_eq!(r.to_string(), "v2");
    assert!(!h.can_undo()); // v1 was evicted
}

#[test]
fn undo_restores_cursor_position() {
    let mut h = EditHistory::default();
    h.push_undo(Rope::from_str("hello"), 2);

    let (rope, cursor) = h.undo(Rope::from_str("heXllo"), 3).unwrap();
    assert_eq!(rope.to_string(), "hello");
    assert_eq!(cursor, 2, "undo should return the stored cursor position");
}

#[test]
fn redo_restores_cursor_position() {
    let mut h = EditHistory::default();
    h.push_undo(Rope::from_str("hello"), 2);

    // Undo pushes current state (heXllo, cursor=3) to redo
    let _ = h.undo(Rope::from_str("heXllo"), 3).unwrap();

    // Redo should return the state that was pushed by undo
    let (rope, cursor) = h.redo(Rope::from_str("hello"), 2).unwrap();
    assert_eq!(rope.to_string(), "heXllo");
    assert_eq!(cursor, 3, "redo should return the cursor from when undo was called");
}
