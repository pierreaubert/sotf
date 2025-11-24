// ============================================================================
// Toast Message System Tests
// ============================================================================
//
// Tests for the enhanced toast notification system including:
// - Message creation with different types
// - Auto-dismiss functionality
// - Persistent messages
// - Type-based styling

use sotf_audio_player_gpui::app::{ToastMessage, ToastType};
use std::thread;
use std::time::Duration;

#[test]
fn test_toast_message_success_creation() {
    let toast = ToastMessage::success("Operation completed");

    assert_eq!(toast.message, "Operation completed");
    assert_eq!(toast.toast_type, ToastType::Success);
    assert_eq!(toast.auto_dismiss_ms, Some(5000));
    assert!(!toast.should_dismiss()); // Just created, shouldn't dismiss yet
}

#[test]
fn test_toast_message_error_creation() {
    let toast = ToastMessage::error("Operation failed");

    assert_eq!(toast.message, "Operation failed");
    assert_eq!(toast.toast_type, ToastType::Error);
    assert_eq!(toast.auto_dismiss_ms, Some(5000));
}

#[test]
fn test_toast_message_info_creation() {
    let toast = ToastMessage::info("Please wait...");

    assert_eq!(toast.message, "Please wait...");
    assert_eq!(toast.toast_type, ToastType::Info);
    assert_eq!(toast.auto_dismiss_ms, Some(5000));
}

#[test]
fn test_toast_message_warning_creation() {
    let toast = ToastMessage::warning("This might take a while");

    assert_eq!(toast.message, "This might take a while");
    assert_eq!(toast.toast_type, ToastType::Warning);
    assert_eq!(toast.auto_dismiss_ms, Some(5000));
}

#[test]
fn test_toast_message_persistent() {
    let toast = ToastMessage::persistent("Scanning library...", ToastType::Info);

    assert_eq!(toast.message, "Scanning library...");
    assert_eq!(toast.toast_type, ToastType::Info);
    assert_eq!(toast.auto_dismiss_ms, None); // No auto-dismiss
    assert!(!toast.should_dismiss()); // Persistent messages never auto-dismiss
}

#[test]
fn test_toast_message_auto_dismiss() {
    let toast = ToastMessage::new("Test message".to_string(), ToastType::Info);

    // Just created, should not dismiss
    assert!(!toast.should_dismiss());

    // Wait a bit but not long enough
    thread::sleep(Duration::from_millis(100));
    assert!(!toast.should_dismiss());
}

#[test]
fn test_toast_message_string_conversion() {
    // Test that Into<String> works correctly
    let toast1 = ToastMessage::success("test");
    assert_eq!(toast1.message, "test");

    let toast2 = ToastMessage::error(String::from("error message"));
    assert_eq!(toast2.message, "error message");

    let toast3 = ToastMessage::info(format!("Loaded {} files", 42));
    assert_eq!(toast3.message, "Loaded 42 files");
}

#[test]
fn test_toast_types_equality() {
    assert_eq!(ToastType::Success, ToastType::Success);
    assert_eq!(ToastType::Error, ToastType::Error);
    assert_eq!(ToastType::Info, ToastType::Info);
    assert_eq!(ToastType::Warning, ToastType::Warning);

    assert_ne!(ToastType::Success, ToastType::Error);
    assert_ne!(ToastType::Info, ToastType::Warning);
}

#[test]
fn test_toast_message_clone() {
    let toast1 = ToastMessage::success("Original message");
    let toast2 = toast1.clone();

    assert_eq!(toast1.message, toast2.message);
    assert_eq!(toast1.toast_type, toast2.toast_type);
    assert_eq!(toast1.auto_dismiss_ms, toast2.auto_dismiss_ms);
}
