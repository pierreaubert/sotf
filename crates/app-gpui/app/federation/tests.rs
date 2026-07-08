use super::local::local_api_connect_host;
use super::misc::pairing_qr_host;

use super::*;

#[test]
fn pairing_qr_host_uses_configured_lan_address() {
    assert_eq!(
        pairing_qr_host("192.168.1.42").as_deref(),
        Some("192.168.1.42")
    );
}

#[test]
fn pairing_qr_host_rejects_loopback_bind() {
    // Loopback addresses must never appear in a QR code meant for LAN pairing.
    assert_eq!(pairing_qr_host("127.0.0.1"), None);
    assert_eq!(pairing_qr_host("localhost"), None);
}

#[test]
fn pairing_qr_host_rejects_unspecified_bind() {
    // 0.0.0.0 is not a usable QR host; the helper should fall back to a
    // concrete LAN address or return None if none exists.
    let result = pairing_qr_host("0.0.0.0");
    assert!(result.as_deref() != Some("0.0.0.0"));
}

#[test]
fn local_api_connect_host_maps_wildcard_to_loopback() {
    assert_eq!(local_api_connect_host("0.0.0.0"), "127.0.0.1");
    assert_eq!(local_api_connect_host("192.168.1.42"), "192.168.1.42");
}
