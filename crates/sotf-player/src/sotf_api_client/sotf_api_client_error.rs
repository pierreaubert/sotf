use super::error::SotfApiClientError;
use super::types::SotfApiResult;

pub(super) fn normalize_base_url(base_url: String) -> SotfApiResult<String> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(SotfApiClientError::InvalidConfig(
            "base URL must start with http:// or https://".to_string(),
        ));
    }
    if base_url.ends_with("/api/v1") {
        Ok(base_url)
    } else {
        Ok(format!("{base_url}/api/v1"))
    }
}

pub(super) fn validate_api_path_segment(segment: &str) -> SotfApiResult<&str> {
    if !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_'))
    {
        Ok(segment)
    } else {
        Err(SotfApiClientError::InvalidConfig(
            "API path segment contains unsupported characters".to_string(),
        ))
    }
}
