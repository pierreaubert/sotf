use crate::xml;

pub(super) fn http_response(status: u16, content_type: &str, body: &str) -> String {
    http_response_with_headers(status, content_type, &[], body)
}

pub(super) fn http_response_with_headers(
    status: u16,
    content_type: &str,
    extra_headers: &[(String, String)],
    body: &str,
) -> String {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        412 => "Precondition Failed",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let extra_headers = extra_headers
        .iter()
        .map(|(name, value)| format!("{}: {}\r\n", name, value))
        .collect::<String>();
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
        status,
        status_text,
        content_type,
        body.len(),
        extra_headers,
        body,
    )
}

pub(super) fn http_soap_response(soap_body: &str) -> String {
    http_response(200, "text/xml; charset=\"utf-8\"", soap_body)
}

pub(super) fn http_soap_fault(code: u32, description: &str) -> String {
    let fault = xml::soap_fault(code, description);
    http_response(500, "text/xml; charset=\"utf-8\"", &fault)
}
