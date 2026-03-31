// ============================================================================
// DLNA Device Discovery (Control Point)
// ============================================================================
//
// Discovers other DLNA MediaRenderers on the local network via SSDP M-SEARCH.
// Used to send audio to external DLNA renderers.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::net::UdpSocket;

const SSDP_MULTICAST: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const SSDP_PORT: u16 = 1900;

/// A discovered DLNA device on the network.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    /// Device friendly name (from description XML).
    pub name: String,
    /// Location URL of the device description XML.
    pub location: String,
    /// UPnP device type URN.
    pub device_type: String,
    /// Unique service name (UUID).
    pub usn: String,
    /// IP address of the device.
    pub address: SocketAddr,
}

/// DLNA device discovery via SSDP.
pub struct DlnaDiscovery;

impl DlnaDiscovery {
    /// Discover DLNA MediaRenderers on the local network.
    ///
    /// Sends an M-SEARCH multicast and collects responses for `timeout` duration.
    pub async fn discover_renderers(timeout: Duration) -> Result<Vec<DiscoveredDevice>, String> {
        Self::discover("urn:schemas-upnp-org:device:MediaRenderer:1", timeout).await
    }

    /// Discover DLNA MediaServers on the local network.
    pub async fn discover_servers(timeout: Duration) -> Result<Vec<DiscoveredDevice>, String> {
        Self::discover("urn:schemas-upnp-org:device:MediaServer:1", timeout).await
    }

    /// Discover all UPnP devices on the local network.
    pub async fn discover_all(timeout: Duration) -> Result<Vec<DiscoveredDevice>, String> {
        Self::discover("ssdp:all", timeout).await
    }

    async fn discover(
        search_target: &str,
        timeout: Duration,
    ) -> Result<Vec<DiscoveredDevice>, String> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
            .await
            .map_err(|e| format!("Failed to bind discovery socket: {}", e))?;

        let mx = timeout.as_secs().clamp(1, 5);
        let search = format!(
            "M-SEARCH * HTTP/1.1\r\n\
             HOST: 239.255.255.250:1900\r\n\
             MAN: \"ssdp:discover\"\r\n\
             MX: {mx}\r\n\
             ST: {st}\r\n\
             \r\n",
            mx = mx,
            st = search_target,
        );

        let target = SocketAddr::V4(SocketAddrV4::new(SSDP_MULTICAST, SSDP_PORT));
        socket
            .send_to(search.as_bytes(), target)
            .await
            .map_err(|e| format!("Failed to send M-SEARCH: {}", e))?;

        log::debug!("[DLNA Discovery] Sent M-SEARCH for {}", search_target);

        let mut devices = Vec::new();
        let mut buf = [0u8; 2048];
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, from))) => {
                    let response = String::from_utf8_lossy(&buf[..len]);
                    if let Some(device) = parse_search_response(&response, from) {
                        // Deduplicate by USN
                        if !devices.iter().any(|d: &DiscoveredDevice| d.usn == device.usn) {
                            log::debug!(
                                "[DLNA Discovery] Found: {} at {}",
                                device.usn,
                                device.location
                            );
                            devices.push(device);
                        }
                    }
                }
                Ok(Err(e)) => {
                    log::debug!("[DLNA Discovery] Receive error: {}", e);
                    break;
                }
                Err(_) => break, // Timeout
            }
        }

        log::info!(
            "[DLNA Discovery] Found {} devices for {}",
            devices.len(),
            search_target
        );

        Ok(devices)
    }
}

fn parse_search_response(response: &str, from: SocketAddr) -> Option<DiscoveredDevice> {
    if !response.starts_with("HTTP/1.1 200") {
        return None;
    }

    let mut location = None;
    let mut st = None;
    let mut usn = None;

    for line in response.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim().to_string();
            match key.as_str() {
                "location" => location = Some(value),
                "st" => st = Some(value),
                "usn" => usn = Some(value),
                _ => {}
            }
        }
    }

    Some(DiscoveredDevice {
        name: String::new(), // Will be filled from description XML
        location: location?,
        device_type: st?,
        usn: usn.unwrap_or_default(),
        address: from,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_search_response() {
        let response = "HTTP/1.1 200 OK\r\n\
            CACHE-CONTROL: max-age=1800\r\n\
            LOCATION: http://192.168.1.50:8200/description.xml\r\n\
            ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\
            USN: uuid:abc-123::urn:schemas-upnp-org:device:MediaRenderer:1\r\n\
            \r\n";

        let from: SocketAddr = "192.168.1.50:8200".parse().unwrap();
        let device = parse_search_response(response, from).unwrap();

        assert_eq!(
            device.location,
            "http://192.168.1.50:8200/description.xml"
        );
        assert!(device.device_type.contains("MediaRenderer"));
        assert!(device.usn.contains("abc-123"));
    }

    #[test]
    fn test_parse_non_200_response() {
        let response = "NOTIFY * HTTP/1.1\r\nNT: upnp:rootdevice\r\n";
        let from: SocketAddr = "192.168.1.50:1900".parse().unwrap();
        assert!(parse_search_response(response, from).is_none());
    }
}
