pub(super) fn endpoint_url(base_url: &str, path: &str) -> String {
    let path = path.trim_start_matches('/');
    if path.is_empty() {
        base_url.to_string()
    } else {
        format!("{base_url}/{path}")
    }
}
