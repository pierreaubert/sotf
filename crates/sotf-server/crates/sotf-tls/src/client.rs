use crate::tofu::{TofuResult, TofuStore};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error, SignatureScheme};
use std::sync::{Arc, Mutex};

/// Custom certificate verifier using Trust-On-First-Use.
///
/// On first connection to an unknown host, returns an error containing the fingerprint.
/// The caller (UI layer) should catch this error, prompt the user, and call
/// `TofuStore::accept()` if the user approves.
///
/// On subsequent connections, the stored fingerprint is checked.
#[derive(Debug)]
pub struct TofuVerifier {
    store: Arc<Mutex<TofuStore>>,
}

impl TofuVerifier {
    pub fn new(store: Arc<Mutex<TofuStore>>) -> Self {
        Self { store }
    }
}

/// Error message prefix used to signal TOFU decisions to the UI layer.
pub const TOFU_UNKNOWN_PREFIX: &str = "TOFU_UNKNOWN:";
pub const TOFU_CHANGED_PREFIX: &str = "TOFU_CHANGED:";

impl ServerCertVerifier for TofuVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let fp = crate::cert_gen::fingerprint(end_entity);
        let host = server_name.to_str();

        let store = self
            .store
            .lock()
            .map_err(|e| Error::General(format!("TOFU store lock poisoned: {e}")))?;

        match store.check(&host, &fp) {
            TofuResult::Trusted => Ok(ServerCertVerified::assertion()),
            TofuResult::Unknown { fingerprint } => Err(Error::General(format!(
                "{TOFU_UNKNOWN_PREFIX}{fingerprint}"
            ))),
            TofuResult::Changed {
                old_fingerprint,
                new_fingerprint,
            } => Err(Error::General(format!(
                "{TOFU_CHANGED_PREFIX}{old_fingerprint}|{new_fingerprint}"
            ))),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        // TOFU replaces CA-chain validation, NOT signature verification.
        // We must still verify that the server possesses the private key
        // matching the certificate — otherwise an attacker who captured the
        // certificate (sent in the clear during handshake) could MITM.
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
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// Build a client TLS config that uses TOFU verification (no client certificate).
///
/// # Errors
/// This function currently always succeeds but returns `Result` for forward compatibility.
pub fn build_client_tls_config(
    tofu_store: Arc<Mutex<TofuStore>>,
) -> Result<Arc<rustls::ClientConfig>, String> {
    let verifier = Arc::new(TofuVerifier::new(tofu_store));

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

/// Build a client TLS config that uses TOFU verification AND presents a client certificate.
///
/// The client certificate is used for mutual TLS — the server verifies our identity
/// via our certificate fingerprint instead of a password.
///
/// # Errors
/// Returns an error if the client certificate or key is invalid.
pub fn build_client_tls_config_with_cert(
    tofu_store: Arc<Mutex<TofuStore>>,
    client_cert: rustls::pki_types::CertificateDer<'static>,
    client_key: rustls::pki_types::PrivateKeyDer<'static>,
) -> Result<Arc<rustls::ClientConfig>, String> {
    let verifier = Arc::new(TofuVerifier::new(tofu_store));

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(vec![client_cert], client_key)
        .map_err(|e| format!("client cert config error: {e}"))?;

    Ok(Arc::new(config))
}

/// Parse a TOFU error message to extract the decision type and fingerprint(s).
///
/// Returns `Some(TofuResult)` if the error is a TOFU decision, `None` otherwise.
#[must_use]
pub fn parse_tofu_error(error_msg: &str) -> Option<TofuResult> {
    if let Some(fp) = error_msg.strip_prefix(TOFU_UNKNOWN_PREFIX) {
        Some(TofuResult::Unknown {
            fingerprint: fp.to_string(),
        })
    } else if let Some(rest) = error_msg.strip_prefix(TOFU_CHANGED_PREFIX) {
        let parts: Vec<&str> = rest.splitn(2, '|').collect();
        (parts.len() == 2).then(|| TofuResult::Changed {
            old_fingerprint: parts[0].to_string(),
            new_fingerprint: parts[1].to_string(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tofu_error_unknown() {
        let msg = format!("{TOFU_UNKNOWN_PREFIX}AA:BB:CC");
        let result = parse_tofu_error(&msg);
        assert_eq!(
            result,
            Some(TofuResult::Unknown {
                fingerprint: "AA:BB:CC".to_string()
            })
        );
    }

    #[test]
    fn test_parse_tofu_error_changed() {
        let msg = format!("{TOFU_CHANGED_PREFIX}AA:BB|CC:DD");
        let result = parse_tofu_error(&msg);
        assert_eq!(
            result,
            Some(TofuResult::Changed {
                old_fingerprint: "AA:BB".to_string(),
                new_fingerprint: "CC:DD".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_tofu_error_not_tofu() {
        assert_eq!(parse_tofu_error("some other error"), None);
    }

    #[test]
    fn test_build_client_config() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let store = TofuStore::load(tmp.path()).expect("load");
        let store = Arc::new(Mutex::new(store));

        let config = build_client_tls_config(store);
        config.unwrap();
    }
}
