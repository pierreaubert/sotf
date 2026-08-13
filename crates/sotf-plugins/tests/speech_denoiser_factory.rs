use sotf_plugins::{catalog_entry, create_plugin};

#[test]
fn speech_denoiser_factory_catalog_layout_and_state_are_consistent() {
    let entry = catalog_entry("speech_denoiser").expect("Speech Denoiser catalog entry");
    let supported = entry.metadata.channel_layout.supported_inputs;
    assert_eq!(supported.supports(1), Some(true));
    assert_eq!(supported.supports(2), Some(true));
    assert_eq!(supported.supports(3), Some(false));

    for channels in [1, 2] {
        assert!(create_plugin("speech_denoiser", &serde_json::json!({}), channels, 48_000).is_ok());
    }
    for channels in [0, 3, 6, 12] {
        assert!(
            create_plugin("speech_denoiser", &serde_json::json!({}), channels, 48_000).is_err()
        );
    }
    assert!(
        create_plugin(
            "speech_denoiser",
            &serde_json::json!({"enabled": true, "unknown": 1}),
            1,
            48_000,
        )
        .is_err()
    );
}
