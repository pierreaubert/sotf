use std::path::Path;

fn ios_path(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("ios")
        .join(relative)
}

#[test]
fn ios_project_builds_rust_library_instead_of_requiring_committed_archive() {
    assert!(
        !ios_path("lib/libsotf_ios.a").exists(),
        "libsotf_ios.a must be produced by the build phase, not committed"
    );

    let project_yml = std::fs::read_to_string(ios_path("project.yml")).unwrap();
    assert!(project_yml.contains("preBuildScripts:"));
    assert!(project_yml.contains("./build-rust.sh"));
}

#[test]
fn ios_bundle_metadata_has_required_shipping_assets() {
    let plist = std::fs::read_to_string(ios_path("SotFApp/Info.plist")).unwrap();
    assert!(plist.contains("<string>$(MARKETING_VERSION)</string>"));
    assert!(plist.contains("<string>$(CURRENT_PROJECT_VERSION)</string>"));
    assert!(plist.contains("<string>LaunchScreen</string>"));
    assert!(!plist.contains("<string>armv7</string>"));

    assert!(ios_path("SotFApp/LaunchScreen.storyboard").exists());
    assert!(ios_path("SotFApp/PrivacyInfo.xcprivacy").exists());
    let app_icon_dir = ios_path("SotFApp/Assets.xcassets/AppIcon.appiconset");
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
    let audio_manager = std::fs::read_to_string(ios_path("SotFApp/AudioManager.swift")).unwrap();

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
