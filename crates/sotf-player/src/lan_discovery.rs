//! LAN discovery advertisement for the native SOTF control API.

use std::net::Ipv4Addr;
#[cfg(not(target_os = "macos"))]
use std::net::{SocketAddr, SocketAddrV4};

#[cfg(not(target_os = "macos"))]
use tokio::net::UdpSocket;

use crate::federation_config::SotfApiSettings;

const SOTF_SERVICE_TYPE: &str = "_sotf._tcp.local";
#[cfg(not(target_os = "macos"))]
const MDNS_MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
#[cfg(not(target_os = "macos"))]
const MDNS_PORT: u16 = 5353;
#[cfg(any(not(target_os = "macos"), test))]
const SERVICE_ENUMERATION_TYPE: &str = "_services._dns-sd._udp.local";
#[cfg(any(not(target_os = "macos"), test))]
const MDNS_TTL_SECS: u32 = 120;

#[derive(Clone, Debug)]
#[cfg_attr(target_os = "macos", allow(dead_code))]
struct SotfServiceDescriptor {
    instance_name: String,
    service_type: String,
    host_name: String,
    port: u16,
    address: Ipv4Addr,
    txt_records: Vec<String>,
}

impl SotfServiceDescriptor {
    fn new(settings: &SotfApiSettings, address: Ipv4Addr) -> Self {
        let friendly_name = settings.friendly_name.trim();
        let instance = dns_label(friendly_name, "SOTF Player");
        let host = format!("{}.local", dns_label(friendly_name, "sotf-player-host"));
        Self {
            instance_name: format!("{instance}.{SOTF_SERVICE_TYPE}"),
            service_type: SOTF_SERVICE_TYPE.to_string(),
            host_name: host,
            port: settings.port,
            address,
            txt_records: vec![
                "api=1".to_string(),
                "path=/api/v1".to_string(),
                "auth=bearer".to_string(),
                "proto=http".to_string(),
                format!("name={}", txt_value(friendly_name, "SOTF Player")),
            ],
        }
    }
}

/// Advertise the SOTF API as `_sotf._tcp` so mobile clients can discover it.
pub async fn run_sotf_lan_discovery(
    settings: SotfApiSettings,
    local_ip: Ipv4Addr,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let descriptor = SotfServiceDescriptor::new(&settings, local_ip);

    #[cfg(target_os = "macos")]
    {
        platform::run_dns_sd_registration(descriptor, cancel).await
    }

    #[cfg(not(target_os = "macos"))]
    {
        run_mdns_responder(descriptor, cancel).await
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::process::{Command, Stdio};

    pub async fn run_dns_sd_registration(
        descriptor: SotfServiceDescriptor,
        mut cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<(), String> {
        let service_name = descriptor
            .instance_name
            .split_once('.')
            .map_or(descriptor.instance_name.as_str(), |(name, _)| name);
        let mut child = Command::new("/usr/bin/dns-sd")
            .arg("-R")
            .arg(service_name)
            .arg("_sotf._tcp")
            .arg("local.")
            .arg(descriptor.port.to_string())
            .args(&descriptor.txt_records)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start dns-sd Bonjour registration: {e}"))?;

        log::info!(
            "[SOTF Discovery] Registered Bonjour service {} on port {}",
            descriptor.instance_name,
            descriptor.port
        );

        loop {
            if *cancel.borrow() {
                break;
            }
            if cancel.changed().await.is_err() {
                break;
            }
        }

        let _ = child.kill();
        let _ = child.wait();
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
async fn run_mdns_responder(
    descriptor: SotfServiceDescriptor,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MDNS_PORT);
    let socket = UdpSocket::bind(bind_addr)
        .await
        .map_err(|e| format!("Failed to bind mDNS responder socket: {e}"))?;
    socket
        .join_multicast_v4(MDNS_MULTICAST, Ipv4Addr::UNSPECIFIED)
        .map_err(|e| format!("Failed to join mDNS multicast group: {e}"))?;

    let target = SocketAddr::V4(SocketAddrV4::new(MDNS_MULTICAST, MDNS_PORT));
    let announcement = build_mdns_response(&descriptor);
    let _ = socket.send_to(&announcement, target).await;

    let mut buf = [0u8; 4096];
    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow() {
                    break;
                }
            }
            received = socket.recv_from(&mut buf) => {
                let Ok((len, from)) = received else {
                    continue;
                };
                if mdns_query_matches(&buf[..len], &descriptor) {
                    let response = build_mdns_response(&descriptor);
                    let target = if from.port() == MDNS_PORT { target } else { from };
                    if let Err(e) = socket.send_to(&response, target).await {
                        log::warn!("[SOTF Discovery] Failed to send mDNS response: {e}");
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn mdns_query_matches(packet: &[u8], descriptor: &SotfServiceDescriptor) -> bool {
    if packet.len() < 12 {
        return false;
    }
    let qdcount = u16::from_be_bytes([packet[4], packet[5]]);
    let mut pos = 12;
    for _ in 0..qdcount {
        let Some((name, next_pos)) = decode_dns_name(packet, pos) else {
            return false;
        };
        pos = next_pos.saturating_add(4);
        if pos > packet.len() {
            return false;
        }
        if name.eq_ignore_ascii_case(&descriptor.service_type)
            || name.eq_ignore_ascii_case(&descriptor.instance_name)
            || name.eq_ignore_ascii_case(&descriptor.host_name)
            || name.eq_ignore_ascii_case(SERVICE_ENUMERATION_TYPE)
        {
            return true;
        }
    }
    false
}

#[cfg(any(not(target_os = "macos"), test))]
fn build_mdns_response(descriptor: &SotfServiceDescriptor) -> Vec<u8> {
    let mut records = Vec::new();
    let ttl = MDNS_TTL_SECS;

    records.push(dns_record(
        &descriptor.service_type,
        12,
        1,
        ttl,
        dns_name_bytes(&descriptor.instance_name),
    ));
    records.push(dns_record(
        SERVICE_ENUMERATION_TYPE,
        12,
        1,
        ttl,
        dns_name_bytes(&descriptor.service_type),
    ));

    let mut srv = Vec::new();
    srv.extend_from_slice(&0u16.to_be_bytes());
    srv.extend_from_slice(&0u16.to_be_bytes());
    srv.extend_from_slice(&descriptor.port.to_be_bytes());
    encode_dns_name(&mut srv, &descriptor.host_name);
    records.push(dns_record(&descriptor.instance_name, 33, 0x8001, ttl, srv));

    records.push(dns_record(
        &descriptor.instance_name,
        16,
        0x8001,
        ttl,
        txt_record_bytes(&descriptor.txt_records),
    ));

    records.push(dns_record(
        &descriptor.host_name,
        1,
        0x8001,
        ttl,
        descriptor.address.octets().to_vec(),
    ));

    let mut packet = Vec::with_capacity(512);
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&0x8400u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&(records.len() as u16).to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    for record in records {
        packet.extend_from_slice(&record);
    }
    packet
}

#[cfg(any(not(target_os = "macos"), test))]
fn dns_record(name: &str, record_type: u16, class: u16, ttl: u32, rdata: Vec<u8>) -> Vec<u8> {
    let mut record = Vec::with_capacity(name.len() + rdata.len() + 16);
    encode_dns_name(&mut record, name);
    record.extend_from_slice(&record_type.to_be_bytes());
    record.extend_from_slice(&class.to_be_bytes());
    record.extend_from_slice(&ttl.to_be_bytes());
    record.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    record.extend_from_slice(&rdata);
    record
}

#[cfg(any(not(target_os = "macos"), test))]
fn dns_name_bytes(name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_dns_name(&mut bytes, name);
    bytes
}

#[cfg(any(not(target_os = "macos"), test))]
fn encode_dns_name(packet: &mut Vec<u8>, name: &str) {
    for label in name.trim_end_matches('.').split('.') {
        let label = label.as_bytes();
        let len = label.len().min(63);
        packet.push(len as u8);
        packet.extend_from_slice(&label[..len]);
    }
    packet.push(0);
}

#[cfg(not(target_os = "macos"))]
fn decode_dns_name(packet: &[u8], mut pos: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut consumed_pos = pos;
    let mut jumped = false;
    let mut jumps = 0;

    loop {
        if pos >= packet.len() || jumps > 10 {
            return None;
        }
        let len = packet[pos] as usize;
        if len == 0 {
            if !jumped {
                consumed_pos = pos + 1;
            }
            break;
        }
        if len & 0xC0 == 0xC0 {
            if pos + 1 >= packet.len() {
                return None;
            }
            if !jumped {
                consumed_pos = pos + 2;
            }
            pos = (len & 0x3F) << 8 | packet[pos + 1] as usize;
            jumped = true;
            jumps += 1;
            continue;
        }
        pos += 1;
        if pos + len > packet.len() {
            return None;
        }
        labels.push(String::from_utf8_lossy(&packet[pos..pos + len]).to_string());
        pos += len;
        if !jumped {
            consumed_pos = pos;
        }
    }

    Some((labels.join("."), consumed_pos))
}

#[cfg(any(not(target_os = "macos"), test))]
fn txt_record_bytes(records: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for record in records {
        let record = record.as_bytes();
        let len = record.len().min(255);
        bytes.push(len as u8);
        bytes.extend_from_slice(&record[..len]);
    }
    bytes
}

fn txt_value(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.chars().filter(|c| *c != '\0').collect()
    }
}

fn dns_label(value: &str, fallback: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch.is_ascii_whitespace() || matches!(ch, '-' | '_' | '.') {
            if !out.ends_with('-') {
                out.push('-');
            }
        }
        if out.len() >= 48 {
            break;
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() {
        fallback.to_string()
    } else {
        out.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings(name: &str) -> SotfApiSettings {
        SotfApiSettings {
            enabled: true,
            bind_address: "0.0.0.0".to_string(),
            port: 8732,
            friendly_name: name.to_string(),
            auth_token: Some("secret".to_string()),
        }
    }

    #[test]
    fn service_descriptor_uses_ios_bonjour_type() {
        let descriptor =
            SotfServiceDescriptor::new(&test_settings("Living Room SOTF"), Ipv4Addr::LOCALHOST);
        assert_eq!(descriptor.service_type, "_sotf._tcp.local");
        assert_eq!(descriptor.port, 8732);
        assert!(descriptor.instance_name.starts_with("Living-Room-SOTF."));
        assert!(descriptor.txt_records.iter().any(|r| r == "path=/api/v1"));
        assert!(descriptor.txt_records.iter().any(|r| r == "auth=bearer"));
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
        let descriptor = SotfServiceDescriptor::new(
            &test_settings("Kitchen SOTF"),
            Ipv4Addr::new(192, 168, 1, 42),
        );
        let packet = build_mdns_response(&descriptor);
        assert!(packet.windows(2).any(|w| w == 8732u16.to_be_bytes()));
        assert!(packet.windows(4).any(|w| w == [192, 168, 1, 42]));
        assert!(packet.windows(5).any(|w| w == b"_sotf"));
    }
}
