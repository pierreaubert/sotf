use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(super) struct Response {
    pub(super) success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) error: Option<String>,
}

impl Response {
    pub(super) fn ok(data: Value) -> Self {
        let mut data = data;
        sort_json_value(&mut data);
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub(super) fn ok_empty() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
        }
    }

    pub(super) fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

/// Serialize a `Response` to JSON without ever panicking.
///
/// `Response::data` can hold arbitrary client-supplied JSON (via
/// `UpdatePlugin { parameters }`, reflected back through
/// `handle_get_plugins`). A NaN / Infinity smuggled into a `Value::Number`
/// would make `serde_json::to_string` return `Err`. We must not let that
/// kill the client thread, since this runs in the IPC hot path. Fall back
/// to a static, always-serializable byte string.
pub(super) fn serialize_response_safely(response: &Response) -> String {
    match serde_json::to_string(response) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Response serialization failed: {}", e);
            // Static fallback. This string is hard-coded valid JSON and
            // matches the on-wire shape of `Response`.
            String::from(
                r#"{"success":false,"error":"internal error: response serialization failed"}"#,
            )
        }
    }
}

/// Recursively sort object keys in a JSON value so serialization order is
/// independent of the `serde_json` map implementation / feature flags.
fn sort_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            for (k, mut v) in entries {
                sort_json_value(&mut v);
                map.insert(k, v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                sort_json_value(v);
            }
        }
        _ => {}
    }
}
