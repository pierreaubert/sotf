use serde_yaml::Value;

#[test]
fn au_extension_template_links_sqlite_for_rust_staticlib() {
    let project = include_str!("../crates/plugins-au/project.yml");
    let yaml: Value = serde_yaml::from_str(project).expect("project.yml should parse");

    let flags = yaml["targetTemplates"]["SOTFAudioUnitExtension"]["settings"]["OTHER_LDFLAGS"]
        .as_sequence()
        .expect("AU extension template should define OTHER_LDFLAGS");

    assert!(
        flags
            .iter()
            .any(|flag| flag.as_str() == Some("-lsotf_audio_plugins_ffi")),
        "AU extensions should link the staged Rust plugin staticlib"
    );
    assert!(
        flags.iter().any(|flag| flag.as_str() == Some("-lsqlite3")),
        "AU extensions should link SQLite because the Rust staticlib pulls rusqlite from SOFA/HRTF support"
    );
}
