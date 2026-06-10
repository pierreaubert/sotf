//! Property-based tests for streaming helpers.

use proptest::prelude::*;
use sotf_streaming::IcyMetadata;

fn simple_icy_value_strategy() -> impl Strategy<Value = String> {
    // Values that won't break the ICY single-quote parser.
    proptest::string::string_regex("[a-zA-Z0-9 _/.:-]+").unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, ..ProptestConfig::default() })]

    /// INVARIANT: parsing arbitrary bytes as ICY metadata never panics.
    #[test]
    fn icy_random_bytes_no_panic(bytes in prop::collection::vec(0u8..255, 0..64)) {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = IcyMetadata::parse(&bytes);
        }));
        prop_assert!(result.is_ok(), "IcyMetadata::parse panicked on bytes: {:?}", bytes);
    }

    /// INVARIANT: a syntactically valid ICY title is parsed back exactly.
    #[test]
    fn icy_title_roundtrip(title in simple_icy_value_strategy()) {
        let raw = format!("StreamTitle='{}';", title);
        let meta = IcyMetadata::parse(raw.as_bytes());
        let expected = if title.is_empty() { None } else { Some(title) };
        prop_assert_eq!(meta.stream_title, expected, "title round-trip failed for {:?}", raw);
        prop_assert_eq!(meta.stream_url, None);
    }

    /// INVARIANT: a syntactically valid ICY URL is parsed back exactly.
    #[test]
    fn icy_url_roundtrip(url in simple_icy_value_strategy()) {
        let raw = format!("StreamUrl='{}';", url);
        let meta = IcyMetadata::parse(raw.as_bytes());
        let expected = if url.is_empty() { None } else { Some(url) };
        prop_assert_eq!(meta.stream_url, expected, "url round-trip failed for {:?}", raw);
        prop_assert_eq!(meta.stream_title, None);
    }

    /// INVARIANT: both ICY fields are parsed from a combined block.
    #[test]
    fn icy_both_fields_parsed(
        title in simple_icy_value_strategy(),
        url in simple_icy_value_strategy()
    ) {
        let raw = format!("StreamTitle='{}';StreamUrl='{}';", title, url);
        let meta = IcyMetadata::parse(raw.as_bytes());
        let expected_title = if title.is_empty() { None } else { Some(title) };
        let expected_url = if url.is_empty() { None } else { Some(url) };
        prop_assert_eq!(meta.stream_title, expected_title);
        prop_assert_eq!(meta.stream_url, expected_url);
    }
}
