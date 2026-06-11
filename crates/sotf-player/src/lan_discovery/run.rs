#[cfg(not(target_os = "macos"))]
use super::build::build_mdns_response;
#[cfg(not(target_os = "macos"))]
use super::consts::{MDNS_MULTICAST, MDNS_PORT};
#[cfg(not(target_os = "macos"))]
use super::misc::bind_mdns_responder_socket;
use super::sotf_service_descriptor::SotfServiceDescriptor;
#[cfg(not(target_os = "macos"))]
use super::sotf_service_descriptor::mdns_query_matches;
#[cfg(target_os = "macos")]
use super::sotf_service_descriptor::platform;
use crate::federation_config::SotfApiSettings;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// Advertise the SOTF API as `_sotf._tcp` so mobile clients can discover it.
pub async fn run_sotf_lan_discovery(
    settings: SotfApiSettings,
    local_ip: Ipv4Addr,
    pairing_enabled: bool,
    cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let descriptor = SotfServiceDescriptor::with_pairing(&settings, local_ip, pairing_enabled);

    #[cfg(target_os = "macos")]
    {
        platform::run_dns_sd_registration(descriptor, cancel).await
    }

    #[cfg(not(target_os = "macos"))]
    {
        run_mdns_responder(descriptor, cancel).await
    }
}

#[cfg(not(target_os = "macos"))]
async fn run_mdns_responder(
    descriptor: SotfServiceDescriptor,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(), String> {
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MDNS_PORT);
    let socket = bind_mdns_responder_socket(bind_addr)?;
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
