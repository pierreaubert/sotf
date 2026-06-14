use super::misc::peer_fingerprints_match;
use super::misc::resolve_dns;
use super::sotf::sotf_peer_client;
use crate::federation_config::{
    ConnectionDiagnostic, ConnectionStatus, FederationSourceEntry, SourceConnectionConfig,
    StepResult,
};

/// Run a structured diagnostic test against a federation source.
/// Tests each layer: DNS -> TCP -> TLS/Protocol.
/// Blocking — creates its own tokio runtime.
pub fn run_connection_diagnostic(source: &FederationSourceEntry) -> ConnectionStatus {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        match &source.connection {
            SourceConnectionConfig::Mpd {
                host,
                port,
                password,
                ..
            } => {
                let diag = diagnose_mpd(host, *port, password.as_deref()).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Subsonic { url, .. } => {
                let diag = diagnose_tcp_from_url(url).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Dlna { location_url, .. } => {
                if let Some(url) = location_url {
                    let diag = diagnose_tcp_from_url(url).await;
                    ConnectionStatus::Diagnostic(diag)
                } else {
                    ConnectionStatus::Error("No DLNA location configured".to_string())
                }
            }
            SourceConnectionConfig::Peer {
                host,
                port,
                accepted_fingerprint,
                auth_token,
            } => {
                let diag = diagnose_sotf_peer(
                    host,
                    *port,
                    accepted_fingerprint.as_deref(),
                    auth_token.as_deref(),
                )
                .await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Tidal { .. } => {
                let diag = diagnose_tcp_simple("api.tidal.com", 443).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::Spotify { .. } => {
                let diag = diagnose_tcp_simple("ap.spotify.com", 443).await;
                ConnectionStatus::Diagnostic(diag)
            }
            SourceConnectionConfig::IcyRadio { url, .. } => {
                if url.is_empty() {
                    ConnectionStatus::Error("No stream URL configured".to_string())
                } else {
                    let diag = diagnose_tcp_from_url(url).await;
                    ConnectionStatus::Diagnostic(diag)
                }
            }
        }
    })
}

async fn diagnose_sotf_peer(
    host: &str,
    port: u16,
    accepted_fingerprint: Option<&str>,
    auth_token: Option<&str>,
) -> ConnectionDiagnostic {
    let timeout = std::time::Duration::from_secs(5);

    let dns_resolve = match resolve_dns(host, port, timeout).await {
        Ok(r) => r,
        Err(diag) => return diag,
    };

    let tcp_connect = match tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("{host}:{port}")),
    )
    .await
    {
        Ok(Ok(_)) => StepResult::Ok(format!("port {port} open")),
        Ok(Err(e)) => StepResult::Fail(format!("{e}")),
        Err(_) => StepResult::Fail("connection timed out".to_string()),
    };

    if !tcp_connect.is_ok() {
        return ConnectionDiagnostic {
            host: host.to_string(),
            port,
            dns_resolve,
            tcp_connect,
            tls_handshake: StepResult::Skipped("TCP failed".to_string()),
            protocol_hello: StepResult::Skipped("TCP failed".to_string()),
        };
    }

    let tls_handshake = StepResult::Skipped("SOTF API uses HTTP bearer auth".to_string());
    let protocol_hello =
        diagnose_sotf_peer_protocol(host, port, accepted_fingerprint, auth_token).await;

    ConnectionDiagnostic {
        host: host.to_string(),
        port,
        dns_resolve,
        tcp_connect,
        tls_handshake,
        protocol_hello,
    }
}

async fn diagnose_sotf_peer_protocol(
    host: &str,
    port: u16,
    accepted_fingerprint: Option<&str>,
    auth_token: Option<&str>,
) -> StepResult {
    let token = auth_token.unwrap_or("").trim();
    let client = match sotf_peer_client(
        host,
        port,
        if token.is_empty() {
            "diagnostic"
        } else {
            token
        },
    ) {
        Ok(client) => client,
        Err(e) => return StepResult::Fail(format!("invalid SOTF API URL: {e}")),
    };

    let discovery = match client.discovery().await {
        Ok(discovery) if discovery.service == "sotf" => discovery,
        Ok(discovery) => {
            return StepResult::Fail(format!(
                "unexpected service '{}' at SOTF API endpoint",
                discovery.service
            ));
        }
        Err(e) => return StepResult::Fail(format!("SOTF API discovery failed: {e}")),
    };

    if let Some(expected) = accepted_fingerprint
        .map(str::trim)
        .filter(|fingerprint| !fingerprint.is_empty())
    {
        match client.pairing_status().await {
            Ok(status) if peer_fingerprints_match(expected, &status.server_fingerprint) => {}
            Ok(status) => {
                return StepResult::Fail(format!(
                    "server fingerprint mismatch: expected {expected}, got {}",
                    status.server_fingerprint
                ));
            }
            Err(e) => return StepResult::Fail(format!("fingerprint check failed: {e}")),
        }
    }

    if token.is_empty() {
        return StepResult::Fail(format!(
            "SOTF API {} reachable; API token required",
            discovery.version
        ));
    }

    match client.state().await {
        Ok(state) => StepResult::Ok(format!(
            "SOTF API {} — {} albums, {} tracks",
            discovery.version, state.library.albums, state.library.tracks
        )),
        Err(e) => StepResult::Fail(format!("SOTF API auth failed: {e}")),
    }
}

/// Diagnose an MPD connection: DNS -> TCP -> MPD greeting + optional auth.
async fn diagnose_mpd(host: &str, port: u16, password: Option<&str>) -> ConnectionDiagnostic {
    let timeout = std::time::Duration::from_secs(5);

    let dns_resolve = match resolve_dns(host, port, timeout).await {
        Ok(r) => r,
        Err(diag) => return diag,
    };

    let tcp_connect = match tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("{host}:{port}")),
    )
    .await
    {
        Ok(Ok(stream)) => {
            let tcp_result = StepResult::Ok(format!("port {port} open"));
            let tls_handshake = StepResult::Skipped("MPD uses plain TCP".to_string());

            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let mut reader = BufReader::new(stream);
            let mut greeting = String::new();
            let protocol_hello =
                match tokio::time::timeout(timeout, reader.read_line(&mut greeting)).await {
                    Ok(Ok(_)) if greeting.starts_with("OK MPD") => {
                        let version = greeting.trim().trim_start_matches("OK MPD ").to_string();
                        if let Some(pw) = password {
                            let escaped = pw.replace('\\', "\\\\").replace('"', "\\\"");
                            let cmd = format!("password \"{escaped}\"\n");
                            let inner = reader.into_inner();
                            let mut writer = tokio::io::BufWriter::new(inner);
                            if writer.write_all(cmd.as_bytes()).await.is_err()
                                || writer.flush().await.is_err()
                            {
                                StepResult::Fail(format!("MPD {version} — auth send failed"))
                            } else {
                                let mut r2 = tokio::io::BufReader::new(writer.into_inner());
                                let mut resp = String::new();
                                match tokio::time::timeout(timeout, r2.read_line(&mut resp)).await {
                                    Ok(Ok(_)) if resp.starts_with("OK") => {
                                        StepResult::Ok(format!("MPD {version} — authenticated"))
                                    }
                                    Ok(Ok(_)) => StepResult::Fail(format!(
                                        "MPD {version} — auth rejected: {}",
                                        resp.trim()
                                    )),
                                    _ => StepResult::Fail(format!(
                                        "MPD {version} — auth response failed"
                                    )),
                                }
                            }
                        } else {
                            StepResult::Ok(format!("MPD {version}"))
                        }
                    }
                    Ok(Ok(_)) => {
                        StepResult::Fail(format!("unexpected greeting: {}", greeting.trim()))
                    }
                    Ok(Err(e)) => StepResult::Fail(format!("read error: {e}")),
                    Err(_) => StepResult::Fail("greeting timed out".to_string()),
                };

            return ConnectionDiagnostic {
                host: host.to_string(),
                port,
                dns_resolve,
                tcp_connect: tcp_result,
                tls_handshake,
                protocol_hello,
            };
        }
        Ok(Err(e)) => StepResult::Fail(format!("{e}")),
        Err(_) => StepResult::Fail("connection timed out".to_string()),
    };

    ConnectionDiagnostic {
        host: host.to_string(),
        port,
        dns_resolve,
        tcp_connect,
        tls_handshake: StepResult::Skipped("TCP failed".to_string()),
        protocol_hello: StepResult::Skipped("TCP failed".to_string()),
    }
}

/// Diagnose a plain TCP connection (DNS + TCP only).
async fn diagnose_tcp_simple(host: &str, port: u16) -> ConnectionDiagnostic {
    let timeout = std::time::Duration::from_secs(5);

    let dns_resolve = match resolve_dns(host, port, timeout).await {
        Ok(r) => r,
        Err(diag) => return diag,
    };

    let tcp_connect = match tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(format!("{host}:{port}")),
    )
    .await
    {
        Ok(Ok(_)) => StepResult::Ok(format!("port {port} open")),
        Ok(Err(e)) => StepResult::Fail(format!("{e}")),
        Err(_) => StepResult::Fail("connection timed out".to_string()),
    };

    let protocol_hello = if tcp_connect.is_ok() {
        StepResult::Ok("TCP reachable".to_string())
    } else {
        StepResult::Skipped("TCP failed".to_string())
    };

    ConnectionDiagnostic {
        host: host.to_string(),
        port,
        dns_resolve,
        tcp_connect,
        tls_handshake: StepResult::Skipped("not tested".to_string()),
        protocol_hello,
    }
}

/// Diagnose connectivity to a URL by extracting host:port and doing DNS + TCP.
async fn diagnose_tcp_from_url(url: &str) -> ConnectionDiagnostic {
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let host_port = stripped.split('/').next().unwrap_or(stripped);
    let (host, port) = if let Some((h, p)) = host_port.rsplit_once(':') {
        (h.to_string(), p.parse().unwrap_or(80))
    } else {
        (
            host_port.to_string(),
            if url.starts_with("https") { 443 } else { 80 },
        )
    };
    diagnose_tcp_simple(&host, port).await
}
