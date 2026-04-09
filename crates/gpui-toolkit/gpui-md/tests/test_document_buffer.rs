use gpui_md::document::DocumentBuffer;

#[test]
fn new_buffer_is_empty() {
    let buf = DocumentBuffer::new();
    assert_eq!(buf.text(), "");
    assert_eq!(buf.len_chars(), 0);
    assert_eq!(buf.len_lines(), 1); // ropey always has at least 1 line
    assert!(!buf.is_dirty());
}

#[test]
fn from_text() {
    let buf = DocumentBuffer::from_text("hello");
    assert_eq!(buf.text(), "hello");
    assert_eq!(buf.len_chars(), 5);
    assert!(!buf.is_dirty());
}

#[test]
fn insert_at_start() {
    let mut buf = DocumentBuffer::new();
    buf.insert(0, "abc");
    assert_eq!(buf.text(), "abc");
    assert!(buf.is_dirty());
}

#[test]
fn insert_at_end() {
    let mut buf = DocumentBuffer::from_text("hello");
    buf.insert(5, " world");
    assert_eq!(buf.text(), "hello world");
}

#[test]
fn insert_in_middle() {
    let mut buf = DocumentBuffer::from_text("helo");
    buf.insert(2, "l");
    assert_eq!(buf.text(), "hello");
}

#[test]
fn remove_range() {
    let mut buf = DocumentBuffer::from_text("hello world");
    buf.remove(5, 11);
    assert_eq!(buf.text(), "hello");
}

#[test]
fn remove_empty_range_is_noop() {
    let mut buf = DocumentBuffer::from_text("hello");
    buf.remove(2, 2);
    assert_eq!(buf.text(), "hello");
    assert!(!buf.is_dirty()); // no change
}

#[test]
fn remove_single_char() {
    let mut buf = DocumentBuffer::from_text("hello");
    buf.remove(1, 2);
    assert_eq!(buf.text(), "hllo");
}

#[test]
fn set_text_replaces_all() {
    let mut buf = DocumentBuffer::from_text("old");
    buf.set_text("new content");
    assert_eq!(buf.text(), "new content");
    assert!(!buf.is_dirty()); // set_text marks clean
}

#[test]
fn version_increments_on_insert() {
    let mut buf = DocumentBuffer::new();
    let v0 = buf.version();
    buf.insert(0, "a");
    let v1 = buf.version();
    assert!(v1 > v0);
}

#[test]
fn version_increments_on_remove() {
    let mut buf = DocumentBuffer::from_text("abc");
    let v0 = buf.version();
    buf.remove(0, 1);
    let v1 = buf.version();
    assert!(v1 > v0);
}

#[test]
fn mark_clean() {
    let mut buf = DocumentBuffer::new();
    buf.insert(0, "dirty");
    assert!(buf.is_dirty());
    buf.mark_clean();
    assert!(!buf.is_dirty());
}

#[test]
fn snapshot_and_restore() {
    let mut buf = DocumentBuffer::from_text("original");
    let snap = buf.snapshot();
    buf.insert(8, " modified");
    assert_eq!(buf.text(), "original modified");
    buf.restore(snap);
    assert_eq!(buf.text(), "original");
}

#[test]
fn line_count() {
    let buf = DocumentBuffer::from_text("line1\nline2\nline3");
    assert_eq!(buf.len_lines(), 3);
}

#[test]
fn char_to_line() {
    let buf = DocumentBuffer::from_text("abc\ndef\nghi");
    assert_eq!(buf.char_to_line(0), 0); // 'a'
    assert_eq!(buf.char_to_line(3), 0); // '\n'
    assert_eq!(buf.char_to_line(4), 1); // 'd'
    assert_eq!(buf.char_to_line(8), 2); // 'g'
}

#[test]
fn line_to_char() {
    let buf = DocumentBuffer::from_text("abc\ndef\nghi");
    assert_eq!(buf.line_to_char(0), 0);
    assert_eq!(buf.line_to_char(1), 4);
    assert_eq!(buf.line_to_char(2), 8);
}

#[test]
fn unicode_insert() {
    let mut buf = DocumentBuffer::new();
    buf.insert(0, "café");
    assert_eq!(buf.text(), "café");
    assert_eq!(buf.len_chars(), 4);
    buf.insert(4, " résumé");
    assert_eq!(buf.text(), "café résumé");
}

#[test]
fn unicode_remove() {
    let mut buf = DocumentBuffer::from_text("café");
    buf.remove(3, 4); // remove 'é'
    assert_eq!(buf.text(), "caf");
}

#[test]
fn emoji_handling() {
    let mut buf = DocumentBuffer::from_text("hello 🌍 world");
    assert_eq!(buf.len_chars(), 13);
    buf.remove(6, 7); // remove emoji
    assert_eq!(buf.text(), "hello  world");
}

#[test]
fn file_path_management() {
    let mut buf = DocumentBuffer::new();
    assert!(buf.file_path().is_none());
    buf.set_file_path(std::path::PathBuf::from("/tmp/test.md"));
    assert_eq!(
        buf.file_path().unwrap().to_str().unwrap(),
        "/tmp/test.md"
    );
}
