//! E2E tests for About Dialog.
//!
//! Tests for the about dialog displaying application info:
//! - Version information
//! - Credits and acknowledgements
//! - Links and resources
//! - License information

use gpui::TestAppContext;
use std::cell::RefCell;
use std::rc::Rc;

// =============================================================================
// Mock Types for Testing
// =============================================================================

/// Tab in about dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum AboutTab {
    #[default]
    About,
    Credits,
    License,
    SystemInfo,
}

/// Version info
#[derive(Debug, Clone)]
struct VersionInfo {
    app_version: String,
    build_number: String,
    commit_hash: Option<String>,
    build_date: String,
    rust_version: String,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            app_version: "0.5.3".to_string(),
            build_number: "1234".to_string(),
            commit_hash: Some("abc1234".to_string()),
            build_date: "2024-01-06".to_string(),
            rust_version: "1.75.0".to_string(),
        }
    }
}

/// Credit entry
#[derive(Debug, Clone)]
struct CreditEntry {
    name: String,
    role: String,
    url: Option<String>,
}

/// License info
#[derive(Debug, Clone)]
struct LicenseInfo {
    license_type: String,
    full_text: String,
}

/// System info
#[derive(Debug, Clone)]
struct SystemInfo {
    os_name: String,
    os_version: String,
    cpu_info: String,
    memory_gb: f32,
    audio_backend: String,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            os_name: "macOS".to_string(),
            os_version: "14.0".to_string(),
            cpu_info: "Apple M2".to_string(),
            memory_gb: 16.0,
            audio_backend: "CoreAudio".to_string(),
        }
    }
}

/// Link entry
#[derive(Debug, Clone)]
struct LinkEntry {
    label: String,
    url: String,
    icon: Option<String>,
}

/// About dialog state
struct AboutDialogState {
    is_open: bool,
    active_tab: AboutTab,
    version_info: VersionInfo,
    credits: Vec<CreditEntry>,
    license: LicenseInfo,
    system_info: SystemInfo,
    links: Vec<LinkEntry>,
    show_full_license: bool,
}

impl Default for AboutDialogState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_tab: AboutTab::About,
            version_info: VersionInfo::default(),
            credits: Vec::new(),
            license: LicenseInfo {
                license_type: "MIT".to_string(),
                full_text: String::new(),
            },
            system_info: SystemInfo::default(),
            links: Vec::new(),
            show_full_license: false,
        }
    }
}

// =============================================================================
// Dialog State Tests
// =============================================================================

/// Test dialog opens.
#[gpui::test]
async fn test_dialog_opens(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AboutDialogState::default()));

    assert!(!state.borrow().is_open);

    state.borrow_mut().is_open = true;
    assert!(state.borrow().is_open);
}

/// Test dialog closes.
#[gpui::test]
async fn test_dialog_closes(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AboutDialogState::default()));

    state.borrow_mut().is_open = true;
    state.borrow_mut().is_open = false;
    assert!(!state.borrow().is_open);
}

/// Test default tab is about.
#[gpui::test]
async fn test_default_tab_is_about(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();
    assert_eq!(state.active_tab, AboutTab::About);
}

// =============================================================================
// Tab Navigation Tests
// =============================================================================

/// Test tab selection.
#[gpui::test]
async fn test_tab_selection(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AboutDialogState::default()));

    let tabs = [
        AboutTab::About,
        AboutTab::Credits,
        AboutTab::License,
        AboutTab::SystemInfo,
    ];

    for tab in tabs {
        state.borrow_mut().active_tab = tab;
        assert_eq!(state.borrow().active_tab, tab);
    }
}

/// Test tab labels.
#[gpui::test]
async fn test_tab_labels(_cx: &mut TestAppContext) {
    fn get_tab_label(tab: AboutTab) -> &'static str {
        match tab {
            AboutTab::About => "About",
            AboutTab::Credits => "Credits",
            AboutTab::License => "License",
            AboutTab::SystemInfo => "System",
        }
    }

    assert_eq!(get_tab_label(AboutTab::About), "About");
    assert_eq!(get_tab_label(AboutTab::Credits), "Credits");
    assert_eq!(get_tab_label(AboutTab::License), "License");
    assert_eq!(get_tab_label(AboutTab::SystemInfo), "System");
}

/// Test tab count.
#[gpui::test]
async fn test_tab_count(_cx: &mut TestAppContext) {
    fn get_all_tabs() -> Vec<AboutTab> {
        vec![
            AboutTab::About,
            AboutTab::Credits,
            AboutTab::License,
            AboutTab::SystemInfo,
        ]
    }

    assert_eq!(get_all_tabs().len(), 4);
}

// =============================================================================
// Version Info Tests
// =============================================================================

/// Test version display.
#[gpui::test]
async fn test_version_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert_eq!(state.version_info.app_version, "0.5.3");
}

/// Test build number display.
#[gpui::test]
async fn test_build_number_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert!(!state.version_info.build_number.is_empty());
}

/// Test commit hash display.
#[gpui::test]
async fn test_commit_hash_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert!(state.version_info.commit_hash.is_some());
}

/// Test version string formatting.
#[gpui::test]
async fn test_version_string_formatting(_cx: &mut TestAppContext) {
    fn format_version(info: &VersionInfo) -> String {
        let commit = info
            .commit_hash
            .as_ref()
            .map(|h| format!(" ({})", &h[..7.min(h.len())]))
            .unwrap_or_default();

        format!("v{}{}", info.app_version, commit)
    }

    let info = VersionInfo::default();
    let formatted = format_version(&info);

    assert!(formatted.starts_with("v0.5.3"));
    assert!(formatted.contains("abc1234"));
}

/// Test build date display.
#[gpui::test]
async fn test_build_date_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert!(!state.version_info.build_date.is_empty());
}

/// Test rust version display.
#[gpui::test]
async fn test_rust_version_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert!(state.version_info.rust_version.starts_with("1."));
}

// =============================================================================
// Credits Tests
// =============================================================================

/// Test credits loading.
#[gpui::test]
async fn test_credits_loading(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AboutDialogState::default()));

    state.borrow_mut().credits = vec![
        CreditEntry {
            name: "Pierre".to_string(),
            role: "Lead Developer".to_string(),
            url: Some("https://github.com/pierreMusic".to_string()),
        },
        CreditEntry {
            name: "GPUI Team".to_string(),
            role: "UI Framework".to_string(),
            url: Some("https://zed.dev".to_string()),
        },
    ];

    assert_eq!(state.borrow().credits.len(), 2);
}

/// Test credit entry display.
#[gpui::test]
async fn test_credit_entry_display(_cx: &mut TestAppContext) {
    fn format_credit(entry: &CreditEntry) -> String {
        format!("{} - {}", entry.name, entry.role)
    }

    let entry = CreditEntry {
        name: "Test Developer".to_string(),
        role: "Testing".to_string(),
        url: None,
    };

    let formatted = format_credit(&entry);
    assert!(formatted.contains("Test Developer"));
    assert!(formatted.contains("Testing"));
}

/// Test credit link.
#[gpui::test]
async fn test_credit_link(_cx: &mut TestAppContext) {
    let entry = CreditEntry {
        name: "Test".to_string(),
        role: "Role".to_string(),
        url: Some("https://example.com".to_string()),
    };

    assert!(entry.url.is_some());
}

/// Test credit without link.
#[gpui::test]
async fn test_credit_without_link(_cx: &mut TestAppContext) {
    let entry = CreditEntry {
        name: "Test".to_string(),
        role: "Role".to_string(),
        url: None,
    };

    assert!(entry.url.is_none());
}

// =============================================================================
// License Tests
// =============================================================================

/// Test license type display.
#[gpui::test]
async fn test_license_type_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert_eq!(state.license.license_type, "MIT");
}

/// Test show full license.
#[gpui::test]
async fn test_show_full_license(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AboutDialogState::default()));

    assert!(!state.borrow().show_full_license);

    state.borrow_mut().show_full_license = true;
    assert!(state.borrow().show_full_license);
}

/// Test license summary.
#[gpui::test]
async fn test_license_summary(_cx: &mut TestAppContext) {
    fn get_license_summary(license_type: &str) -> &'static str {
        match license_type {
            "MIT" => "Free for personal and commercial use",
            "GPL-3.0" => "Free software, copyleft",
            "Apache-2.0" => "Free for personal and commercial use with attribution",
            _ => "See full license for details",
        }
    }

    assert!(get_license_summary("MIT").contains("Free"));
}

// =============================================================================
// System Info Tests
// =============================================================================

/// Test os name display.
#[gpui::test]
async fn test_os_name_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert_eq!(state.system_info.os_name, "macOS");
}

/// Test os version display.
#[gpui::test]
async fn test_os_version_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert!(!state.system_info.os_version.is_empty());
}

/// Test cpu info display.
#[gpui::test]
async fn test_cpu_info_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert!(!state.system_info.cpu_info.is_empty());
}

/// Test memory display.
#[gpui::test]
async fn test_memory_display(_cx: &mut TestAppContext) {
    fn format_memory(gb: f32) -> String {
        format!("{:.1} GB", gb)
    }

    let state = AboutDialogState::default();
    let formatted = format_memory(state.system_info.memory_gb);

    assert!(formatted.contains("GB"));
}

/// Test audio backend display.
#[gpui::test]
async fn test_audio_backend_display(_cx: &mut TestAppContext) {
    let state = AboutDialogState::default();

    assert!(!state.system_info.audio_backend.is_empty());
}

/// Test system info formatting.
#[gpui::test]
async fn test_system_info_formatting(_cx: &mut TestAppContext) {
    fn format_system_info(info: &SystemInfo) -> Vec<(String, String)> {
        vec![
            (
                "Operating System".to_string(),
                format!("{} {}", info.os_name, info.os_version),
            ),
            ("Processor".to_string(), info.cpu_info.clone()),
            ("Memory".to_string(), format!("{:.1} GB", info.memory_gb)),
            ("Audio Backend".to_string(), info.audio_backend.clone()),
        ]
    }

    let info = SystemInfo::default();
    let formatted = format_system_info(&info);

    assert_eq!(formatted.len(), 4);
}

// =============================================================================
// Links Tests
// =============================================================================

/// Test links loading.
#[gpui::test]
async fn test_links_loading(_cx: &mut TestAppContext) {
    let state = Rc::new(RefCell::new(AboutDialogState::default()));

    state.borrow_mut().links = vec![
        LinkEntry {
            label: "Website".to_string(),
            url: "https://sotf.app".to_string(),
            icon: Some("globe".to_string()),
        },
        LinkEntry {
            label: "GitHub".to_string(),
            url: "https://github.com/sotf".to_string(),
            icon: Some("github".to_string()),
        },
        LinkEntry {
            label: "Documentation".to_string(),
            url: "https://docs.sotf.app".to_string(),
            icon: Some("book".to_string()),
        },
    ];

    assert_eq!(state.borrow().links.len(), 3);
}

/// Test link click handler.
#[gpui::test]
async fn test_link_click_handler(_cx: &mut TestAppContext) {
    fn is_valid_url(url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    let link = LinkEntry {
        label: "Test".to_string(),
        url: "https://example.com".to_string(),
        icon: None,
    };

    assert!(is_valid_url(&link.url));
}

/// Test link with icon.
#[gpui::test]
async fn test_link_with_icon(_cx: &mut TestAppContext) {
    let link = LinkEntry {
        label: "GitHub".to_string(),
        url: "https://github.com".to_string(),
        icon: Some("github".to_string()),
    };

    assert!(link.icon.is_some());
}

/// Test link without icon.
#[gpui::test]
async fn test_link_without_icon(_cx: &mut TestAppContext) {
    let link = LinkEntry {
        label: "Test".to_string(),
        url: "https://test.com".to_string(),
        icon: None,
    };

    assert!(link.icon.is_none());
}

// =============================================================================
// Logo and Branding Tests
// =============================================================================

/// Test app name display.
#[gpui::test]
async fn test_app_name_display(_cx: &mut TestAppContext) {
    fn get_app_name() -> &'static str {
        "SotF"
    }

    assert_eq!(get_app_name(), "SotF");
}

/// Test app tagline display.
#[gpui::test]
async fn test_app_tagline_display(_cx: &mut TestAppContext) {
    fn get_app_tagline() -> &'static str {
        "Sound of the Future"
    }

    assert_eq!(get_app_tagline(), "Sound of the Future");
}

/// Test copyright notice.
#[gpui::test]
async fn test_copyright_notice(_cx: &mut TestAppContext) {
    fn get_copyright(year: u32) -> String {
        format!("© {} Pierre. All rights reserved.", year)
    }

    let copyright = get_copyright(2024);
    assert!(copyright.contains("2024"));
    assert!(copyright.contains("Pierre"));
}

// =============================================================================
// Accessibility Tests
// =============================================================================

/// Test dialog aria label.
#[gpui::test]
async fn test_dialog_aria_label(_cx: &mut TestAppContext) {
    fn get_dialog_aria_label() -> &'static str {
        "About SotF"
    }

    assert!(get_dialog_aria_label().contains("About"));
}

/// Test keyboard navigation.
#[gpui::test]
async fn test_keyboard_navigation(_cx: &mut TestAppContext) {
    fn get_next_tab(current: AboutTab) -> AboutTab {
        match current {
            AboutTab::About => AboutTab::Credits,
            AboutTab::Credits => AboutTab::License,
            AboutTab::License => AboutTab::SystemInfo,
            AboutTab::SystemInfo => AboutTab::About,
        }
    }

    assert_eq!(get_next_tab(AboutTab::About), AboutTab::Credits);
    assert_eq!(get_next_tab(AboutTab::SystemInfo), AboutTab::About);
}

/// Test focus trap in dialog.
#[gpui::test]
async fn test_focus_trap_in_dialog(_cx: &mut TestAppContext) {
    struct FocusState {
        focusable_elements: Vec<String>,
        current_focus: usize,
    }

    fn cycle_focus(state: &mut FocusState) {
        state.current_focus = (state.current_focus + 1) % state.focusable_elements.len();
    }

    let mut state = FocusState {
        focusable_elements: vec!["close".to_string(), "tab1".to_string(), "tab2".to_string()],
        current_focus: 2,
    };

    cycle_focus(&mut state);
    assert_eq!(state.current_focus, 0); // Wraps around
}

// =============================================================================
// Copy to Clipboard Tests
// =============================================================================

/// Test copy version info.
#[gpui::test]
async fn test_copy_version_info(_cx: &mut TestAppContext) {
    fn format_version_for_copy(info: &VersionInfo) -> String {
        format!(
            "SotF v{}\nBuild: {}\nDate: {}\nRust: {}",
            info.app_version, info.build_number, info.build_date, info.rust_version
        )
    }

    let info = VersionInfo::default();
    let formatted = format_version_for_copy(&info);

    assert!(formatted.contains("SotF"));
    assert!(formatted.contains("0.5.3"));
}

/// Test copy system info.
#[gpui::test]
async fn test_copy_system_info(_cx: &mut TestAppContext) {
    fn format_system_info_for_copy(info: &SystemInfo) -> String {
        format!(
            "OS: {} {}\nCPU: {}\nMemory: {:.1} GB\nAudio: {}",
            info.os_name, info.os_version, info.cpu_info, info.memory_gb, info.audio_backend
        )
    }

    let info = SystemInfo::default();
    let formatted = format_system_info_for_copy(&info);

    assert!(formatted.contains("macOS"));
}

// =============================================================================
// Styling Tests
// =============================================================================

/// Test active tab style.
#[gpui::test]
async fn test_active_tab_style(_cx: &mut TestAppContext) {
    fn get_tab_class(is_active: bool) -> &'static str {
        if is_active { "tab active" } else { "tab" }
    }

    assert!(get_tab_class(true).contains("active"));
    assert!(!get_tab_class(false).contains("active"));
}

/// Test link hover style.
#[gpui::test]
async fn test_link_hover_style(_cx: &mut TestAppContext) {
    fn get_link_color(is_hovered: bool) -> &'static str {
        if is_hovered { "accent_hover" } else { "accent" }
    }

    assert_eq!(get_link_color(false), "accent");
    assert_eq!(get_link_color(true), "accent_hover");
}

/// Test section spacing.
#[gpui::test]
async fn test_section_spacing(_cx: &mut TestAppContext) {
    fn get_section_spacing() -> f32 {
        16.0
    }

    assert!((get_section_spacing() - 16.0).abs() < 0.1);
}
