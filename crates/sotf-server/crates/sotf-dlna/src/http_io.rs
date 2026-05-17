// ============================================================================
// Hardened HTTP request reading
// ============================================================================
//
// Shared helpers used by both the renderer and the media-server HTTP
// listeners. These exist to centralise the size caps reviewed in
// `reviews/review-sotf-dlna.md`:
//
//   * Per-header line length is capped at `MAX_HEADER_LINE` bytes.
//   * Total number of header lines is capped at `MAX_HEADERS`.
//   * `Content-Length` is parsed BEFORE allocating the body buffer and
//     capped at `MAX_BODY` so a hostile peer cannot make us allocate
//     gigabytes of zeroed Vec by sending `Content-Length: 999999999`.
//   * A read timeout bounds slow-loris style attacks on header reads.

use tokio::io::{AsyncRead, AsyncReadExt};

/// Maximum number of bytes per HTTP header line.
pub const MAX_HEADER_LINE: usize = 8 * 1024;

/// Maximum number of header lines (excluding the request line and the
/// terminating empty line).
pub const MAX_HEADERS: usize = 100;

/// Maximum SOAP / control body size. Real SOAP control payloads are far
/// smaller; we keep some headroom for embedded DIDL-Lite metadata.
pub const MAX_BODY: usize = 64 * 1024;

/// Maximum time we will wait for the request line + headers + body.
pub const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug)]
pub struct ParsedRequest {
    pub method: String,
    pub path: String,
    /// All headers, with names lower-cased. Currently unused by callers
    /// but kept so future endpoint logic can inspect e.g. `range`,
    /// `connection`, `user-agent` without re-reading the stream.
    #[allow(dead_code)]
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Read an HTTP request line + headers + body from `reader`, applying the
/// size caps above. Returns an error string suitable for `log::debug!` on
/// any violation.
pub async fn read_http_request<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<ParsedRequest, String> {
    let mut request_line = String::new();
    let n = tokio::time::timeout(
        READ_TIMEOUT,
        read_line_capped(reader, &mut request_line, MAX_HEADER_LINE),
    )
    .await
    .map_err(|_| "read timeout (request line)".to_string())??;
    if n == 0 {
        return Err("empty request".to_string());
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("malformed request line".to_string());
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    let mut headers: Vec<(String, String)> = Vec::new();
    let mut content_length: usize = 0;
    loop {
        if headers.len() >= MAX_HEADERS {
            return Err(format!("too many headers (> {})", MAX_HEADERS));
        }
        let mut line = String::new();
        tokio::time::timeout(
            READ_TIMEOUT,
            read_line_capped(reader, &mut line, MAX_HEADER_LINE),
        )
        .await
        .map_err(|_| "read timeout (header)".to_string())??;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let mut split = trimmed.splitn(2, ':');
        let key = match split.next() {
            Some(k) => k.trim().to_string(),
            None => continue,
        };
        let value = split.next().map(|v| v.trim().to_string()).unwrap_or_default();
        let key_lc = key.to_ascii_lowercase();
        if key_lc == "content-length" {
            // Parse early so we can reject before allocating the body.
            content_length = value.parse::<usize>().unwrap_or(0);
        }
        headers.push((key_lc, value));
    }

    if content_length > MAX_BODY {
        return Err(format!(
            "Content-Length {} exceeds cap {}",
            content_length, MAX_BODY
        ));
    }

    let mut body = Vec::new();
    if content_length > 0 {
        // Now safe to allocate — we know the cap was honoured.
        body.resize(content_length, 0);
        tokio::time::timeout(READ_TIMEOUT, reader.read_exact(&mut body))
            .await
            .map_err(|_| "read timeout (body)".to_string())?
            .map_err(|e| e.to_string())?;
    }

    Ok(ParsedRequest {
        method,
        path,
        headers,
        body,
    })
}

/// `read_line` variant that aborts when the line exceeds `cap` bytes
/// (preventing `read_line` from buffering gigabytes into a single `String`
/// while waiting for a newline that never arrives).
async fn read_line_capped<R: AsyncRead + Unpin>(
    reader: &mut R,
    line: &mut String,
    cap: usize,
) -> Result<usize, String> {
    // Read byte-by-byte but use a small intermediate vec to avoid the
    // cost of one syscall per char — `BufReader` already buffers, so this
    // is a simple loop over the internal buffer.
    let mut buf = [0u8; 1];
    let mut read = 0;
    loop {
        let n = reader.read(&mut buf).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(read);
        }
        read += n;
        if read > cap {
            return Err(format!("header line exceeds cap {} bytes", cap));
        }
        line.push(buf[0] as char);
        if buf[0] == b'\n' {
            return Ok(read);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    async fn parse(payload: &[u8]) -> Result<ParsedRequest, String> {
        let mut reader = Cursor::new(payload.to_vec());
        read_http_request(&mut reader).await
    }

    #[tokio::test]
    async fn reads_simple_request() {
        let payload = b"POST /AVTransport/control HTTP/1.1\r\n\
                       Content-Length: 5\r\n\
                       Content-Type: text/xml\r\n\
                       \r\n\
                       hello";
        let req = parse(payload).await.unwrap();
        assert_eq!(req.method, "POST");
        assert_eq!(req.path, "/AVTransport/control");
        assert_eq!(req.body, b"hello");
    }

    /// Review requirement: refuse requests whose `Content-Length` is over
    /// the body cap — we must NOT pre-allocate `vec![0u8; content_length]`
    /// before checking.
    #[tokio::test]
    async fn rejects_oversize_content_length() {
        let payload = format!(
            "POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        );
        let err = parse(payload.as_bytes()).await.unwrap_err();
        assert!(err.contains("exceeds cap"), "got: {}", err);
    }

    /// Review requirement: cap each header line at `MAX_HEADER_LINE` bytes
    /// so a peer cannot OOM us with a multi-gigabyte header line lacking
    /// a `\n`.
    #[tokio::test]
    async fn rejects_oversize_header_line() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"POST / HTTP/1.1\r\n");
        // One header line longer than the cap, no `\r\n` until well past
        // the cap.
        payload.extend_from_slice(b"X-Huge: ");
        payload.extend(std::iter::repeat_n(b'A', MAX_HEADER_LINE + 16));
        payload.extend_from_slice(b"\r\n\r\n");
        let err = parse(&payload).await.unwrap_err();
        assert!(err.contains("header line exceeds cap"), "got: {}", err);
    }

    /// Review requirement: cap total header count at `MAX_HEADERS`.
    #[tokio::test]
    async fn rejects_too_many_headers() {
        let mut payload = String::new();
        payload.push_str("POST / HTTP/1.1\r\n");
        for i in 0..(MAX_HEADERS + 1) {
            payload.push_str(&format!("X-H-{}: v\r\n", i));
        }
        payload.push_str("\r\n");
        let err = parse(payload.as_bytes()).await.unwrap_err();
        assert!(err.contains("too many headers"), "got: {}", err);
    }
}
