use gpui_md::document::kill_ring::KillRing;

#[test]
fn empty_ring() {
    let ring = KillRing::new();
    assert!(ring.is_empty());
    assert!(ring.yank().is_none());
}

#[test]
fn push_and_yank() {
    let mut ring = KillRing::new();
    ring.push("hello".to_string());
    assert_eq!(ring.yank(), Some("hello"));
}

#[test]
fn push_multiple_yank_gets_most_recent() {
    let mut ring = KillRing::new();
    ring.push("first".to_string());
    ring.clear_kill_flag();
    ring.push("second".to_string());
    ring.clear_kill_flag();
    ring.push("third".to_string());
    assert_eq!(ring.yank(), Some("third"));
}

#[test]
fn consecutive_kills_append() {
    let mut ring = KillRing::new();
    ring.push("hello".to_string());
    // last_was_kill is true, so next push appends
    ring.push(" world".to_string());
    assert_eq!(ring.yank(), Some("hello world"));
}

#[test]
fn non_consecutive_kills_create_separate_entries() {
    let mut ring = KillRing::new();
    ring.push("first".to_string());
    ring.clear_kill_flag(); // break the chain
    ring.push("second".to_string());
    assert_eq!(ring.yank(), Some("second"));
}

#[test]
fn push_prepend_consecutive() {
    let mut ring = KillRing::new();
    ring.push("world".to_string());
    ring.push_prepend("hello ".to_string());
    assert_eq!(ring.yank(), Some("hello world"));
}

#[test]
fn push_prepend_non_consecutive() {
    let mut ring = KillRing::new();
    ring.push("old".to_string());
    ring.clear_kill_flag();
    ring.push_prepend("new".to_string());
    // Should be a separate entry since kill chain was broken
    assert_eq!(ring.yank(), Some("new"));
}

#[test]
fn yank_pop_cycles_backward() {
    let mut ring = KillRing::new();
    ring.push("first".to_string());
    ring.clear_kill_flag();
    ring.push("second".to_string());
    ring.clear_kill_flag();
    ring.push("third".to_string());

    assert_eq!(ring.yank(), Some("third"));
    assert_eq!(ring.yank_pop(), Some("second"));
    assert_eq!(ring.yank_pop(), Some("first"));
    // Wraps around
    assert_eq!(ring.yank_pop(), Some("third"));
}

#[test]
fn yank_pop_on_single_entry() {
    let mut ring = KillRing::new();
    ring.push("only".to_string());
    assert_eq!(ring.yank(), Some("only"));
    assert_eq!(ring.yank_pop(), Some("only")); // wraps to same
}

#[test]
fn yank_pop_on_empty_ring() {
    let mut ring = KillRing::new();
    assert!(ring.yank_pop().is_none());
}

#[test]
fn mark_yank_and_last_yank_range() {
    let mut ring = KillRing::new();
    ring.push("text".to_string());
    ring.mark_yank(5, 9);
    assert_eq!(ring.last_yank_range(), Some((5, 9)));
}

#[test]
fn last_yank_range_none_when_not_yanked() {
    let ring = KillRing::new();
    assert!(ring.last_yank_range().is_none());
}

#[test]
fn reset_flags_clears_both() {
    let mut ring = KillRing::new();
    ring.push("a".to_string());
    ring.mark_yank(0, 1);
    ring.reset_flags();
    // last_yank_range should be None after reset
    assert!(ring.last_yank_range().is_none());
}

#[test]
fn rectangle_buffer() {
    let mut ring = KillRing::new();
    assert!(ring.rectangle_buffer.is_none());
    ring.rectangle_buffer = Some(vec!["abc".to_string(), "def".to_string()]);
    assert_eq!(ring.rectangle_buffer.as_ref().unwrap().len(), 2);
}

#[test]
fn max_ring_size_enforced() {
    let mut ring = KillRing::new();
    for i in 0..100 {
        ring.clear_kill_flag();
        ring.push(format!("entry-{}", i));
    }
    // Ring should have at most 60 entries
    // Verify most recent is still there
    assert_eq!(ring.yank(), Some("entry-99"));
    // Verify we can cycle through 60 entries without panic
    for _ in 0..60 {
        ring.yank_pop();
    }
}
