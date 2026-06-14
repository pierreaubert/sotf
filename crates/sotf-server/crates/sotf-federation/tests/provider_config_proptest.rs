// ============================================================================
// Property-Based Tests for sotf-federation provider configs
// ============================================================================
//
// Verifies that every provider configuration struct round-trips through JSON
// serialization without losing information.

use proptest::prelude::*;
use sotf_federation::{DlnaProviderConfig, MpdProviderConfig};

fn host_strategy() -> BoxedStrategy<String> {
    proptest::string::string_regex("[a-zA-Z0-9_.:-]+")
        .unwrap()
        .boxed()
}

fn dlna_config_strategy() -> BoxedStrategy<DlnaProviderConfig> {
    (
        proptest::string::string_regex("https?://[a-zA-Z0-9_.:/-]+")
            .unwrap()
            .boxed(),
        proptest::string::string_regex("[a-zA-Z0-9_ ./:-]+")
            .unwrap()
            .boxed(),
    )
        .prop_map(|(location_url, friendly_name)| DlnaProviderConfig {
            location_url,
            friendly_name,
        })
        .boxed()
}

fn mpd_config_strategy() -> BoxedStrategy<MpdProviderConfig> {
    (
        host_strategy(),
        1u16..65535u16,
        prop::option::of(
            proptest::string::string_regex("[a-zA-Z0-9_!@#$%^&*./:-]+")
                .unwrap()
                .boxed(),
        ),
        1u16..65535u16,
    )
        .prop_map(|(host, port, password, httpd_port)| MpdProviderConfig {
            host,
            port,
            password,
            httpd_port,
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// INVARIANT: `DlnaProviderConfig` round-trips through JSON serialization.
    #[test]
    fn dlna_provider_config_json_roundtrip(config in dlna_config_strategy()) {
        let json = serde_json::to_string(&config).expect("serialize");
        let decoded: DlnaProviderConfig = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(
            decoded, config,
            "DLNA provider config JSON round-trip failed: {}",
            json
        );
    }

    /// INVARIANT: `MpdProviderConfig` round-trips through JSON serialization.
    #[test]
    fn mpd_provider_config_json_roundtrip(config in mpd_config_strategy()) {
        let json = serde_json::to_string(&config).expect("serialize");
        let decoded: MpdProviderConfig = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(
            decoded, config,
            "MPD provider config JSON round-trip failed: {}",
            json
        );
    }
}
