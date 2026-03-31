// ============================================================================
// Cast Device Discovery via mDNS/DNS-SD
// ============================================================================
//
// Discovers AirPlay receivers (_raop._tcp) and Chromecast devices
// (_googlecast._tcp) on the local network using mDNS multicast.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::net::UdpSocket;

const MDNS_MULTICAST: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;

/// Type of cast device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastDeviceType {
    AirPlay,
    Chromecast,
}

impl std::fmt::Display for CastDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CastDeviceType::AirPlay => write!(f, "AirPlay"),
            CastDeviceType::Chromecast => write!(f, "Chromecast"),
        }
    }
}

/// A discovered cast device on the network.
#[derive(Debug, Clone)]
pub struct CastDevice {
    /// Device type (AirPlay or Chromecast).
    pub device_type: CastDeviceType,
    /// Human-readable device name (e.g. "Living Room HomePod").
    pub name: String,
    /// IP address of the device.
    pub address: Ipv4Addr,
    /// Service port (AirPlay: typically 7000, Chromecast: 8009).
    pub port: u16,
    /// mDNS instance name (the full service name).
    pub instance_name: String,
    /// TXT record key-value pairs.
    pub txt_records: HashMap<String, String>,
}

impl CastDevice {
    /// For AirPlay devices: the device model string from TXT records.
    pub fn model(&self) -> Option<&str> {
        self.txt_records.get("am").map(|s| s.as_str())
    }

    /// For Chromecast: the device model from TXT records.
    pub fn chromecast_model(&self) -> Option<&str> {
        self.txt_records.get("md").map(|s| s.as_str())
    }

    /// For AirPlay: supported features bitmask.
    pub fn airplay_features(&self) -> Option<u64> {
        self.txt_records
            .get("ft")
            .and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok())
    }
}

/// mDNS-based device discovery.
pub struct CastDiscovery;

impl CastDiscovery {
    /// Discover AirPlay receivers on the local network.
    pub async fn discover_airplay(timeout: Duration) -> Result<Vec<CastDevice>, String> {
        Self::discover("_raop._tcp.local", CastDeviceType::AirPlay, timeout).await
    }

    /// Discover Chromecast devices on the local network.
    pub async fn discover_chromecast(timeout: Duration) -> Result<Vec<CastDevice>, String> {
        Self::discover(
            "_googlecast._tcp.local",
            CastDeviceType::Chromecast,
            timeout,
        )
        .await
    }

    /// Discover all cast devices (AirPlay + Chromecast).
    pub async fn discover_all(timeout: Duration) -> Result<Vec<CastDevice>, String> {
        let (airplay, chromecast) = tokio::join!(
            Self::discover_airplay(timeout),
            Self::discover_chromecast(timeout),
        );

        let mut devices = airplay.unwrap_or_default();
        devices.extend(chromecast.unwrap_or_default());
        Ok(devices)
    }

    async fn discover(
        service_type: &str,
        device_type: CastDeviceType,
        timeout: Duration,
    ) -> Result<Vec<CastDevice>, String> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
            .await
            .map_err(|e| format!("Failed to bind mDNS socket: {}", e))?;

        // Build mDNS query packet
        let query = build_mdns_query(service_type);
        let target = SocketAddr::V4(SocketAddrV4::new(MDNS_MULTICAST, MDNS_PORT));

        socket
            .send_to(&query, target)
            .await
            .map_err(|e| format!("Failed to send mDNS query: {}", e))?;

        log::debug!("[Cast Discovery] Sent mDNS query for {}", service_type);

        let mut devices = Vec::new();
        let mut buf = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, from))) => {
                    if let Some(device) = parse_mdns_response(&buf[..len], from, device_type)
                        && !devices.iter().any(|d: &CastDevice| {
                            d.address == device.address && d.port == device.port
                        })
                    {
                        log::debug!(
                            "[Cast Discovery] Found {} device: {} at {}:{}",
                            device.device_type,
                            device.name,
                            device.address,
                            device.port,
                        );
                        devices.push(device);
                    }
                }
                Ok(Err(e)) => {
                    log::debug!("[Cast Discovery] Receive error: {}", e);
                    break;
                }
                Err(_) => break, // Timeout
            }
        }

        log::info!(
            "[Cast Discovery] Found {} {} devices",
            devices.len(),
            device_type,
        );

        Ok(devices)
    }
}

// ============================================================================
// mDNS packet construction and parsing
// ============================================================================
//
// Minimal DNS wire format implementation for mDNS queries and responses.
// Only supports the subset needed for service discovery (PTR, SRV, TXT, A).

/// Build a DNS query packet for a PTR record (service enumeration).
fn build_mdns_query(service_name: &str) -> Vec<u8> {
    let mut packet = Vec::with_capacity(128);

    // DNS header: ID=0, flags=0 (standard query), QDCOUNT=1
    packet.extend_from_slice(&[0, 0]); // ID
    packet.extend_from_slice(&[0, 0]); // Flags (standard query)
    packet.extend_from_slice(&[0, 1]); // QDCOUNT = 1
    packet.extend_from_slice(&[0, 0]); // ANCOUNT = 0
    packet.extend_from_slice(&[0, 0]); // NSCOUNT = 0
    packet.extend_from_slice(&[0, 0]); // ARCOUNT = 0

    // Question: service_name, type=PTR (12), class=IN (1) with unicast bit
    encode_dns_name(&mut packet, service_name);
    packet.extend_from_slice(&[0, 12]); // QTYPE = PTR
    packet.extend_from_slice(&[0, 1]); // QCLASS = IN

    packet
}

fn encode_dns_name(packet: &mut Vec<u8>, name: &str) {
    for label in name.split('.') {
        let len = label.len() as u8;
        packet.push(len);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0); // Root label
}

/// Parse an mDNS response and extract device info.
/// This is a simplified parser that handles the common case.
fn parse_mdns_response(
    data: &[u8],
    from: SocketAddr,
    device_type: CastDeviceType,
) -> Option<CastDevice> {
    if data.len() < 12 {
        return None;
    }

    // Check this is a response (QR bit set)
    let flags = u16::from_be_bytes([data[2], data[3]]);
    if flags & 0x8000 == 0 {
        return None; // This is a query, not a response
    }

    // DNS header: bytes 4-5=QDCOUNT, 6-7=ANCOUNT, 8-9=NSCOUNT, 10-11=ARCOUNT
    let qdcount = u16::from_be_bytes([data[4], data[5]]);
    let ancount = u16::from_be_bytes([data[6], data[7]]);
    let nscount = u16::from_be_bytes([data[8], data[9]]);
    let arcount = u16::from_be_bytes([data[10], data[11]]);
    let total_rr = ancount + nscount + arcount;

    if total_rr == 0 {
        return None;
    }

    // Parse resource records looking for SRV, TXT, and A records
    let mut pos = 12;

    // Skip question section
    for _ in 0..qdcount {
        pos = skip_dns_name(data, pos)?;
        pos += 4; // QTYPE + QCLASS
        if pos > data.len() {
            return None;
        }
    }

    let mut name = String::new();
    let mut port: u16 = 0;
    let mut address = Ipv4Addr::UNSPECIFIED;
    let mut txt_records = HashMap::new();

    let total = ancount + nscount + arcount;
    for _ in 0..total {
        if pos >= data.len() {
            break;
        }

        let _rr_name_start = pos;
        pos = skip_dns_name(data, pos)?;
        if pos + 10 > data.len() {
            break;
        }

        let rtype = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let _rclass = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
        let _ttl = u32::from_be_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
        let rdlength = u16::from_be_bytes([data[pos + 8], data[pos + 9]]) as usize;
        pos += 10;

        if pos + rdlength > data.len() {
            break;
        }

        match rtype {
            33 => {
                // SRV record: priority(2) + weight(2) + port(2) + target
                if rdlength >= 6 {
                    port = u16::from_be_bytes([data[pos + 4], data[pos + 5]]);
                }
            }
            16 => {
                // TXT record: one or more length-prefixed strings
                let mut tpos = pos;
                let end = pos + rdlength;
                while tpos < end {
                    let tlen = data[tpos] as usize;
                    tpos += 1;
                    if tpos + tlen > end {
                        break;
                    }
                    let txt = String::from_utf8_lossy(&data[tpos..tpos + tlen]);
                    if let Some((key, value)) = txt.split_once('=') {
                        txt_records.insert(key.to_string(), value.to_string());
                    }
                    tpos += tlen;
                }
                // Extract name from TXT "fn" key (Chromecast) or construct from instance
                if let Some(friendly) = txt_records.get("fn") {
                    name = friendly.clone();
                }
            }
            1 => {
                // A record: 4 bytes IPv4
                if rdlength == 4 {
                    address = Ipv4Addr::new(data[pos], data[pos + 1], data[pos + 2], data[pos + 3]);
                }
            }
            12 => {
                // PTR record: extract instance name
                if name.is_empty()
                    && let Some(decoded) = decode_dns_name(data, pos)
                {
                    // Instance name is the first label (before the service type)
                    if let Some(dot) = decoded.find('.') {
                        name = decoded[..dot].to_string();
                    } else {
                        name = decoded;
                    }
                }
            }
            _ => {} // Skip other record types
        }

        pos += rdlength;
    }

    // Fall back to sender IP if no A record
    if address == Ipv4Addr::UNSPECIFIED && let SocketAddr::V4(v4) = from {
        address = *v4.ip();
    }

    // Need at least a port to be useful
    if port == 0 {
        // AirPlay default
        if device_type == CastDeviceType::AirPlay {
            port = 7000;
        } else {
            port = 8009;
        }
    }

    if name.is_empty() {
        name = format!("{} ({})", device_type, address);
    }

    Some(CastDevice {
        device_type,
        name,
        address,
        port,
        instance_name: String::new(),
        txt_records,
    })
}

/// Skip a DNS name (handling compression pointers).
fn skip_dns_name(data: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        if pos >= data.len() {
            return None;
        }
        let len = data[pos] as usize;
        if len == 0 {
            return Some(pos + 1);
        }
        if len & 0xC0 == 0xC0 {
            // Compression pointer — 2 bytes
            return Some(pos + 2);
        }
        pos += 1 + len;
    }
}

/// Decode a DNS name at the given position (handling compression).
fn decode_dns_name(data: &[u8], mut pos: usize) -> Option<String> {
    let mut parts = Vec::new();
    let mut jumps = 0;

    loop {
        if pos >= data.len() || jumps > 10 {
            return None;
        }
        let len = data[pos] as usize;
        if len == 0 {
            break;
        }
        if len & 0xC0 == 0xC0 {
            if pos + 1 >= data.len() {
                return None;
            }
            pos = (len & 0x3F) << 8 | data[pos + 1] as usize;
            jumps += 1;
            continue;
        }
        pos += 1;
        if pos + len > data.len() {
            return None;
        }
        parts.push(String::from_utf8_lossy(&data[pos..pos + len]).to_string());
        pos += len;
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_mdns_query() {
        let query = build_mdns_query("_raop._tcp.local");
        // Header: 12 bytes
        assert_eq!(query[0..2], [0, 0]); // ID
        assert_eq!(query[4..6], [0, 1]); // QDCOUNT = 1
        // Should contain the encoded name
        assert!(query.len() > 12);
        // First label "_raop" = length 5
        assert_eq!(query[12], 5);
        assert_eq!(&query[13..18], b"_raop");
    }

    #[test]
    fn test_encode_dns_name() {
        let mut buf = Vec::new();
        encode_dns_name(&mut buf, "_raop._tcp.local");
        assert_eq!(buf[0], 5); // "_raop"
        assert_eq!(&buf[1..6], b"_raop");
        assert_eq!(buf[6], 4); // "_tcp"
        assert_eq!(&buf[7..11], b"_tcp");
        assert_eq!(buf[11], 5); // "local"
        assert_eq!(&buf[12..17], b"local");
        assert_eq!(buf[17], 0); // root
    }

    #[test]
    fn test_skip_dns_name_simple() {
        let data = [5, b'h', b'e', b'l', b'l', b'o', 0, 99];
        assert_eq!(skip_dns_name(&data, 0), Some(7));
    }

    #[test]
    fn test_skip_dns_name_compression() {
        let data = [0xC0, 0x0C]; // Compression pointer
        assert_eq!(skip_dns_name(&data, 0), Some(2));
    }

    #[test]
    fn test_decode_dns_name() {
        let mut data = Vec::new();
        encode_dns_name(&mut data, "test.local");
        let name = decode_dns_name(&data, 0).unwrap();
        assert_eq!(name, "test.local");
    }

    #[test]
    fn test_cast_device_type_display() {
        assert_eq!(CastDeviceType::AirPlay.to_string(), "AirPlay");
        assert_eq!(CastDeviceType::Chromecast.to_string(), "Chromecast");
    }

    #[test]
    fn test_cast_device_txt_helpers() {
        let mut txt = HashMap::new();
        txt.insert("am".to_string(), "AudioAccessory5,1".to_string());
        txt.insert("fn".to_string(), "Living Room".to_string());

        let device = CastDevice {
            device_type: CastDeviceType::AirPlay,
            name: "Living Room".to_string(),
            address: Ipv4Addr::new(192, 168, 1, 50),
            port: 7000,
            instance_name: String::new(),
            txt_records: txt,
        };

        assert_eq!(device.model(), Some("AudioAccessory5,1"));
        assert_eq!(device.chromecast_model(), None);
    }

    #[test]
    fn test_parse_mdns_response_rejects_query() {
        // A packet with QR=0 (query) should be rejected
        let data = [0u8; 12]; // All zeros = query
        let from: SocketAddr = "192.168.1.1:5353".parse().unwrap();
        assert!(parse_mdns_response(&data, from, CastDeviceType::AirPlay).is_none());
    }
}
