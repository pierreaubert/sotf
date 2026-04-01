// ============================================================================
// SSDP (Simple Service Discovery Protocol)
// ============================================================================
//
// Handles multicast announcements (NOTIFY) and search responses (M-SEARCH).
// SSDP uses UDP multicast on 239.255.255.250:1900.

use crate::device::DlnaDevice;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::net::UdpSocket;

const SSDP_MULTICAST: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const SSDP_PORT: u16 = 1900;

/// Send SSDP alive announcements for a device.
pub async fn send_alive(device: &DlnaDevice, local_ip: Ipv4Addr) -> Result<(), String> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|e| format!("Failed to bind SSDP socket: {}", e))?;

    let location = format!("http://{}:{}/description.xml", local_ip, device.http_port);
    let target = SocketAddr::V4(SocketAddrV4::new(SSDP_MULTICAST, SSDP_PORT));

    let notify = format!(
        "NOTIFY * HTTP/1.1\r\n\
         HOST: 239.255.255.250:1900\r\n\
         CACHE-CONTROL: max-age=1800\r\n\
         LOCATION: {location}\r\n\
         NT: {nt}\r\n\
         NTS: ssdp:alive\r\n\
         SERVER: SOTF/1.0 UPnP/1.0 SOTF-DLNA/1.0\r\n\
         USN: {usn}\r\n\
         \r\n",
        location = location,
        nt = device.device_type.urn(),
        usn = device.usn(),
    );

    socket
        .send_to(notify.as_bytes(), target)
        .await
        .map_err(|e| format!("Failed to send SSDP alive: {}", e))?;

    log::debug!(
        "[SSDP] Sent alive for {} at {}",
        device.friendly_name,
        location
    );

    Ok(())
}

/// Send SSDP byebye announcement (device going offline).
pub async fn send_byebye(device: &DlnaDevice) -> Result<(), String> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|e| format!("Failed to bind SSDP socket: {}", e))?;

    let target = SocketAddr::V4(SocketAddrV4::new(SSDP_MULTICAST, SSDP_PORT));

    let notify = format!(
        "NOTIFY * HTTP/1.1\r\n\
         HOST: 239.255.255.250:1900\r\n\
         NT: {nt}\r\n\
         NTS: ssdp:byebye\r\n\
         USN: {usn}\r\n\
         \r\n",
        nt = device.device_type.urn(),
        usn = device.usn(),
    );

    socket
        .send_to(notify.as_bytes(), target)
        .await
        .map_err(|e| format!("Failed to send SSDP byebye: {}", e))?;

    log::debug!("[SSDP] Sent byebye for {}", device.friendly_name);

    Ok(())
}

/// Listen for SSDP M-SEARCH requests and respond with device info.
pub async fn listen_and_respond(
    device: DlnaDevice,
    local_ip: Ipv4Addr,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, SSDP_PORT))
        .await
        .map_err(|e| format!("Failed to bind SSDP listener: {}", e))?;

    socket
        .join_multicast_v4(SSDP_MULTICAST, Ipv4Addr::UNSPECIFIED)
        .map_err(|e| format!("Failed to join SSDP multicast: {}", e))?;

    let location = format!("http://{}:{}/description.xml", local_ip, device.http_port);
    let mut buf = [0u8; 2048];

    log::info!(
        "[SSDP] Listening for M-SEARCH on {}:{}",
        SSDP_MULTICAST,
        SSDP_PORT
    );

    loop {
        let mut cancel = cancel.clone();
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, from)) => {
                        let msg = String::from_utf8_lossy(&buf[..len]);
                        if msg.contains("M-SEARCH") && is_matching_search(&msg, &device) {
                            let response = format!(
                                "HTTP/1.1 200 OK\r\n\
                                 CACHE-CONTROL: max-age=1800\r\n\
                                 LOCATION: {location}\r\n\
                                 ST: {st}\r\n\
                                 SERVER: SOTF/1.0 UPnP/1.0 SOTF-DLNA/1.0\r\n\
                                 USN: {usn}\r\n\
                                 \r\n",
                                location = location,
                                st = device.device_type.urn(),
                                usn = device.usn(),
                            );
                            if let Err(e) = socket.send_to(response.as_bytes(), from).await {
                                log::warn!("[SSDP] Failed to respond to M-SEARCH from {}: {}", from, e);
                            } else {
                                log::debug!("[SSDP] Responded to M-SEARCH from {}", from);
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("[SSDP] Receive error: {}", e);
                    }
                }
            }
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    log::info!("[SSDP] Listener shutting down");
                    break;
                }
            }
        }
    }

    Ok(())
}

fn is_matching_search(msg: &str, device: &DlnaDevice) -> bool {
    let st_line = msg
        .lines()
        .find(|l| l.to_lowercase().starts_with("st:"))
        .unwrap_or("");
    let st = st_line
        .split(':')
        .skip(1)
        .collect::<Vec<_>>()
        .join(":")
        .trim()
        .to_string();

    st == "ssdp:all"
        || st == "upnp:rootdevice"
        || st == device.device_type.urn()
        || st.contains(&device.uuid.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_matching_search_ssdp_all() {
        let device = DlnaDevice::new_renderer("Test", 8200);
        assert!(is_matching_search("M-SEARCH\r\nST: ssdp:all\r\n", &device));
    }

    #[test]
    fn test_is_matching_search_device_type() {
        let device = DlnaDevice::new_renderer("Test", 8200);
        let msg = format!("M-SEARCH\r\nST: {}\r\n", device.device_type.urn());
        assert!(is_matching_search(&msg, &device));
    }

    #[test]
    fn test_is_matching_search_wrong_type() {
        let device = DlnaDevice::new_renderer("Test", 8200);
        assert!(!is_matching_search(
            "M-SEARCH\r\nST: urn:schemas-upnp-org:device:MediaServer:1\r\n",
            &device
        ));
    }
}
