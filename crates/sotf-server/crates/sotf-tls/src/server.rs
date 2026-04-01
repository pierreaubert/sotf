use crate::cert_gen;
use rustls::client::danger::HandshakeSignatureValid;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, UnixTime};
use rustls::{DigitallySignedStruct, DistinguishedName, Error, SignatureScheme};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
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

/// Build a `rustls::ServerConfig` that requires mutual TLS (mTLS).
///
/// Clients must present a certificate whose SHA-256 fingerprint is in the
/// `trusted_fingerprints` set. This is the server-side equivalent of TOFU:
/// the admin pins known client fingerprints instead of relying on a CA.
///
/// # Errors
/// Returns an error if the server certificate or key is invalid.
#[allow(
    clippy::implicit_hasher,
    reason = "HashSet is stored in Arc<Mutex<>> so generalizing the hasher is impractical"
)]
pub fn build_server_tls_config_mtls(
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
    trusted_fingerprints: Arc<Mutex<HashSet<String>>>,
) -> Result<Arc<rustls::ServerConfig>, String> {
    let verifier = Arc::new(PinnedClientVerifier {
        trusted_fingerprints,
    });

    let config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![cert], key)
        .map_err(|e| format!("TLS mTLS server config error: {e}"))?;

    Ok(Arc::new(config))
}

/// Client certificate verifier that checks the client cert's SHA-256 fingerprint
/// against a set of trusted fingerprints.
#[derive(Debug)]
struct PinnedClientVerifier {
    trusted_fingerprints: Arc<Mutex<HashSet<String>>>,
}

impl rustls::server::danger::ClientCertVerifier for PinnedClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        // No CA hints — clients should present their self-signed cert unconditionally.
        &[]
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, Error> {
        let fp = cert_gen::fingerprint(end_entity);

        let trusted = self
            .trusted_fingerprints
            .lock()
            .map_err(|e| Error::General(format!("fingerprint store lock poisoned: {e}")))?;

        if trusted.contains(&fp) {
            log::debug!("[TLS] Client certificate accepted: {fp}");
            Ok(rustls::server::danger::ClientCertVerified::assertion())
        } else {
            log::warn!("[TLS] Client certificate rejected (unknown fingerprint): {fp}");
            Err(Error::General(format!(
                "client certificate not trusted: {fp}"
            )))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        let provider = rustls::crypto::CryptoProvider::get_default()
            .ok_or_else(|| Error::General("no crypto provider installed".into()))?;
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        let provider = rustls::crypto::CryptoProvider::get_default()
            .ok_or_else(|| Error::General("no crypto provider installed".into()))?;
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }
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
    use std::net::IpAddr;

    #[test]
    fn test_build_server_config() {
        let (cert, key) = cert_gen::generate_self_signed(
            &["localhost".to_string()],
            &[IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)],
        )
        .expect("cert gen");

        let config = build_server_tls_config(cert, key);
        config.unwrap();
    }
}
