use crate::federation_config::ServerConfig;

pub(super) fn generate_pairing_nonce() -> String {
    let bytes: [u8; 16] = rand::random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn generate_api_auth_token() -> String {
    let bytes: [u8; 32] = rand::random();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn ensure_sotf_api_connection_config(config: &mut ServerConfig) -> bool {
    let mut changed = false;
    if !config.api.enabled {
        config.api.enabled = true;
        changed = true;
    }
    if config
        .api
        .auth_token
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        config.api.auth_token = Some(generate_api_auth_token());
        changed = true;
    }
    changed
}
