use crate::misc::parse_release_year;
use crate::misc::tidal_cover_url;
use crate::misc::truncate_for_log;
use crate::tidal_service::TidalService;
use sotf_services::*;

#[test]
fn test_tidal_service_not_authenticated() {
    let service = TidalService::new();
    assert!(!service.is_authenticated());
}

#[test]
fn test_tidal_quality_mapping() {
    let mut service = TidalService::new();
    service.quality = AudioQuality::Lossless;
    assert_eq!(service.quality_to_tidal_quality(), "LOSSLESS");

    service.quality = AudioQuality::High;
    assert_eq!(service.quality_to_tidal_quality(), "HIGH");

    service.quality = AudioQuality::HiRes;
    assert_eq!(service.quality_to_tidal_quality(), "HI_RES_LOSSLESS");
}

#[test]
fn test_tidal_device_code_requires_client_id() {
    // Force an empty client id so the test is hermetic: `new()` would
    // otherwise honor TIDAL_CLIENT_ID from the environment and this test
    // would POST to the real auth server.
    let mut service = TidalService::new().with_client_id("");
    let result = service.authenticate(ServiceCredentials::DeviceCode);
    assert!(result.is_err());
    match result {
        Err(ServiceError::AuthError(msg)) => {
            assert!(msg.contains("client_id"));
        }
        _ => panic!("Expected AuthError"),
    }
}

#[test]
fn test_parse_release_year_well_formed() {
    assert_eq!(parse_release_year("1991-09-24"), Some(1991));
    assert_eq!(parse_release_year("2026"), Some(2026));
    assert_eq!(parse_release_year("2026-05"), Some(2026));
}

#[test]
fn test_parse_release_year_short_input_no_panic() {
    // Inputs shorter than 4 bytes must not panic — the original code
    // sliced [..4] and would have crashed here.
    assert_eq!(parse_release_year(""), None);
    assert_eq!(parse_release_year("1"), None);
    assert_eq!(parse_release_year("12"), None);
    assert_eq!(parse_release_year("199"), None);
}

#[test]
fn test_parse_release_year_non_ascii_no_panic() {
    // A 4-byte input that is not on a UTF-8 boundary at byte 4 would have
    // panicked under the original `d[..4]` slice.
    assert_eq!(parse_release_year("éé"), None);
    // Non-numeric prefix.
    assert_eq!(parse_release_year("abcd-01-01"), None);
    // Mixed prefix.
    assert_eq!(parse_release_year("19a1-01-01"), None);
}

#[test]
fn test_tidal_cover_url_valid() {
    let url = tidal_cover_url("ab12cd34-5678-90ef-1234-567890abcdef").unwrap();
    assert!(url.starts_with("https://resources.tidal.com/images/"));
    assert!(url.ends_with("/640x640.jpg"));
    // Dashes get turned into path separators.
    assert!(url.contains("/ab12cd34/5678/90ef/1234/567890abcdef/"));
}

#[test]
fn test_tidal_cover_url_rejects_path_traversal() {
    // Inputs containing characters outside of hex+dash are rejected so a
    // hostile or unexpected `cover` value cannot form a `../` sequence.
    assert_eq!(tidal_cover_url("../../etc/passwd"), None);
    assert_eq!(tidal_cover_url("ab12/cd34"), None);
    assert_eq!(tidal_cover_url("ab12?evil=1"), None);
    assert_eq!(tidal_cover_url(""), None);
}

#[test]
fn test_truncate_for_log() {
    assert_eq!(truncate_for_log("hello", 10), "hello");
    assert_eq!(truncate_for_log("hello world", 5), "hello…");
}

#[test]
fn test_tidal_service_debug_redacts_tokens() {
    let mut service = TidalService::new();
    service.access_token = Some("secret-access-token-do-not-log".to_string());
    service.refresh_token = Some("refresh-secret-token".to_string());
    let dbg = format!("{:?}", service);
    assert!(!dbg.contains("secret-access-token-do-not-log"));
    assert!(!dbg.contains("refresh-secret-token"));
    // First 4 chars of each token are visible, the rest redacted.
    assert!(dbg.contains("secr"));
    assert!(dbg.contains("refr"));
    assert!(dbg.contains("***"));
}

#[test]
fn test_malformed_tidal_session_json_is_rejected() {
    let result: Result<crate::types::TidalSession, _> =
        serde_json::from_str(r#"{"userId": "not-a-number"}"#);
    assert!(result.is_err());

    let result: Result<crate::types::TidalSession, _> =
        serde_json::from_str(r#"{"countryCode": "US"}"#);
    assert!(result.is_err());
}

#[test]
fn test_malformed_tidal_device_auth_json_is_rejected() {
    let result: Result<crate::types::TidalDeviceAuth, _> =
        serde_json::from_str(r#"{"expiresIn": "not-a-number"}"#);
    assert!(result.is_err());
}

#[test]
fn test_service_error_auth_does_not_echo_secret() {
    // ServiceError Display prints messages verbatim by design — device-flow
    // messages legitimately carry user codes — so the real contract enforced
    // at construction sites is that secrets are passed through
    // `redact_secret` before being embedded in an error. This test pins that
    // contract: a redacted error must not echo the raw token, but must carry
    // the redacted prefix for debuggability.
    let token = "bearer-secret-token-12345";
    let err = ServiceError::AuthError(format!("token validation failed: {}", redact_secret(token)));
    let text = err.to_string();
    assert!(
        !text.contains(token),
        "ServiceError must not echo the raw secret, got: {text}"
    );
    assert!(
        text.contains("bear***"),
        "ServiceError should carry the redacted prefix, got: {text}"
    );
}
