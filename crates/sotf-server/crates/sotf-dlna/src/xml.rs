// ============================================================================
// Minimal SOAP/XML helpers for UPnP control
// ============================================================================

/// Extract the value of a SOAP action element from a SOAP envelope body.
/// Looks for `<actionName>...</actionName>` and extracts inner text of child elements.
pub fn extract_soap_action(body: &str) -> Option<(&str, Vec<(&str, &str)>)> {
    // Find the SOAP Body content
    let body_start = body
        .find("<s:Body>")
        .or_else(|| body.find("<SOAP-ENV:Body>"))?;
    let body_end = body
        .find("</s:Body>")
        .or_else(|| body.find("</SOAP-ENV:Body>"))?;
    let inner = &body[body_start..body_end];

    // Find the action element (first element inside Body with namespace prefix u:)
    let action_start = inner.find("<u:")?;
    let after_u = &inner[action_start + 3..];
    let tag_end = after_u.find([' ', '>'])?;
    let action_name = &inner[action_start + 3..action_start + 3 + tag_end];

    // Find the end of the action opening tag (skip attributes/namespace)
    let action_content_start = inner[action_start..].find('>')? + action_start + 1;
    let action_content = &inner[action_content_start..];

    // Extract arguments as key-value pairs from the action element's children
    let mut args = Vec::new();
    let mut pos = 0;
    while pos < action_content.len() {
        let rest = &action_content[pos..];
        // Stop at the closing action tag
        if rest.starts_with("</u:") || rest.starts_with("</") && rest[2..].starts_with(action_name)
        {
            break;
        }
        if rest.starts_with('<')
            && !rest.starts_with("</")
            && let Some(tag_close) = rest[1..].find('>')
        {
            let tag = &rest[1..1 + tag_close];
            let tag_name = tag.split_whitespace().next().unwrap_or(tag);
            let value_start = 1 + tag_close + 1;
            let close_tag = format!("</{}>", tag_name);
            if let Some(value_end_rel) = rest[value_start..].find(&close_tag) {
                let value = &action_content[pos + value_start..pos + value_start + value_end_rel];
                args.push((tag_name, value));
                pos += value_start + value_end_rel + close_tag.len();
                continue;
            }
        }
        pos += 1;
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
        code, description,
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
        assert_eq!(args[0], ("InstanceID", "0"));
        assert_eq!(args[1], ("CurrentURI", "http://example.com/song.flac"));
    }
}
