use super::error::MpdError;
use super::types::MpdKv;

/// MPD response — either success with key-value lines or an error.
#[derive(Debug, Clone)]
pub enum MpdResponse {
    Ok(Vec<MpdKv>),
    Error(MpdError),
    /// For list_ok_begin mode: OK between commands.
    ListOk,
}

impl MpdResponse {
    pub fn ok() -> Self {
        MpdResponse::Ok(vec![])
    }

    pub fn ok_with(kvs: Vec<MpdKv>) -> Self {
        MpdResponse::Ok(kvs)
    }

    pub fn format(&self) -> String {
        match self {
            MpdResponse::Ok(kvs) => {
                let mut out = String::new();
                for kv in kvs {
                    out.push_str(&kv.key);
                    out.push_str(": ");
                    out.push_str(&kv.value);
                    out.push('\n');
                }
                out.push_str("OK\n");
                out
            }
            MpdResponse::Error(err) => err.format(),
            MpdResponse::ListOk => "list_OK\n".to_string(),
        }
    }
}
