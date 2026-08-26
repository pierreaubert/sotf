use std::collections::BTreeMap;
use std::io::{self, Read, Write};

use thiserror::Error;

use crate::protocol::ProtocolLimits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn json(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.into(),
        }
    }

    pub fn text(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: body.into(),
        }
    }

    pub fn write_to(&self, writer: &mut impl Write, limit: usize) -> Result<(), HttpError> {
        if self.body.len() > limit {
            return Err(HttpError::ResponseTooLarge(self.body.len()));
        }
        let reason = reason_phrase(self.status);
        write!(
            writer,
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status,
            reason,
            self.content_type,
            self.body.len()
        )?;
        writer.write_all(&self.body)?;
        writer.flush()?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum HttpError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("request line is missing or too large")]
    RequestLine,
    #[error("unsupported HTTP method")]
    Method,
    #[error("unsupported HTTP version")]
    Version,
    #[error("invalid request target")]
    Target,
    #[error("headers are malformed or exceed limits")]
    Headers,
    #[error("duplicate content-length header")]
    DuplicateContentLength,
    #[error("transfer-encoding is unsupported")]
    TransferEncoding,
    #[error("content length is invalid")]
    ContentLength,
    #[error("request body of {actual} bytes exceeds limit {limit}")]
    BodyTooLarge { actual: usize, limit: usize },
    #[error("request ended before its declared body length")]
    TruncatedBody,
    #[error("request contains bytes after its declared body")]
    TrailingBytes,
    #[error("response body of {0} bytes exceeds limit")]
    ResponseTooLarge(usize),
}

pub fn parse_request_bytes(
    bytes: &[u8],
    limits: &ProtocolLimits,
) -> Result<HttpRequest, HttpError> {
    let header_end = find_header_end(bytes).ok_or(HttpError::Headers)?;
    let head = &bytes[..header_end];
    let request = parse_head(head, limits)?;
    let body = &bytes[header_end + 4..];
    finish_request(request, body, limits, true)
}

pub fn read_request(
    reader: &mut impl Read,
    limits: &ProtocolLimits,
) -> Result<HttpRequest, HttpError> {
    let mut bytes = Vec::with_capacity(1024);
    let mut byte = [0_u8; 1];
    loop {
        let count = reader.read(&mut byte)?;
        if count == 0 {
            return Err(HttpError::Headers);
        }
        bytes.push(byte[0]);
        if bytes.len() > limits.header_bytes + limits.request_line_bytes + 4 {
            return Err(HttpError::Headers);
        }
        if bytes.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let head_len = bytes.len() - 4;
    let request = parse_head(&bytes[..head_len], limits)?;
    let declared = content_length(&request)?;
    let body_limit = body_limit(&request.path, limits);
    if declared > body_limit {
        return Err(HttpError::BodyTooLarge {
            actual: declared,
            limit: body_limit,
        });
    }
    let mut body = vec![0_u8; declared];
    if let Err(error) = reader.read_exact(&mut body) {
        return if error.kind() == io::ErrorKind::UnexpectedEof {
            Err(HttpError::TruncatedBody)
        } else {
            Err(HttpError::Io(error))
        };
    }
    finish_request(request, &body, limits, false)
}

fn parse_head(head: &[u8], limits: &ProtocolLimits) -> Result<HttpRequest, HttpError> {
    if head.len() > limits.header_bytes + limits.request_line_bytes {
        return Err(HttpError::Headers);
    }
    let text = std::str::from_utf8(head).map_err(|_| HttpError::Headers)?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or(HttpError::RequestLine)?;
    if request_line.is_empty() || request_line.len() > limits.request_line_bytes {
        return Err(HttpError::RequestLine);
    }
    let mut parts = request_line.split_ascii_whitespace();
    let method = match parts.next() {
        Some("GET") => Method::Get,
        Some("POST") => Method::Post,
        _ => return Err(HttpError::Method),
    };
    let path = parts.next().ok_or(HttpError::Target)?;
    if parts.next() != Some("HTTP/1.1") || parts.next().is_some() {
        return Err(HttpError::Version);
    }
    if !path.starts_with('/')
        || path.contains('#')
        || path.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(HttpError::Target);
    }

    let mut headers = BTreeMap::new();
    let mut header_count = 0;
    let mut header_bytes = 0;
    for line in lines {
        if line.is_empty() {
            return Err(HttpError::Headers);
        }
        header_count += 1;
        header_bytes += line.len() + 2;
        if header_count > limits.header_count || header_bytes > limits.header_bytes {
            return Err(HttpError::Headers);
        }
        let (name, value) = line.split_once(':').ok_or(HttpError::Headers)?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || value.bytes().any(|byte| byte == b'\r' || byte == b'\n')
        {
            return Err(HttpError::Headers);
        }
        let name = name.to_ascii_lowercase();
        if headers
            .insert(name.clone(), value.trim().to_owned())
            .is_some()
        {
            if name == "content-length" {
                return Err(HttpError::DuplicateContentLength);
            }
            return Err(HttpError::Headers);
        }
    }
    if headers.contains_key("transfer-encoding") {
        return Err(HttpError::TransferEncoding);
    }
    Ok(HttpRequest {
        method,
        path: path.to_owned(),
        headers,
        body: Vec::new(),
    })
}

fn finish_request(
    mut request: HttpRequest,
    bytes: &[u8],
    limits: &ProtocolLimits,
    reject_trailing: bool,
) -> Result<HttpRequest, HttpError> {
    let declared = content_length(&request)?;
    let limit = body_limit(&request.path, limits);
    if declared > limit {
        return Err(HttpError::BodyTooLarge {
            actual: declared,
            limit,
        });
    }
    if bytes.len() < declared {
        return Err(HttpError::TruncatedBody);
    }
    if reject_trailing && bytes.len() > declared {
        return Err(HttpError::TrailingBytes);
    }
    request.body.extend_from_slice(&bytes[..declared]);
    Ok(request)
}

fn content_length(request: &HttpRequest) -> Result<usize, HttpError> {
    match request.header("content-length") {
        Some(value) => value.parse().map_err(|_| HttpError::ContentLength),
        None if request.method == Method::Post => Err(HttpError::ContentLength),
        None => Ok(0),
    }
}

fn body_limit(path: &str, limits: &ProtocolLimits) -> usize {
    if path.starts_with("/qa/") || path.starts_with("/fixture/") {
        limits.fixture_body_bytes
    } else {
        limits.command_body_bytes
    }
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Error",
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn request(head: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes = head.as_bytes().to_vec();
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn parses_bounded_request() {
        let bytes = request(
            "POST /action HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\n\r\n",
            b"{}",
        );
        let parsed = parse_request_bytes(&bytes, &ProtocolLimits::default()).unwrap();
        assert_eq!(parsed.method, Method::Post);
        assert_eq!(parsed.path, "/action");
        assert_eq!(parsed.body, b"{}");
    }

    #[test]
    fn rejects_duplicate_length_transfer_encoding_and_trailing_bytes() {
        let limits = ProtocolLimits::default();
        let duplicate = request(
            "POST /action HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n",
            b"{}",
        );
        assert!(matches!(
            parse_request_bytes(&duplicate, &limits),
            Err(HttpError::DuplicateContentLength)
        ));
        let chunked = request(
            "POST /action HTTP/1.1\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n",
            b"",
        );
        assert!(matches!(
            parse_request_bytes(&chunked, &limits),
            Err(HttpError::TransferEncoding)
        ));
        let trailing = request(
            "POST /action HTTP/1.1\r\nContent-Length: 2\r\n\r\n",
            b"{}extra",
        );
        assert!(matches!(
            parse_request_bytes(&trailing, &limits),
            Err(HttpError::TrailingBytes)
        ));
    }

    #[test]
    fn fixture_and_command_limits_are_distinct() {
        let limits = ProtocolLimits {
            command_body_bytes: 1,
            fixture_body_bytes: 2,
            ..ProtocolLimits::default()
        };
        let ordinary = request("POST /action HTTP/1.1\r\nContent-Length: 2\r\n\r\n", b"{}");
        assert!(matches!(
            parse_request_bytes(&ordinary, &limits),
            Err(HttpError::BodyTooLarge { limit: 1, .. })
        ));
        let fixture = request(
            "POST /qa/fixture HTTP/1.1\r\nContent-Length: 2\r\n\r\n",
            b"{}",
        );
        assert!(parse_request_bytes(&fixture, &limits).is_ok());
    }

    #[test]
    fn enforces_request_line_header_count_header_bytes_and_utf8_limits() {
        let request_line_limits = ProtocolLimits {
            request_line_bytes: 8,
            ..ProtocolLimits::default()
        };
        assert!(matches!(
            parse_request_bytes(b"GET /live HTTP/1.1\r\n\r\n", &request_line_limits),
            Err(HttpError::RequestLine)
        ));

        let count_limits = ProtocolLimits {
            header_count: 1,
            ..ProtocolLimits::default()
        };
        assert!(matches!(
            parse_request_bytes(b"GET /live HTTP/1.1\r\nA: 1\r\nB: 2\r\n\r\n", &count_limits),
            Err(HttpError::Headers)
        ));

        let byte_limits = ProtocolLimits {
            header_bytes: 4,
            ..ProtocolLimits::default()
        };
        assert!(matches!(
            parse_request_bytes(b"GET /live HTTP/1.1\r\nLong: value\r\n\r\n", &byte_limits),
            Err(HttpError::Headers)
        ));

        let invalid_utf8 = b"GET /live HTTP/1.1\r\nName: \xff\r\n\r\n";
        assert!(matches!(
            parse_request_bytes(invalid_utf8, &ProtocolLimits::default()),
            Err(HttpError::Headers)
        ));
    }

    #[test]
    fn rejects_oversized_or_truncated_bodies_before_dispatch_and_bounds_responses() {
        let limits = ProtocolLimits {
            command_body_bytes: 1,
            ..ProtocolLimits::default()
        };
        let oversized = request("POST /action HTTP/1.1\r\nContent-Length: 4096\r\n\r\n", b"");
        assert!(matches!(
            read_request(&mut Cursor::new(oversized), &limits),
            Err(HttpError::BodyTooLarge {
                actual: 4096,
                limit: 1
            })
        ));

        let truncated = request("POST /action HTTP/1.1\r\nContent-Length: 2\r\n\r\n", b"{");
        assert!(matches!(
            read_request(&mut Cursor::new(truncated), &ProtocolLimits::default()),
            Err(HttpError::TruncatedBody)
        ));

        let mut output = Vec::new();
        assert!(matches!(
            HttpResponse::text(200, "too large").write_to(&mut output, 2),
            Err(HttpError::ResponseTooLarge(_))
        ));
        assert!(output.is_empty());
    }
}
