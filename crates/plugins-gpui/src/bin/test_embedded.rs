//! Test binary for GPUI embedded view experiments
//!
//! This explores different approaches for using GPUI in an Audio Unit context
//! where we can't call Application::run() because the host owns the event loop.
//!
//! Note: MetalView and EmbeddedView tests are in the library unit tests.
//! Run: cargo test -p gpui-au

use gpui::*;
use std::rc::Rc;

fn mac_platform() -> Rc<dyn Platform> {
    Rc::new(gpui_macos::MacPlatform::new(false))
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();

    println!("=== GPUI Embedded View Test ===\n");

    // Test 1: Check if we can create Application without running
    println!("Test 1: Create Application without run()");
    test_app_without_run();

    // Test 2: Try background thread approach
    println!("\nTest 2: Background thread approach");
    test_background_thread();

    // Test 3: Try window creation without run()
    println!("\nTest 3: Window creation without run()");
    test_window_creation_without_run();

    // Test 4: raw_window_handle analysis
    println!("\nTest 4: raw_window_handle analysis");
    test_raw_window_handle();

    println!("\n=== Tests Complete ===");
    println!("\n=== SUMMARY ===");
    println!("  Application::with_platform() works without run()");
    println!("  GPUI requires main thread (cannot use background thread)");
    println!("? Window creation needs App context from run() callback");
    println!("\n=== MetalView and EmbeddedView ===");
    println!("Run library tests for full coverage:");
    println!("  cargo test -p gpui-au -- --nocapture");
    println!("\nRECOMMENDED APPROACH:");
    println!("Use EmbeddedView for AU integration:");
    println!("- Metal-backed NSView for embedding in host");
    println!("- GPUI text system for high-quality text rendering");
    println!("- Manual render loop synchronized with host");
}

fn test_app_without_run() {
    println!("  Creating Application::with_platform()...");
    let app = Application::with_platform(mac_platform());
    println!("  Application created successfully");

    // Try to access background executor without run()
    let _bg_executor = app.background_executor();
    println!("  Got background executor");

    let _fg_executor = app.foreground_executor();
    println!("  Got foreground executor");

    let _text_system = app.text_system();
    println!("  Got text system");

    // The app will be dropped here without running
    // This tests if GPUI cleans up properly
    println!("  Dropping Application without run()...");
    drop(app);
    println!("  Application dropped cleanly");
}

fn test_background_thread() {
    println!("  SKIPPED - GPUI requires main thread");
}

fn test_window_creation_without_run() {
    println!("  Creating Application and trying to open window without run()...");

    // Key insight: Application provides access to App context
    // but windows are normally created inside run() callback

    let app = Application::with_platform(mac_platform());
    println!("  Application created");

    // The challenge: open_window is called on App (&mut App), not Application
    // We need to somehow get access to the App context without run()

    println!("  Note: Window creation requires App context from run() callback");
    println!("  Need to explore GPUI internals or fork to expose this");

    drop(app);
    println!("  Application dropped");
}

fn test_raw_window_handle() {
    println!("  Testing raw_window_handle types...");

    // The goal is to understand what we'd get from HasWindowHandle trait
    // On macOS, it returns AppKitWindowHandle which contains NSView pointer

    println!("  raw_window_handle crate provides AppKitWindowHandle");
    println!("  MacWindow implements HasWindowHandle -> returns native_view");
    println!("  Challenge: Need to create MacWindow to extract NSView");
}
