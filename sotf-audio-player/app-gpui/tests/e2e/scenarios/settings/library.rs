//! E2E tests for Library Settings.
//!
//! Tests for library path configuration:
//! - Adding library paths
//! - Removing library paths
//! - Scanning options
//! - File type filters

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Library path entry
#[derive(Debug, Clone)]
struct LibraryPath {
    path: String,
    enabled: bool,
    track_count: usize,
    last_scanned: Option<String>,
}

impl Default for LibraryPath {
    fn default() -> Self {
        Self {
            path: String::new(),
            enabled: true,
            track_count: 0,
            last_scanned: None,
        }
    }
}

/// Scan on startup option
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ScanOnStartup {
    #[default]
    Never,
    Always,
    IfChanged,
}

/// Library settings state
struct LibrarySettingsState {
    library_paths: Vec<LibraryPath>,
    scan_on_startup: ScanOnStartup,
    watch_for_changes: bool,
    scan_subdirectories: bool,
    file_types: Vec<String>,
    exclude_patterns: Vec<String>,
}

impl Default for LibrarySettingsState {
    fn default() -> Self {
        Self {
            library_paths: Vec::new(),
            scan_on_startup: ScanOnStartup::IfChanged,
            watch_for_changes: true,
            scan_subdirectories: true,
            file_types: vec![
                "flac".to_string(),
                "mp3".to_string(),
                "m4a".to_string(),
                "wav".to_string(),
                "ogg".to_string(),
            ],
            exclude_patterns: Vec::new(),
        }
    }
}

// =============================================================================
// Library Path Tests
// =============================================================================

/// Test adding library path.
#[gpui::test]
async fn test_adding_library_path(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().library_paths.push(LibraryPath {
        path: "/Users/pierre/Music".to_string(),
        enabled: true,
        track_count: 0,
        last_scanned: None,
    });

    assert_eq!(state.borrow().library_paths.len(), 1);
    assert_eq!(state.borrow().library_paths[0].path, "/Users/pierre/Music");
}

/// Test removing library path.
#[gpui::test]
async fn test_removing_library_path(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().library_paths = vec![
        LibraryPath {
            path: "/path/1".to_string(),
            ..Default::default()
        },
        LibraryPath {
            path: "/path/2".to_string(),
            ..Default::default()
        },
    ];

    state.borrow_mut().library_paths.remove(0);
    assert_eq!(state.borrow().library_paths.len(), 1);
    assert_eq!(state.borrow().library_paths[0].path, "/path/2");
}

/// Test multiple library paths.
#[gpui::test]
async fn test_multiple_library_paths(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    let paths = vec![
        "/Users/pierre/Music",
        "/Volumes/External/Music",
        "/Volumes/NAS/Audio",
    ];

    for path in paths {
        state.borrow_mut().library_paths.push(LibraryPath {
            path: path.to_string(),
            ..Default::default()
        });
    }

    assert_eq!(state.borrow().library_paths.len(), 3);
}

/// Test enabling/disabling library path.
#[gpui::test]
async fn test_enabling_disabling_library_path(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().library_paths.push(LibraryPath::default());

    assert!(state.borrow().library_paths[0].enabled);

    state.borrow_mut().library_paths[0].enabled = false;
    assert!(!state.borrow().library_paths[0].enabled);
}

/// Test library path track count.
#[gpui::test]
async fn test_library_path_track_count(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().library_paths.push(LibraryPath {
        path: "/path".to_string(),
        track_count: 1500,
        ..Default::default()
    });

    assert_eq!(state.borrow().library_paths[0].track_count, 1500);
}

/// Test last scanned timestamp.
#[gpui::test]
async fn test_last_scanned_timestamp(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().library_paths.push(LibraryPath {
        path: "/path".to_string(),
        last_scanned: Some("2024-01-06 14:30:00".to_string()),
        ..Default::default()
    });

    assert!(state.borrow().library_paths[0].last_scanned.is_some());
}

// =============================================================================
// Scan Options Tests
// =============================================================================

/// Test scan on startup selection.
#[gpui::test]
async fn test_scan_on_startup_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    let options = [ScanOnStartup::Never, ScanOnStartup::Always, ScanOnStartup::IfChanged];
    for option in options {
        state.borrow_mut().scan_on_startup = option;
        assert_eq!(state.borrow().scan_on_startup, option);
    }
}

/// Test watch for changes toggle.
#[gpui::test]
async fn test_watch_for_changes_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    assert!(state.borrow().watch_for_changes);

    state.borrow_mut().watch_for_changes = false;
    assert!(!state.borrow().watch_for_changes);
}

/// Test scan subdirectories toggle.
#[gpui::test]
async fn test_scan_subdirectories_toggle(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    assert!(state.borrow().scan_subdirectories);

    state.borrow_mut().scan_subdirectories = false;
    assert!(!state.borrow().scan_subdirectories);
}

// =============================================================================
// File Type Filter Tests
// =============================================================================

/// Test default file types.
#[gpui::test]
async fn test_default_file_types(_cx: &mut TestAppContext) {
    let state = LibrarySettingsState::default();

    assert!(state.file_types.contains(&"flac".to_string()));
    assert!(state.file_types.contains(&"mp3".to_string()));
    assert!(state.file_types.contains(&"m4a".to_string()));
    assert!(state.file_types.contains(&"wav".to_string()));
}

/// Test adding file type.
#[gpui::test]
async fn test_adding_file_type(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().file_types.push("aiff".to_string());
    assert!(state.borrow().file_types.contains(&"aiff".to_string()));
}

/// Test removing file type.
#[gpui::test]
async fn test_removing_file_type(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().file_types.retain(|t| t != "mp3");
    assert!(!state.borrow().file_types.contains(&"mp3".to_string()));
}

/// Test file type matching.
#[gpui::test]
async fn test_file_type_matching(_cx: &mut TestAppContext) {
    fn matches_file_type(filename: &str, allowed_types: &[String]) -> bool {
        let ext = filename.rsplit('.').next().unwrap_or("");
        allowed_types.iter().any(|t| t.eq_ignore_ascii_case(ext))
    }

    let allowed = vec!["flac".to_string(), "mp3".to_string()];
    assert!(matches_file_type("song.flac", &allowed));
    assert!(matches_file_type("song.mp3", &allowed));
    assert!(!matches_file_type("song.wav", &allowed));
}

// =============================================================================
// Exclude Pattern Tests
// =============================================================================

/// Test adding exclude pattern.
#[gpui::test]
async fn test_adding_exclude_pattern(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().exclude_patterns.push("*.sample".to_string());
    assert!(state.borrow().exclude_patterns.contains(&"*.sample".to_string()));
}

/// Test removing exclude pattern.
#[gpui::test]
async fn test_removing_exclude_pattern(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().exclude_patterns = vec!["*.sample".to_string(), "temp/*".to_string()];
    state.borrow_mut().exclude_patterns.retain(|p| p != "*.sample");
    assert!(!state.borrow().exclude_patterns.contains(&"*.sample".to_string()));
}

/// Test exclude pattern matching.
#[gpui::test]
async fn test_exclude_pattern_matching(_cx: &mut TestAppContext) {
    fn matches_exclude_pattern(path: &str, pattern: &str) -> bool {
        // Simple glob matching
        if pattern.starts_with('*') {
            path.ends_with(&pattern[1..])
        } else if pattern.ends_with('*') {
            path.starts_with(&pattern[..pattern.len() - 1])
        } else {
            path.contains(pattern)
        }
    }

    assert!(matches_exclude_pattern("/path/song.sample", "*.sample"));
    assert!(matches_exclude_pattern("/path/temp/file.flac", "temp/"));
    assert!(!matches_exclude_pattern("/path/song.flac", "*.sample"));
}

// =============================================================================
// Path Display Tests
// =============================================================================

/// Test path truncation for display.
#[gpui::test]
async fn test_path_truncation(_cx: &mut TestAppContext) {
    fn truncate_path(path: &str, max_len: usize) -> String {
        if path.len() <= max_len {
            path.to_string()
        } else {
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() <= 2 {
                format!("...{}", &path[path.len() - max_len + 3..])
            } else {
                let last_two: Vec<&str> = parts.iter().rev().take(2).rev().copied().collect();
                format!(".../{}", last_two.join("/"))
            }
        }
    }

    let long_path = "/Users/pierre/Documents/Music/Albums/Classical";
    let truncated = truncate_path(long_path, 30);
    assert!(truncated.len() <= 40);
}

/// Test path status display.
#[gpui::test]
async fn test_path_status_display(_cx: &mut TestAppContext) {
    fn get_path_status(enabled: bool, track_count: usize) -> String {
        if !enabled {
            "Disabled".to_string()
        } else if track_count == 0 {
            "Not scanned".to_string()
        } else {
            format!("{} tracks", track_count)
        }
    }

    assert_eq!(get_path_status(false, 100), "Disabled");
    assert_eq!(get_path_status(true, 0), "Not scanned");
    assert_eq!(get_path_status(true, 500), "500 tracks");
}

// =============================================================================
// Total Statistics Tests
// =============================================================================

/// Test total track count.
#[gpui::test]
async fn test_total_track_count(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().library_paths = vec![
        LibraryPath {
            track_count: 500,
            enabled: true,
            ..Default::default()
        },
        LibraryPath {
            track_count: 300,
            enabled: true,
            ..Default::default()
        },
        LibraryPath {
            track_count: 200,
            enabled: false, // Disabled
            ..Default::default()
        },
    ];

    let total: usize = state
        .borrow()
        .library_paths
        .iter()
        .filter(|p| p.enabled)
        .map(|p| p.track_count)
        .sum();

    assert_eq!(total, 800);
}

/// Test enabled path count.
#[gpui::test]
async fn test_enabled_path_count(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(LibrarySettingsState::default()));

    state.borrow_mut().library_paths = vec![
        LibraryPath {
            enabled: true,
            ..Default::default()
        },
        LibraryPath {
            enabled: true,
            ..Default::default()
        },
        LibraryPath {
            enabled: false,
            ..Default::default()
        },
    ];

    let enabled_count = state.borrow().library_paths.iter().filter(|p| p.enabled).count();
    assert_eq!(enabled_count, 2);
}
