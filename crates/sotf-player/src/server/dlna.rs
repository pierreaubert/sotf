use super::misc::get_local_ipv4;
use std::net::Ipv4Addr;

/// URL that local-network DLNA clients can use to reach the media server.
#[must_use]
pub fn dlna_server_url(port: u16) -> String {
    dlna_server_url_for_bind("0.0.0.0", port)
}

/// URL that DLNA clients can use for a configured bind address.
#[must_use]
pub fn dlna_server_url_for_bind(bind_address: &str, port: u16) -> String {
    let host = dlna_advertised_ipv4(bind_address);
    format!("http://{host}:{port}/")
}

/// IPv4 address to advertise in DLNA URLs for a configured bind address.
#[must_use]
pub fn dlna_advertised_ipv4(bind_address: &str) -> Ipv4Addr {
    bind_address
        .trim()
        .parse::<Ipv4Addr>()
        .ok()
        .filter(|ip| !ip.is_unspecified())
        .unwrap_or_else(get_local_ipv4)
}
