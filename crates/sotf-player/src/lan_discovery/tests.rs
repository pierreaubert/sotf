use super::build::build_mdns_query;
#[cfg(any(not(target_os = "macos"), test))]
use super::build::build_mdns_response;
use super::consts::SOTF_API_SERVICE_TYPE;
use super::misc::decode_dns_name;
use super::parse::parse_sotf_mdns_response;
use super::sotf_service_descriptor::SotfServiceDescriptor;
#[cfg(any(not(target_os = "macos"), test))]
use super::txt::txt_record_bytes;
use crate::federation_config::SotfApiSettings;
use std::net::Ipv4Addr;

fn test_settings(name: &str) -> SotfApiSettings {
    SotfApiSettings {
        enabled: true,
        bind_address: "0.0.0.0".to_string(),
        port: 8732,
        friendly_name: name.to_string(),
        auth_token: Some("secret".to_string()),
    }
}

#[cfg(any(not(target_os = "macos"), test))]
fn remove_answer_record(packet: &mut Vec<u8>, index: usize) {
    let ancount = u16::from_be_bytes([packet[6], packet[7]]) as usize;
    assert!(index < ancount, "answer index out of bounds");

    let mut pos = 12;
    for _ in 0..index {
        let (_, name_end) = decode_dns_name(packet, pos).expect("valid record name");
        let rdlength =
            u16::from_be_bytes([packet[name_end + 8], packet[name_end + 9]]) as usize;
        pos = name_end + 10 + rdlength;
    }

    let start = pos;
    let (_, name_end) = decode_dns_name(packet, pos).expect("valid record name");
    let rdlength = u16::from_be_bytes([packet[name_end + 8], packet[name_end + 9]]) as usize;
    let end = name_end + 10 + rdlength;

    packet.drain(start..end);
    let new_ancount = (ancount - 1) as u16;
    packet[6..8].copy_from_slice(&new_ancount.to_be_bytes());
}

#[test]
fn service_descriptor_uses_ios_bonjour_type() {
    let descriptor = SotfServiceDescriptor::with_pairing(
        &test_settings("Living Room SOTF"),
        Ipv4Addr::LOCALHOST,
        false,
    );
    assert_eq!(descriptor.service_type, "_sotf._tcp.local");
    assert_eq!(descriptor.port, 8732);
    assert!(descriptor.instance_name.starts_with("Living-Room-SOTF."));
    assert!(descriptor.txt_records.iter().any(|r| r == "path=/api/v1"));
    assert!(descriptor.txt_records.iter().any(|r| r == "auth=bearer"));
    assert!(
        !descriptor
            .txt_records
            .iter()
            .any(|r| r.starts_with("pairing="))
    );
}

#[test]
fn service_descriptor_advertises_pairing_without_nonce() {
    let descriptor = SotfServiceDescriptor::with_pairing(
        &test_settings("Living Room SOTF"),
        Ipv4Addr::LOCALHOST,
        true,
    );
    assert!(descriptor.txt_records.iter().any(|r| r == "pairing=1"));
    assert!(
        !descriptor
            .txt_records
            .iter()
            .any(|r| r.starts_with("nonce="))
    );
}

#[test]
fn txt_record_bytes_are_length_prefixed() {
    let bytes = txt_record_bytes(&["api=1".to_string(), "path=/api/v1".to_string()]);
    assert_eq!(bytes[0], 5);
    assert_eq!(&bytes[1..6], b"api=1");
    assert_eq!(bytes[6], 12);
    assert_eq!(&bytes[7..19], b"path=/api/v1");
}

#[test]
fn mdns_response_contains_srv_port_and_address() {
    let descriptor = SotfServiceDescriptor::with_pairing(
        &test_settings("Kitchen SOTF"),
        Ipv4Addr::new(192, 168, 1, 42),
        false,
    );
    let packet = build_mdns_response(&descriptor);
    assert!(packet.windows(2).any(|w| w == 8732u16.to_be_bytes()));
    assert!(packet.windows(4).any(|w| w == [192, 168, 1, 42]));
    assert!(packet.windows(5).any(|w| w == b"_sotf"));
}

#[test]
fn mdns_query_asks_for_sotf_ptr_records() {
    let packet = build_mdns_query(SOTF_API_SERVICE_TYPE);
    assert_eq!(packet[4..6], [0, 1]);
    assert!(packet.windows(5).any(|w| w == b"_sotf"));
    assert_eq!(&packet[packet.len() - 4..], &[0, 12, 0, 1]);
}

#[test]
fn parses_advertised_sotf_api_server() {
    let descriptor = SotfServiceDescriptor::with_pairing(
        &test_settings("Kitchen SOTF"),
        Ipv4Addr::new(192, 168, 1, 42),
        false,
    );
    let packet = build_mdns_response(&descriptor);
    let from = "192.168.1.42:5353".parse().unwrap();
    let servers = parse_sotf_mdns_response(&packet, from);

    assert_eq!(servers.len(), 1);
    let server = &servers[0];
    assert_eq!(server.friendly_name, "Kitchen SOTF");
    assert_eq!(server.address, Ipv4Addr::new(192, 168, 1, 42));
    assert_eq!(server.port, 8732);
    assert_eq!(server.protocol, "http");
    assert_eq!(server.api_path, "/api/v1");
    assert_eq!(server.auth, "bearer");
    assert_eq!(server.origin_url, "http://192.168.1.42:8732");
    assert_eq!(server.api_base_url, "http://192.168.1.42:8732/api/v1");
    assert_eq!(server.txt_records.get("api").map(String::as_str), Some("1"));
}

#[test]
fn parser_rejects_mdns_queries_as_servers() {
    let packet = build_mdns_query(SOTF_API_SERVICE_TYPE);
    let from = "192.168.1.42:5353".parse().unwrap();
    assert!(parse_sotf_mdns_response(&packet, from).is_empty());
}

#[test]
fn parser_rejects_empty_packet() {
    let from = "192.168.1.42:5353".parse().unwrap();
    assert!(parse_sotf_mdns_response(&[], from).is_empty());
}

#[test]
fn parser_rejects_short_packet() {
    let from = "192.168.1.42:5353".parse().unwrap();
    assert!(parse_sotf_mdns_response(&[0; 11], from).is_empty());
}

#[test]
fn parser_rejects_query_flag_packets() {
    // Standard query response flag is 0x8400; flip the response bit off.
    let mut packet = build_mdns_query(SOTF_API_SERVICE_TYPE);
    packet[2] = 0x00;
    packet[3] = 0x00;
    let from = "192.168.1.42:5353".parse().unwrap();
    assert!(parse_sotf_mdns_response(&packet, from).is_empty());
}

#[test]
fn parser_uses_fallback_ipv4_address() {
    // The descriptor advertises 192.168.1.42; remove the A record so the
    // parser has to fall back to the source address.
    let descriptor = SotfServiceDescriptor::with_pairing(
        &test_settings("Kitchen SOTF"),
        Ipv4Addr::new(192, 168, 1, 42),
        false,
    );
    let mut packet = build_mdns_response(&descriptor);
    // A record is the last answer (index 4: PTR, PTR, SRV, TXT, A).
    remove_answer_record(&mut packet, 4);

    let from = "192.168.1.99:5353".parse().unwrap();
    let servers = parse_sotf_mdns_response(&packet, from);
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].address, Ipv4Addr::new(192, 168, 1, 99));
}

#[test]
fn parser_skips_ipv6_source_fallback() {
    let descriptor = SotfServiceDescriptor::with_pairing(
        &test_settings("Kitchen SOTF"),
        Ipv4Addr::new(192, 168, 1, 42),
        false,
    );
    let mut packet = build_mdns_response(&descriptor);
    // Remove the A record; IPv6 sources provide no usable IPv4 fallback.
    remove_answer_record(&mut packet, 4);

    let from = "[::1]:5353".parse().unwrap();
    assert!(parse_sotf_mdns_response(&packet, from).is_empty());
}

#[test]
fn parser_skips_instance_without_srv() {
    let descriptor = SotfServiceDescriptor::with_pairing(
        &test_settings("Kitchen SOTF"),
        Ipv4Addr::new(192, 168, 1, 42),
        false,
    );
    let mut packet = build_mdns_response(&descriptor);
    // Remove the SRV record (index 2: PTR, PTR, SRV, TXT, A).
    remove_answer_record(&mut packet, 2);

    let from = "192.168.1.42:5353".parse().unwrap();
    assert!(parse_sotf_mdns_response(&packet, from).is_empty());
}
