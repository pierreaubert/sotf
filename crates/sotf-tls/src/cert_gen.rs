use rcgen::{CertificateParams, DnType, KeyPair, SanType};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha2::{Digest, Sha256};
use std::net::IpAddr;

/// Generate a self-signed certificate with the given hostnames and IP SANs.
///
/// Returns (DER-encoded certificate, DER-encoded private key).
///
/// # Errors
/// Returns an error if hostname validation, key generation, or certificate signing fails.
pub fn generate_self_signed(
    hostnames: &[String],
    ips: &[IpAddr],
) -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), String> {
    let mut params = CertificateParams::new(hostnames.to_vec())
        .map_err(|e| format!("invalid hostnames: {e}"))?;

    params.distinguished_name.push(DnType::CommonName, "SOTF Server");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "SOTF");

    for ip in ips {
        params.subject_alt_names.push(SanType::IpAddress(*ip));
    }

    // Valid for 365 days
    params.not_before = rcgen::date_time_ymd(
        chrono::Utc::now().format("%Y").to_string().parse().unwrap_or(2026),
        chrono::Utc::now().format("%m").to_string().parse().unwrap_or(1),
        chrono::Utc::now().format("%d").to_string().parse().unwrap_or(1),
    );
    let next_year = chrono::Utc::now() + chrono::Duration::days(365);
    params.not_after = rcgen::date_time_ymd(
        next_year.format("%Y").to_string().parse().unwrap_or(2027),
        next_year.format("%m").to_string().parse().unwrap_or(1),
        next_year.format("%d").to_string().parse().unwrap_or(1),
    );

    let key_pair = KeyPair::generate().map_err(|e| format!("key generation failed: {e}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| format!("cert generation failed: {e}"))?;

    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der =
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der().clone()));

    Ok((cert_der, key_der))
}

/// Compute the SHA-256 fingerprint of a DER-encoded certificate.
///
/// Returns a colon-separated hex string like `"AB:CD:EF:..."`.
#[must_use] 
pub fn fingerprint(cert_der: &CertificateDer<'_>) -> String {
    let digest = Sha256::digest(cert_der.as_ref());
    digest
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Enumerate all non-loopback IPv4 and IPv6 addresses on local interfaces.
#[must_use] 
pub fn local_ip_addresses() -> Vec<IpAddr> {
    let mut addrs = Vec::new();
    // Use a simple UDP socket trick to find the default route IP
    if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
        // Connect to a public DNS to determine our local IP
        if socket.connect("8.8.8.8:80").is_ok()
            && let Ok(local_addr) = socket.local_addr()
                && !local_addr.ip().is_loopback() {
                    addrs.push(local_addr.ip());
                }
    }
    // Always include loopback for local testing
    addrs.push(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    addrs.dedup();
    addrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_self_signed() {
        let (cert, key) = generate_self_signed(
            &["localhost".to_string(), "sotf.local".to_string()],
            &[IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)],
        )
        .expect("cert generation should succeed");

        assert!(!cert.as_ref().is_empty());
        match &key {
            PrivateKeyDer::Pkcs8(k) => assert!(!k.secret_pkcs8_der().is_empty()),
            _ => panic!("expected PKCS8 key"),
        }
    }

    #[test]
    fn test_fingerprint_format() {
        let (cert, _) = generate_self_signed(
            &["test.local".to_string()],
            &[],
        )
        .expect("cert generation should succeed");

        let fp = fingerprint(&cert);
        // SHA-256 = 32 bytes = 32 hex pairs + 31 colons = 95 chars
        assert_eq!(fp.len(), 95);
        assert_eq!(fp.matches(':').count(), 31);
    }

    #[test]
    fn test_local_ip_addresses() {
        let addrs = local_ip_addresses();
        assert!(!addrs.is_empty());
        assert!(addrs.contains(&IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)));
    }
}
