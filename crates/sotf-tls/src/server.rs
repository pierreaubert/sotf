use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsAcceptor;

/// Build a `rustls::ServerConfig` from a certificate and private key.
///
/// The config accepts any client (no client certificate required).
///
/// # Errors
/// Returns an error if the certificate or key is invalid.
pub fn build_server_tls_config(
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
) -> Result<Arc<rustls::ServerConfig>, String> {
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .map_err(|e| format!("TLS server config error: {e}"))?;

    Ok(Arc::new(config))
}

/// Accept a TLS connection on a TCP stream.
///
/// Returns the negotiated TLS stream ready for async I/O.
///
/// # Errors
/// Returns an error if the TLS handshake fails.
pub async fn tls_accept(
    acceptor: &TlsAcceptor,
    stream: TcpStream,
) -> Result<tokio_rustls::server::TlsStream<TcpStream>, String> {
    acceptor
        .accept(stream)
        .await
        .map_err(|e| format!("TLS handshake failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert_gen;
    use std::net::IpAddr;

    #[test]
    fn test_build_server_config() {
        let (cert, key) = cert_gen::generate_self_signed(
            &["localhost".to_string()],
            &[IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)],
        )
        .expect("cert gen");

        let config = build_server_tls_config(cert, key);
        assert!(config.is_ok());
    }
}
