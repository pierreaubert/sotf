use super::api::api_content_length;
use super::types::ApiRequest;
use super::types::SotfApiConnectionQrPayload;
use serde_json::Value;

pub(super) fn parse_api_request(buf: &[u8], header_end: usize) -> Result<ApiRequest, String> {
    let header_text = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| "request headers are not valid UTF-8".to_string())?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing request method".to_string())?;
    let path = parts
        .next()
        .ok_or_else(|| "missing request path".to_string())?;
    let version = parts
        .next()
        .ok_or_else(|| "missing HTTP version".to_string())?;

    if !version.starts_with("HTTP/1.") {
        return Err("unsupported HTTP version".to_string());
    }

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "malformed request header".to_string())?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }

    let content_length = api_content_length(&headers)?;
    let body_start = header_end;
    let body_end = body_start + content_length;
    Ok(ApiRequest {
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
        headers,
        body: buf[body_start..body_end].to_vec(),
    })
}

pub fn parse_sotf_api_connection_qr_payload(
    payload: &str,
) -> Result<SotfApiConnectionQrPayload, String> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|err| format!("invalid SOTF API QR payload JSON: {err}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "SOTF API QR payload must be a JSON object".to_string())?;

    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind != "sotf-api-connection" {
        return Err("QR code is not a SOTF API connection".to_string());
    }

    let version = object.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version != 1 {
        return Err(format!("unsupported SOTF API QR version: {version}"));
    }

    let auth = object
        .get("auth")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !auth.eq_ignore_ascii_case("bearer") {
        return Err("SOTF API QR code does not contain bearer authentication".to_string());
    }

    let url = object
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| "SOTF API QR code is missing the server URL".to_string())?
        .to_string();
    let token = object
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| "SOTF API QR code is missing the API token".to_string())?
        .to_string();
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("SOTF Player")
        .to_string();

    Ok(SotfApiConnectionQrPayload { name, url, token })
}
