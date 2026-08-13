pub use super::eq::{EQFilter, KautzSectionConfig};
use super::plugin_settings::PluginSettings;
use super::plugin_type::PluginType;

#[test]
fn binaural_decoder_defaults_omit_legacy_noop_params() {
    let settings = PluginSettings::default_for(&PluginType::BinauralDecoder).unwrap();
    let mut serialized = serde_json::to_value(&settings).unwrap();
    let variant = serialized
        .as_object_mut()
        .and_then(|object| object.values_mut().next())
        .and_then(serde_json::Value::as_object_mut)
        .unwrap();
    assert!(variant.get("enable_optimization").is_none());
    assert!(variant.get("headphone_eq_enabled").is_none());

    variant.insert("enable_optimization".into(), serde_json::json!(false));
    variant.insert("headphone_eq_enabled".into(), serde_json::json!(true));
    let restored: PluginSettings = serde_json::from_value(serialized).unwrap();
    assert!(matches!(restored, PluginSettings::BinauralDecoder { .. }));
}

/// Plain biquad serialization stays minimal — no `topology` / `lambda`
/// / `kautz_sections` keys leak into legacy JSON. Pins the producer
/// contract: code that round-trips this JSON against an older parser
/// must not see unexpected fields.
#[test]
fn eq_to_plugin_config_omits_topology_for_plain_biquads() {
    use math_audio_iir_fir::BiquadFilterType;
    let filter = EQFilter::new(BiquadFilterType::Peak, 1000.0, 1.0, 3.0);
    let settings = PluginSettings::EQ {
        channels: 2,
        filters: vec![filter],
        channel_filters: None,
        per_channel_mode: false,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
        auto_gain_enabled: false,
        oversampling: 1.0,
    };
    let cfg = settings.to_plugin_config(48_000.0);
    let filters = cfg.parameters.get("filters").expect("filters present");
    let first = &filters.as_array().expect("array")[0];
    assert!(first.get("topology").is_none());
    assert!(first.get("lambda").is_none());
    assert!(first.get("kautz_sections").is_none());
}

/// Warped-biquad topology + lambda survive the engine→plugin JSON
/// round-trip. Before this PR the producer dropped these fields,
/// silently downgrading RoomEQ-emitted warped filters to plain biquads.
#[test]
fn eq_to_plugin_config_emits_topology_and_lambda_for_warped() {
    use math_audio_iir_fir::BiquadFilterType;
    let filter = EQFilter::new_warped(BiquadFilterType::Peak, 80.0, 2.0, -4.0, Some(0.5));
    let settings = PluginSettings::EQ {
        channels: 2,
        filters: vec![filter],
        channel_filters: None,
        per_channel_mode: false,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
        auto_gain_enabled: false,
        oversampling: 1.0,
    };
    let cfg = settings.to_plugin_config(48_000.0);
    let filters = cfg.parameters.get("filters").expect("filters present");
    let first = &filters.as_array().expect("array")[0];
    assert_eq!(
        first.get("topology").and_then(|v| v.as_str()),
        Some("warped_biquad")
    );
    assert_eq!(first.get("lambda").and_then(|v| v.as_f64()), Some(0.5));
}

/// Kautz topology serialises its pole sections so the plugin's runtime
/// can reconstruct the parallel modal correction. Sections with no
/// explicit gain still serialise (defaults to 0).
#[test]
fn eq_to_plugin_config_emits_kautz_sections() {
    let sections = vec![
        KautzSectionConfig {
            pole_freq: 45.0,
            q: 12.0,
            gain: -3.0,
        },
        KautzSectionConfig {
            pole_freq: 80.0,
            q: 8.0,
            gain: -2.0,
        },
    ];
    let filter = EQFilter::new_kautz(100.0, 1.0, 0.0, sections);
    let settings = PluginSettings::EQ {
        channels: 2,
        filters: vec![filter],
        channel_filters: None,
        per_channel_mode: false,
        max_filters: 10,
        tdf2: false,
        topology: 0.0,
        auto_gain_enabled: false,
        oversampling: 1.0,
    };
    let cfg = settings.to_plugin_config(48_000.0);
    let filters = cfg.parameters.get("filters").expect("filters present");
    let first = &filters.as_array().expect("array")[0];
    assert_eq!(
        first.get("topology").and_then(|v| v.as_str()),
        Some("kautz_filter")
    );
    let emitted = first
        .get("kautz_sections")
        .and_then(|v| v.as_array())
        .expect("kautz_sections is an array");
    assert_eq!(emitted.len(), 2);
    assert_eq!(
        emitted[0].get("pole_freq").and_then(|v| v.as_f64()),
        Some(45.0)
    );
    assert_eq!(emitted[1].get("q").and_then(|v| v.as_f64()), Some(8.0));
}
