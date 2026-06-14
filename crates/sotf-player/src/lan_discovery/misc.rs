#[cfg(not(target_os = "macos"))]
use std::net::{SocketAddr, SocketAddrV4};
#[cfg(not(target_os = "macos"))]
use tokio::net::UdpSocket;

#[cfg(not(target_os = "macos"))]
pub(super) fn bind_mdns_responder_socket(bind_addr: SocketAddrV4) -> Result<UdpSocket, String> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .map_err(|e| format!("Failed to create mDNS responder socket: {e}"))?;
    socket
        .set_reuse_address(true)
        .map_err(|e| format!("Failed to enable SO_REUSEADDR for mDNS responder: {e}"))?;
    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "ios",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    ))]
    socket
        .set_reuse_port(true)
        .map_err(|e| format!("Failed to enable SO_REUSEPORT for mDNS responder: {e}"))?;
    socket
        .bind(&socket2::SockAddr::from(SocketAddr::V4(bind_addr)))
        .map_err(|e| format!("Failed to bind mDNS responder socket: {e}"))?;
    socket
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to make mDNS responder socket nonblocking: {e}"))?;
    UdpSocket::from_std(socket.into())
        .map_err(|e| format!("Failed to create Tokio mDNS responder socket: {e}"))
}

pub(super) fn encode_dns_name(packet: &mut Vec<u8>, name: &str) {
    for label in name.trim_end_matches('.').split('.') {
        let label = label.as_bytes();
        let len = label.len().min(63);
        packet.push(len as u8);
        packet.extend_from_slice(&label[..len]);
    }
    packet.push(0);
}

pub(super) fn decode_dns_name(packet: &[u8], mut pos: usize) -> Option<(String, usize)> {
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

pub(super) fn display_name_from_instance(instance_name: &str) -> String {
    instance_name
        .split_once('.')
        .map_or(instance_name, |(name, _)| name)
        .replace('-', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_name(packet: &mut Vec<u8>, name: &str) -> usize {
        let start = packet.len();
        encode_dns_name(packet, name);
        start
    }

    #[test]
    fn decode_dns_name_simple() {
        let mut packet = Vec::new();
        encode_name(&mut packet, "example.com");
        let (decoded, end) = decode_dns_name(&packet, 0).unwrap();
        assert_eq!(decoded, "example.com");
        assert_eq!(end, packet.len());
    }

    #[test]
    fn decode_dns_name_root() {
        let packet = vec![0];
        let (decoded, end) = decode_dns_name(&packet, 0).unwrap();
        assert_eq!(decoded, "");
        assert_eq!(end, 1);
    }

    #[test]
    fn decode_dns_name_with_compression_pointer() {
        let mut packet = Vec::new();
        // Encode "local" at `name_start`; the first byte is the label length.
        let name_start = encode_name(&mut packet, "local");
        encode_name(&mut packet, "_sotf._tcp");
        // Pointer back to the "local" label length byte.
        packet.push(0xC0 | (name_start >> 8) as u8);
        packet.push(name_start as u8);

        let pointer_pos = packet.len() - 2;
        let (decoded, end) = decode_dns_name(&packet, pointer_pos).unwrap();
        assert_eq!(decoded, "local");
        // Consumed position is after the 2-byte pointer.
        assert_eq!(end, pointer_pos + 2);
    }

    #[test]
    fn decode_dns_name_truncated_label_returns_none() {
        // Length byte claims 5 bytes but packet only has 3.
        let packet = vec![5, b'a', b'b', b'c'];
        assert!(decode_dns_name(&packet, 0).is_none());
    }

    #[test]
    fn decode_dns_name_compression_pointer_out_of_bounds() {
        // Pointer to offset 500 in a 4-byte packet.
        let packet = vec![0, 0, 0xC0 | 0x01, 0xF4];
        assert!(decode_dns_name(&packet, 2).is_none());
    }

    #[test]
    fn decode_dns_name_too_many_jumps() {
        let mut packet = vec![0; 30];
        for i in 0..30 {
            packet[i] = 0xC0 | ((i + 2) >> 8) as u8;
            if i + 1 < 30 {
                packet[i + 1] = (i + 2) as u8;
            }
        }
        assert!(decode_dns_name(&packet, 0).is_none());
    }

    #[test]
    fn display_name_from_instance_replaces_dashes() {
        assert_eq!(
            display_name_from_instance("Living-Room-SOTF._sotf._tcp.local"),
            "Living Room SOTF"
        );
    }

    #[test]
    fn display_name_from_instance_without_dot() {
        assert_eq!(display_name_from_instance("Kitchen-SOTF"), "Kitchen SOTF");
    }
}
