use std::path::PathBuf;

/// Read a checked-in text file with line endings normalized, so CRLF
/// checkouts compare like LF ones.
fn read_text(path: impl AsRef<std::path::Path>) -> String {
    let path = path.as_ref();
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

fn tvos_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tvos")
        .join(path)
}

#[test]
fn tvos_privacy_manifest_declares_accessed_apis_without_tracking() {
    let manifest = read_text(tvos_path("SotFTV/PrivacyInfo.xcprivacy"));

    assert!(manifest.contains("NSPrivacyAccessedAPICategoryFileTimestamp"));
    assert!(manifest.contains("NSPrivacyAccessedAPICategoryDiskSpace"));
    assert!(manifest.contains("NSPrivacyAccessedAPICategoryUserDefaults"));
    assert!(manifest.contains("<key>NSPrivacyCollectedDataTypes</key>"));
    assert!(manifest.contains("<array/>"));
    assert!(manifest.contains("<key>NSPrivacyTracking</key>"));
    assert!(manifest.contains("<false/>"));
    assert!(manifest.contains("<key>NSPrivacyTrackingDomains</key>"));
}

#[test]
fn tvos_project_sources_include_privacy_manifest_folder() {
    let project_yml = read_text(tvos_path("project.yml"));

    assert!(project_yml.contains("sources:"));
    assert!(project_yml.contains("- SotFTV"));
    assert!(tvos_path("SotFTV/PrivacyInfo.xcprivacy").exists());
}

#[test]
fn tvos_project_builds_rust_library_instead_of_requiring_committed_archive() {
    let project_yml = read_text(tvos_path("project.yml"));
    let gitignore = read_text(tvos_path(".gitignore"));

    assert!(project_yml.contains("preBuildScripts:"));
    assert!(project_yml.contains("./build-rust.sh"));
    assert!(project_yml.contains("$(DERIVED_FILE_DIR)/rust/libsotf_tvos.a"));
    assert!(!project_yml.contains("$(PROJECT_DIR)/lib/libsotf_tvos.a"));
    assert!(gitignore.lines().any(|line| line.trim() == "lib/"));
}

#[test]
fn tvos_bundle_metadata_has_required_shipping_assets() {
    let project_yml = read_text(tvos_path("project.yml"));
    let plist = read_text(tvos_path("SotFTV/Info.plist"));

    assert!(project_yml.contains("MARKETING_VERSION:"));
    assert!(project_yml.contains("CURRENT_PROJECT_VERSION:"));
    assert!(plist.contains("<string>$(MARKETING_VERSION)</string>"));
    assert!(plist.contains("<string>$(CURRENT_PROJECT_VERSION)</string>"));

    let privacy = read_text(tvos_path("SotFTV/PrivacyInfo.xcprivacy"));
    assert!(privacy.contains("NSPrivacyAccessedAPICategoryFileTimestamp"));
    assert!(privacy.contains("NSPrivacyAccessedAPICategoryDiskSpace"));
    assert!(privacy.contains("NSPrivacyAccessedAPICategoryUserDefaults"));
    assert!(
        privacy.contains("<key>NSPrivacyTracking</key>\n\t<false/>"),
        "SotFTV must declare that it does not track users"
    );
    assert!(
        privacy.contains("<key>NSPrivacyCollectedDataTypes</key>\n\t<array/>"),
        "local network and document import are permission strings, not collected-data declarations"
    );
}
