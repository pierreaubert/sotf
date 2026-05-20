# sotf-tls

TLS infrastructure for SOTF — self-signed certs, TOFU trust store, server/client helpers.

## Overview

Provides zero-configuration TLS encryption for SOTF's peer-to-peer networking. Uses a Trust-On-First-Use (TOFU) model — like SSH — so devices automatically establish encrypted connections without requiring a certificate authority.

## Features

- **Self-signed certificates**: Automatic certificate generation via rcgen
- **TOFU trust model**: First-use trust with fingerprint verification on subsequent connections
- **Mutual TLS**: Optional client certificate authentication
- **Pure Rust**: Built on rustls — no OpenSSL dependency
- **Persistent trust store**: Remembers trusted hosts across restarts

## Usage

### Server

```rust
use sotf_tls::{build_server_tls_config, CertStore};

let store = CertStore::load_or_generate("./certs")?;
let tls_config = build_server_tls_config(&store.cert, &store.key)?;
```

### Client

```rust
use sotf_tls::{build_client_tls_config, TofuStore};

let tofu = TofuStore::load("./trust.toml")?;
let tls_config = build_client_tls_config()?;
```

## Architecture

```
src/
├── lib.rs         # Re-exports
├── cert_gen.rs    # Certificate generation + fingerprinting
├── cert_store.rs  # Persistent certificate storage
├── tofu.rs        # Trust-On-First-Use store
├── client.rs      # TLS client config builders
└── server.rs      # TLS server config builders
```

## Dependencies

- `rustls` — TLS implementation
- `tokio-rustls` — Async TLS
- `rcgen` — Certificate generation
- `sha2` — Fingerprint hashing
- `serde` / `toml` — Trust store serialization

## Testing

```bash
cargo test -p sotf-tls
cargo check -p sotf-tls && cargo clippy -p sotf-tls
```

## License

See the root workspace `LICENSE` file.
