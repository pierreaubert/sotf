use std::path::Path;

const IOS_APP_DIR: &str = "SotFPlayer";

fn ios_path(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("ios")
        .join(relative)
}

fn app_path(relative: &str) -> std::path::PathBuf {
    ios_path(&format!("{IOS_APP_DIR}/{relative}"))
}

#[test]
fn ios_project_builds_rust_library_instead_of_requiring_committed_archive() {
    let project_yml = std::fs::read_to_string(ios_path("project.yml")).unwrap();
    let gitignore = std::fs::read_to_string(ios_path(".gitignore")).unwrap();
    assert!(project_yml.contains("preBuildScripts:"));
    assert!(project_yml.contains("./build-rust.sh"));
    assert!(project_yml.contains("$(DERIVED_FILE_DIR)/rust/libsotf_ios.a"));
    assert!(!project_yml.contains("$(PROJECT_DIR)/lib/libsotf_ios.a"));
    assert!(gitignore.lines().any(|line| line.trim() == "lib/"));
}

#[test]
fn ios_bundle_metadata_has_required_shipping_assets() {
    let plist = std::fs::read_to_string(app_path("Info.plist")).unwrap();
    assert!(plist.contains("<string>$(MARKETING_VERSION)</string>"));
    assert!(plist.contains("<string>$(CURRENT_PROJECT_VERSION)</string>"));
    assert!(plist.contains("<string>LaunchScreen</string>"));
    assert!(!plist.contains("<string>armv7</string>"));

    assert!(app_path("LaunchScreen.storyboard").exists());
    assert!(app_path("PrivacyInfo.xcprivacy").exists());
    let app_icon_dir = app_path("Assets.xcassets/AppIcon.appiconset");
    let app_icon_json = std::fs::read_to_string(app_icon_dir.join("Contents.json")).unwrap();
    let app_icon: serde_json::Value = serde_json::from_str(&app_icon_json).unwrap();
    let images = app_icon["images"].as_array().unwrap();
    assert!(!images.is_empty());
    for image in images {
        let filename = image["filename"]
            .as_str()
            .expect("each AppIcon slot must name a source image");
        assert!(
            app_icon_dir.join(filename).exists(),
            "missing AppIcon source image: {filename}"
        );
    }
}

#[test]
fn ios_route_change_handler_does_not_pause_for_airplay_or_bluetooth_switches() {
    let audio_manager = std::fs::read_to_string(app_path("AudioManager.swift")).unwrap();

    assert!(audio_manager.contains("shouldPauseForUnavailableRoute"));
    assert!(audio_manager.contains(".headphones"));
    assert!(audio_manager.contains(".headsetMic"));
    assert!(audio_manager.contains("continuing playback"));
    assert!(
        !audio_manager.contains(
            "device unavailable — pausing\")\n            sotf_ios_audio_route_changed()"
        )
    );
}

#[test]
fn ios_project_uses_sotfplayer_as_the_canonical_app_target() {
    let project_yml = std::fs::read_to_string(ios_path("project.yml")).unwrap();
    let pbxproj =
        std::fs::read_to_string(ios_path("SotFPlayer.xcodeproj/project.pbxproj")).unwrap();

    assert!(project_yml.contains("name: SotFPlayer"));
    assert!(project_yml.contains("SotFPlayer:"));
    assert!(project_yml.contains("sources:\n      - SotFPlayer"));
    assert!(pbxproj.contains("PBXNativeTarget \"SotFPlayer\""));
    assert!(!ios_path("SotFApp").exists());
}

#[test]
fn ios_platform_hooks_cover_p1_runtime_events() {
    let app_delegate = std::fs::read_to_string(app_path("AppDelegate.swift")).unwrap();
    let bridge = std::fs::read_to_string(app_path("BridgingHeader.h")).unwrap();

    assert!(app_delegate.contains("UIContentSizeCategory.didChangeNotification"));
    assert!(app_delegate.contains("dynamicTypeScale(for:"));
    assert!(app_delegate.contains("sotf_ios_dynamic_type_scale_changed"));
    assert!(app_delegate.contains("applicationDidReceiveMemoryWarning"));
    assert!(app_delegate.contains("sotf_ios_memory_warning()"));
    assert!(app_delegate.contains("NSProcessInfoPowerStateDidChange"));
    assert!(app_delegate.contains("sotf_ios_low_power_mode_changed"));

    assert!(bridge.contains("sotf_ios_dynamic_type_scale_changed"));
    assert!(bridge.contains("sotf_ios_memory_warning"));
    assert!(bridge.contains("sotf_ios_low_power_mode_changed"));
}

#[test]
fn ios_airplay_route_picker_is_exposed_to_gpui() {
    let app_delegate = std::fs::read_to_string(app_path("AppDelegate.swift")).unwrap();
    let audio_settings = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../app-gpui/components/settings/audio_device/device.rs"),
    )
    .unwrap();

    assert!(app_delegate.contains("@_cdecl(\"sotf_ios_show_route_picker\")"));
    assert!(app_delegate.contains("MPVolumeView"));
    assert!(audio_settings.contains("show-airplay-route-picker"));
    assert!(audio_settings.contains("sotf_ios_show_route_picker"));
}

#[test]
fn gpui_ios_safe_area_applies_landscape_side_insets() {
    let render = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../app-gpui/ui/render.rs"),
    )
    .unwrap();

    assert!(render.contains("let (top, left, bottom, right) = gpui_ios::safe_area_insets();"));
    assert!(render.contains(".pl(px(left))"));
    assert!(render.contains(".pr(px(right))"));
}
