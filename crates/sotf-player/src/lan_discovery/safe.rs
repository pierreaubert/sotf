pub(super) fn safe_protocol(protocol: Option<&String>) -> String {
    match protocol.map(|p| p.trim().to_ascii_lowercase()).as_deref() {
        Some("https") => "https".to_string(),
        _ => "http".to_string(),
    }
}

pub(super) fn safe_api_path(path: Option<&String>) -> String {
    let path = path.map_or("/api/v1", |p| p.trim());
    if path.is_empty() {
        "/api/v1".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}
