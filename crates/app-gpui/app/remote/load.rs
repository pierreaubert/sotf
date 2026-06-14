#[cfg(target_os = "ios")]
use super::misc::cstring_for_keychain;
#[cfg(target_os = "macos")]
use super::misc::macos_keychain;
#[cfg(target_os = "ios")]
use super::misc::sotf_ios_keychain_load;

#[cfg(target_os = "ios")]
pub(super) fn load_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    let key = cstring_for_keychain(&server.token_secret_key(), "token key")?;
    // SAFETY: `key` is a valid NUL-terminated string for the duration of the
    // call. The Swift bridge returns either NULL or a pointer to a static
    // UTF-8 buffer that remains valid until the next load call.
    let token = unsafe { sotf_ios_keychain_load(key.as_ptr()) };
    if token.is_null() {
        return None;
    }
    // SAFETY: non-null pointer returned by the Swift bridge points at a
    // NUL-terminated UTF-8 string.
    let token = unsafe { std::ffi::CStr::from_ptr(token) }.to_str().ok()?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

#[cfg(target_os = "macos")]
pub(super) fn load_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    macos_keychain::load_token(&server.token_secret_key())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(super) fn load_persisted_remote_server_token(
    server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    match sotf_audio_player::config::load_remote_server_token(&server.token_secret_key()) {
        Ok(token) => token,
        Err(err) => {
            log::warn!("Failed to load remote server token from internal store: {err}");
            None
        }
    }
}

#[cfg(not(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "linux",
    target_os = "windows"
)))]
pub(super) fn load_persisted_remote_server_token(
    _server: &sotf_audio_player::SotfRemoteServer,
) -> Option<String> {
    None
}
