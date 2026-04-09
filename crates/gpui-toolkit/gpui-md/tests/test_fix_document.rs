//! TDD tests for document model fixes.
//!
//! Issue #21: history/kill_ring use Vec::remove(0) which is O(n) — should use VecDeque
//! Issue #23: DocumentBuffer::remove uses debug_assert only — silent in release mode

use gpui_md::document::{DocumentBuffer, EditHistory};
use ropey::Rope;

// ---------------------------------------------------------------------------
// Issue #21: Vec::remove(0) is O(n) in history and kill ring
//
// With max_history=100 this is negligible, but correctness should be
// verified with large limits. After switching to VecDeque the behavior
// must remain identical.
// ---------------------------------------------------------------------------

#[test]
fn issue_21_large_history_retains_correct_entries() {
    let mut h = EditHistory::new(1000);
    for i in 0..2000 {
        h.push_undo(Rope::from_str(&format!("entry {}", i)), 0);
    }
    assert!(h.can_undo());

    // The oldest entries should have been evicted, newest retained
    let (snapshot, _cursor) = h.undo(Rope::from_str("current"), 0).unwrap();
    let text: String = snapshot.chars().collect();
    assert_eq!(
        text, "entry 1999",
        "most recent entry should be on top of the undo stack"
    );
}

#[test]
fn issue_21_history_max_is_enforced_after_eviction() {
    let mut h = EditHistory::new(5);
    for i in 0..20 {
        h.push_undo(Rope::from_str(&format!("e{}", i)), 0);
    }
    // Should have at most 5 entries
    let mut count = 0;
    let mut current = Rope::from_str("now");
    while let Some((snap, _)) = h.undo(current.clone(), 0) {
        current = snap;
        count += 1;
    }
    assert!(
        count <= 5,
        "history should respect max_history limit, got {} entries",
        count
    );
}

// ---------------------------------------------------------------------------
// Issue #23: DocumentBuffer::remove uses debug_assert only
//
// In release mode the debug_assert is stripped and ropey panics with a
// less helpful message. The function should gracefully handle:
//   - inverted ranges (start > end)
//   - out-of-bounds indices
// ---------------------------------------------------------------------------

#[test]
fn issue_23_remove_with_inverted_range_is_noop() {
    let mut buf = DocumentBuffer::from_text("hello");
    // start > end → should handle gracefully (no-op or swap), not panic
    buf.remove(3, 1);
    assert_eq!(
        buf.text(),
        "hello",
        "remove with start > end should be a no-op"
    );
}

#[test]
fn issue_23_remove_at_document_boundary_is_safe() {
    let mut buf = DocumentBuffer::from_text("hello");
    // Removing exactly [0, len_chars) should work
    buf.remove(0, 5);
    assert_eq!(buf.text(), "");
}

#[test]
fn issue_23_remove_empty_range_is_noop() {
    let mut buf = DocumentBuffer::from_text("hello");
    let v = buf.version();
    buf.remove(2, 2); // zero-width range
    assert_eq!(buf.text(), "hello");
    // Version should not increment for a no-op
    assert_eq!(buf.version(), v);
}
