# sotf-tls

TLS infrastructure — self-signed certs, TOFU trust store, server/client helpers.

## Architecture

Provides zero-config TLS for SOTF's peer-to-peer networking (streaming, DLNA, MPD) using Trust-On-First-Use (TOFU) instead of a CA.

- `cert_gen.rs` — Self-signed certificate generation via rcgen, `fingerprint()` function
- `cert_store.rs` — `CertStore`: persistent certificate storage
- `tofu.rs` — `TofuStore`: Trust-On-First-Use store, `TrustedHost`, `TofuResult`
- `client.rs` — `build_client_tls_config()`, `build_client_tls_config_with_cert()`, `TofuVerifier`
- `server.rs` — `build_server_tls_config()`, `build_server_tls_config_mtls()`, `tls_accept()`

## Key Public API

- `build_server_tls_config(cert, key) -> ServerConfig` — standard TLS server (`server.rs`)
- `build_server_tls_config_mtls(cert, key, trust) -> ServerConfig` — mutual TLS (`server.rs`)
- `build_client_tls_config() -> ClientConfig` — TOFU client config (`client.rs`)
- `TofuStore` — persists trusted host fingerprints (`tofu.rs`)
- `CertStore` — stores generated certificates (`cert_store.rs`)
- `fingerprint(cert) -> String` — SHA-256 fingerprint of a certificate (`cert_gen.rs`)

## Testing

```bash
cargo test -p sotf-tls
```

## Important Notes

- Uses rustls (not OpenSSL) — pure Rust TLS
- TOFU model: first connection to a host is trusted, subsequent connections verify the same fingerprint
- Certificates are self-signed via rcgen — no CA required
- Supports mutual TLS (mTLS) for authenticated peer connections
