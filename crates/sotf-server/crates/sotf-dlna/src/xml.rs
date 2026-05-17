// ============================================================================
// Minimal SOAP/XML helpers for UPnP control
// ============================================================================
//
// SECURITY NOTE — XXE / DTD processing:
//
// This module performs hand-rolled SOAP parsing on UNTRUSTED input from the
// network. The parser DOES NOT interpret DTD/DOCTYPE/`<!ENTITY>` declarations,
// PEs, or external entities — `&xxx;` references that are not in the small
// predefined set (`amp lt gt quot apos` and the numeric forms `&#NN;` /
// `&#xHH;`) are returned verbatim. Classic XXE (`SYSTEM "file://"`,
// `http://` SYSTEM entities) and billion-laughs amplification therefore do
// NOT apply.
//
// If this parser is ever replaced by a real XML library (quick-xml, xmlrs,
// roxmltree, libxml2, ...), the replacement MUST be configured to:
//   1. Reject or ignore `<!DOCTYPE ...>` and any internal/external subset.
//   2. Disable entity expansion beyond the 5 predefined entities.
//   3. Disable network/file resolution of external entities.
//
// Failing to do so re-introduces XXE on a network-facing port.

/// Decode the limited set of XML entities that may legitimately appear in
/// SOAP argument text: `&amp; &lt; &gt; &quot; &apos;` and numeric character
/// references `&#NN;` / `&#xHH;`. Unknown entities are passed through
/// verbatim (defensive; controllers must not rely on them).
pub fn xml_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if let Some(semi_rel) = s[i + 1..].find(';') {
                let entity = &s[i + 1..i + 1 + semi_rel];
                let decoded: Option<char> = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    e if e.starts_with("#x") || e.starts_with("#X") => {
                        u32::from_str_radix(&e[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                    }
                    e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
                    _ => None,
                };
                if let Some(c) = decoded {
                    out.push(c);
                    i += 1 + semi_rel + 1;
                    continue;
                }
            }
            // Unknown entity or no matching ';' — preserve verbatim.
            out.push('&');
            i += 1;
        } else {
            let next = s[i..].find('&').map(|d| i + d).unwrap_or(s.len());
            out.push_str(&s[i..next]);
            i = next;
        }
    }
    out
}

/// XML-escape a string for use as element text or attribute value.
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Strip any `prefix:` from an XML local-name. Returns the original string
/// when no colon is present.
fn local_name(qname: &str) -> &str {
    match qname.find(':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    }
}

/// Locate the SOAP envelope's `Body` element (regardless of namespace
/// prefix) and return its inner content.
fn find_body_inner(body: &str) -> Option<&str> {
    let mut cursor = 0;
    let mut inner_start: Option<usize> = None;
    while let Some(rel) = body[cursor..].find('<') {
        let lt = cursor + rel;
        if body[lt..].starts_with("<!--") {
            let close = body[lt..].find("-->")?;
            cursor = lt + close + 3;
            continue;
        }
        if body[lt..].starts_with("<?") {
            let close = body[lt..].find("?>")?;
            cursor = lt + close + 2;
            continue;
        }
        if body[lt..].starts_with("<!") {
            // DOCTYPE / CDATA — skip to matching '>'.
            let close = body[lt..].find('>')?;
            cursor = lt + close + 1;
            continue;
        }
        let after = &body[lt + 1..];
        if after.starts_with('/') {
            cursor = lt + 1;
            continue;
        }
        let tag_end_rel = after.find('>')?;
        let qname_end_rel = after[..tag_end_rel]
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(tag_end_rel);
        let qname = &after[..qname_end_rel];
        if local_name(qname) == "Body" {
            inner_start = Some(lt + 1 + tag_end_rel + 1);
            break;
        }
        cursor = lt + 1 + tag_end_rel + 1;
    }
    let start = inner_start?;
    // Matching close tag: first `</...Body>` after `start`.
    let mut search = start;
    loop {
        let rel = body[search..].find("</")?;
        let cls = search + rel + 2;
        let cls_end_rel = body[cls..].find('>')?;
        let close_qname = body[cls..cls + cls_end_rel].trim();
        if local_name(close_qname) == "Body" {
            return Some(&body[start..cls - 2]);
        }
        search = cls + cls_end_rel + 1;
    }
}

/// Extract the SOAP action name and its arguments from a SOAP envelope.
///
/// Accepts any namespace prefix on `Envelope` / `Body` / action element.
/// Returns the action local name and a list of `(arg-local-name,
/// entity-decoded-value)` pairs. Returns `None` on malformed input.
pub fn extract_soap_action(body: &str) -> Option<(String, Vec<(String, String)>)> {
    let inner = find_body_inner(body)?;
    parse_action_element(inner)
}

fn parse_action_element(inner: &str) -> Option<(String, Vec<(String, String)>)> {
    let mut cursor = 0;
    let (action_name, action_inner) = loop {
        let rel = inner[cursor..].find('<')?;
        let lt = cursor + rel;
        if inner[lt..].starts_with("<!--") {
            let close = inner[lt..].find("-->")?;
            cursor = lt + close + 3;
            continue;
        }
        if inner[lt..].starts_with("<?") {
            let close = inner[lt..].find("?>")?;
            cursor = lt + close + 2;
            continue;
        }
        if inner[lt..].starts_with("</") || inner[lt..].starts_with("<!") {
            cursor = lt + 1;
            continue;
        }
        let after = &inner[lt + 1..];
        let tag_end_rel = after.find('>')?;
        let tag_inside = &after[..tag_end_rel];
        let self_closing = tag_inside.ends_with('/');
        let qname_end_rel = tag_inside
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(tag_inside.len());
        let qname = &tag_inside[..qname_end_rel];
        let local = local_name(qname).to_string();
        if self_closing {
            return Some((local, Vec::new()));
        }
        let inner_start = lt + 1 + tag_end_rel + 1;
        let close_needle_a = format!("</{}>", qname);
        let close_rel = inner[inner_start..].find(&close_needle_a).or_else(|| {
            let close_needle_b = format!("</{}", qname);
            inner[inner_start..].find(&close_needle_b).and_then(|p| {
                let after_close = &inner[inner_start + p + close_needle_b.len()..];
                after_close.find('>').map(|q| p + close_needle_b.len() + q)
            })
        })?;
        break (local, &inner[inner_start..inner_start + close_rel]);
    };

    let mut args: Vec<(String, String)> = Vec::new();
    let mut cursor = 0;
    while cursor < action_inner.len() {
        let Some(rel) = action_inner[cursor..].find('<') else {
            break;
        };
        let lt = cursor + rel;
        if action_inner[lt..].starts_with("<!--") {
            let close = action_inner[lt..].find("-->")?;
            cursor = lt + close + 3;
            continue;
        }
        if action_inner[lt..].starts_with("<![CDATA[") {
            let close = action_inner[lt..].find("]]>")?;
            cursor = lt + close + 3;
            continue;
        }
        if action_inner[lt..].starts_with("</") {
            break;
        }
        let after = &action_inner[lt + 1..];
        let tag_end_rel = after.find('>')?;
        let tag_inside = &after[..tag_end_rel];
        let self_closing = tag_inside.ends_with('/');
        let qname_end_rel = tag_inside
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(tag_inside.len());
        let qname = &tag_inside[..qname_end_rel];
        let arg_local = local_name(qname).to_string();
        if self_closing {
            args.push((arg_local, String::new()));
            cursor = lt + 1 + tag_end_rel + 1;
            continue;
        }
        let value_start = lt + 1 + tag_end_rel + 1;
        let close_needle = format!("</{}>", qname);
        let value_end_rel = action_inner[value_start..].find(&close_needle)?;
        let raw_value = &action_inner[value_start..value_start + value_end_rel];
        args.push((arg_local, xml_decode(raw_value)));
        cursor = value_start + value_end_rel + close_needle.len();
    }
    Some((action_name, args))
}

/// Build a SOAP response envelope.
pub fn soap_response(action: &str, service_type: &str, args: &[(&str, &str)]) -> String {
    let mut body = String::new();
    for (key, value) in args {
        body.push_str(&format!("      <{}>{}</{}>\n", key, value, key));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:{action}Response xmlns:u="{service_type}">
{body}    </u:{action}Response>
  </s:Body>
</s:Envelope>"#,
        action = action,
        service_type = service_type,
        body = body,
    )
}

/// Build a SOAP fault response.
pub fn soap_fault(code: u32, description: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <s:Fault>
      <faultcode>s:Client</faultcode>
      <faultstring>UPnPError</faultstring>
      <detail>
        <UPnPError xmlns="urn:schemas-upnp-org:control-1-0">
          <errorCode>{}</errorCode>
          <errorDescription>{}</errorDescription>
        </UPnPError>
      </detail>
    </s:Fault>
  </s:Body>
</s:Envelope>"#,
        code,
        xml_escape(description),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soap_response() {
        let resp = soap_response(
            "GetVolume",
            "urn:schemas-upnp-org:service:RenderingControl:1",
            &[("CurrentVolume", "75")],
        );
        assert!(resp.contains("GetVolumeResponse"));
        assert!(resp.contains("<CurrentVolume>75</CurrentVolume>"));
    }

    #[test]
    fn test_soap_fault() {
        let fault = soap_fault(402, "Invalid Args");
        assert!(fault.contains("<errorCode>402</errorCode>"));
        assert!(fault.contains("Invalid Args"));
    }

    #[test]
    fn test_extract_soap_action() {
        let soap = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <CurrentURI>http://example.com/song.flac</CurrentURI>
      <CurrentURIMetaData></CurrentURIMetaData>
    </u:SetAVTransportURI>
  </s:Body>
</s:Envelope>"#;

        let (action, args) = extract_soap_action(soap).unwrap();
        assert_eq!(action, "SetAVTransportURI");
        assert_eq!(args.len(), 3);
        assert_eq!(args[0].0, "InstanceID");
        assert_eq!(args[0].1, "0");
        assert_eq!(args[1].0, "CurrentURI");
        assert_eq!(args[1].1, "http://example.com/song.flac");
    }

    /// Review requirement: parser must accept arbitrary namespace prefixes,
    /// not just `s:` / `u:`.
    #[test]
    fn test_extract_soap_action_alternate_prefixes() {
        let soap = r#"<?xml version="1.0"?>
<SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/">
  <SOAP-ENV:Body>
    <m:Play xmlns:m="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Speed>1</Speed>
    </m:Play>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>"#;
        let (action, args) = extract_soap_action(soap).unwrap();
        assert_eq!(action, "Play");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], ("InstanceID".to_string(), "0".to_string()));
        assert_eq!(args[1], ("Speed".to_string(), "1".to_string()));
    }

    #[test]
    fn test_extract_soap_action_no_prefix() {
        let soap = r#"<?xml version="1.0"?>
<Envelope xmlns="http://schemas.xmlsoap.org/soap/envelope/">
  <Body>
    <Pause xmlns="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
    </Pause>
  </Body>
</Envelope>"#;
        let (action, args) = extract_soap_action(soap).unwrap();
        assert_eq!(action, "Pause");
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], ("InstanceID".to_string(), "0".to_string()));
    }

    /// Review requirement: SOAP arg values must be entity-decoded — a
    /// controller sending `&amp;` must yield a literal `&` to the
    /// adapter, not the raw `&amp;` text.
    #[test]
    fn test_extract_soap_action_entity_decoded() {
        let soap = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <CurrentURI>http://host/a%20b&amp;c=1&amp;d=&quot;x&quot;</CurrentURI>
      <CurrentURIMetaData>&lt;DIDL-Lite&gt;&lt;item&gt;&amp;apos;&lt;/item&gt;&lt;/DIDL-Lite&gt;</CurrentURIMetaData>
    </u:SetAVTransportURI>
  </s:Body>
</s:Envelope>"#;
        let (action, args) = extract_soap_action(soap).unwrap();
        assert_eq!(action, "SetAVTransportURI");
        let map: std::collections::HashMap<_, _> = args.into_iter().collect();
        assert_eq!(
            map.get("CurrentURI").map(String::as_str),
            Some("http://host/a%20b&c=1&d=\"x\"")
        );
        // One decode level: outer escape strips off, inner &amp;apos;
        // becomes literal &apos; (controllers escape DIDL once for SOAP).
        assert_eq!(
            map.get("CurrentURIMetaData").map(String::as_str),
            Some("<DIDL-Lite><item>&apos;</item></DIDL-Lite>")
        );
    }

    /// XXE / DOCTYPE smoke test: the parser must ignore DTD declarations
    /// and must NEVER expand external entities.
    #[test]
    fn test_extract_soap_action_doctype_ignored() {
        let soap = r#"<?xml version="1.0"?>
<!DOCTYPE Envelope [
  <!ENTITY xxe SYSTEM "file:///etc/passwd">
]>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/">
  <s:Body>
    <u:Play xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>&xxe;</InstanceID>
    </u:Play>
  </s:Body>
</s:Envelope>"#;
        let (action, args) = extract_soap_action(soap).unwrap();
        assert_eq!(action, "Play");
        // Unknown entity preserved verbatim, NEVER expanded.
        assert_eq!(args[0].1, "&xxe;");
    }

    #[test]
    fn test_xml_decode_basic() {
        assert_eq!(
            xml_decode("a&amp;b&lt;c&gt;d&quot;e&apos;f"),
            "a&b<c>d\"e'f"
        );
        assert_eq!(xml_decode("&#65;&#x42;"), "AB");
        assert_eq!(xml_decode("unknown &foo; entity"), "unknown &foo; entity");
    }

    #[test]
    fn test_xml_escape_basic() {
        assert_eq!(
            xml_escape("a<b>c&d\"e'f"),
            "a&lt;b&gt;c&amp;d&quot;e&apos;f"
        );
    }
}
