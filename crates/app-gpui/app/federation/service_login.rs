//! Tidal/Spotify login and logout flows for the federation settings screen.
//!
//! All provider calls are blocking network I/O, so they run on dedicated
//! background threads (same pattern as `scan_federation_source`) that report
//! back over `std::sync::mpsc` channels. The UI thread drains those channels
//! from the per-tick [`App::update_service_logins`] call — no provider call
//! ever blocks the UI thread.
//!
//! Token persistence goes through the shared `sotf_audio_player::service_login`
//! helpers plus the usual `save_federation_source` path, so the TUI and GPUI
//! stay in lockstep.

use crate::app::App;
use crate::app::types::ToastMessage;

#[cfg(feature = "spotify")]
use crate::app::state::app::{SpotifyLoginMessage, SpotifyLoginState};
#[cfg(feature = "tidal")]
use crate::app::state::app::{TidalLoginMessage, TidalLoginPrompt, TidalLoginState};

/// How often the Tidal device-code worker polls the token endpoint.
#[cfg(feature = "tidal")]
const TIDAL_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

impl App {
    /// Drain the login workers' channels. Called every tick; cheap no-op when
    /// no login flow is active.
    #[cfg(any(feature = "tidal", feature = "spotify"))]
    pub fn update_service_logins(&mut self) {
        #[cfg(feature = "tidal")]
        self.update_tidal_login();
        #[cfg(feature = "spotify")]
        self.update_spotify_login();
    }

    // -----------------------------------------------------------------
    // Tidal device-code login
    // -----------------------------------------------------------------

    /// Start the Tidal device-code login flow for the given source.
    #[cfg(feature = "tidal")]
    pub fn start_tidal_login(&mut self, index: usize) {
        use sotf_audio_player::federation_config::SourceConnectionConfig;

        if self.federation.tidal_login.is_some() {
            self.ui_state.toast_message = Some(ToastMessage::warning(
                "A Tidal login is already in progress.",
            ));
            return;
        }

        let Some(source) = self.federation.sources.get(index) else {
            return;
        };
        let SourceConnectionConfig::Tidal {
            client_id,
            country_code,
            ..
        } = &source.connection
        else {
            return;
        };
        let source_id = source.source_id.clone();
        let client_id = client_id.clone();
        let country_code = country_code.clone();

        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("tidal-device-login".into())
            .spawn(move || tidal_device_login_worker(&client_id, &country_code, &tx));
        if let Err(e) = spawned {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start Tidal login: {e}"
            )));
            return;
        }

        self.federation.tidal_login = Some(TidalLoginState {
            source_id,
            receiver: rx,
            prompt: None,
        });
    }

    /// Cancel a running Tidal login flow (the worker thread exits on its own
    /// once the code expires; dropping the receiver makes its sends no-ops).
    #[cfg(feature = "tidal")]
    pub fn cancel_tidal_login(&mut self) {
        self.federation.tidal_login = None;
    }

    /// Drain Tidal login messages; apply tokens and persist on completion.
    #[cfg(feature = "tidal")]
    fn update_tidal_login(&mut self) {
        let (messages, disconnected) = {
            let Some(login) = &mut self.federation.tidal_login else {
                return;
            };
            let mut messages = Vec::new();
            let mut disconnected = false;
            loop {
                match login.receiver.try_recv() {
                    Ok(message) => messages.push(message),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            (messages, disconnected)
        };

        let mut finished = false;
        for message in messages {
            match message {
                TidalLoginMessage::Prompt {
                    verification_url,
                    user_code,
                    expires_in_secs,
                } => {
                    if let Some(login) = &mut self.federation.tidal_login {
                        login.prompt = Some(TidalLoginPrompt {
                            verification_url,
                            user_code,
                            expires_in_secs,
                        });
                    }
                }
                TidalLoginMessage::Completed {
                    access_token,
                    refresh_token,
                } => {
                    let source_id = self
                        .federation
                        .tidal_login
                        .as_ref()
                        .map(|login| login.source_id.clone());
                    if let Some(source_id) = source_id {
                        self.complete_tidal_login(&source_id, &access_token, &refresh_token);
                    }
                    finished = true;
                }
                TidalLoginMessage::Expired => {
                    self.ui_state.toast_message = Some(ToastMessage::warning(
                        "Tidal login code expired; please try again.",
                    ));
                    finished = true;
                }
                TidalLoginMessage::Failed(error) => {
                    self.ui_state.toast_message =
                        Some(ToastMessage::error(format!("Tidal login failed: {error}")));
                    finished = true;
                }
            }
        }

        // A dead worker that never reported a terminal message means the
        // thread panicked; do not leave the UI stuck in "waiting".
        if disconnected && !finished {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Tidal login worker stopped unexpectedly.",
            ));
            finished = true;
        }

        if finished {
            self.federation.tidal_login = None;
        }
    }

    /// Persist freshly-issued device-auth tokens into the source config (both
    /// the in-memory list the settings screen renders and the music database).
    #[cfg(feature = "tidal")]
    fn complete_tidal_login(&mut self, source_id: &str, access_token: &str, refresh_token: &str) {
        let Some(index) = self
            .federation
            .sources
            .iter()
            .position(|s| s.source_id == source_id)
        else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Tidal login succeeded, but the source was removed before it could be saved.",
            ));
            return;
        };

        let previous = self.federation.sources[index].clone();
        if !sotf_audio_player::apply_tidal_device_tokens(
            &mut self.federation.sources[index],
            access_token,
            refresh_token,
        ) {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Tidal login succeeded, but the source is no longer a Tidal source.",
            ));
            return;
        }

        if self.save_federation_source_or_revert(index, previous, "save Tidal login") {
            self.ui_state.toast_message = Some(ToastMessage::success("Tidal login successful."));
            sotf_audio_player::reset_service_sessions();
        }
    }

    /// Log out of Tidal: clear both tokens from the source and persist.
    #[cfg(feature = "tidal")]
    pub fn tidal_logout(&mut self, index: usize) {
        if index >= self.federation.sources.len() {
            return;
        }

        // A running login flow for this source is obsolete now.
        if self
            .federation
            .tidal_login
            .as_ref()
            .is_some_and(|l| l.source_id == self.federation.sources[index].source_id)
        {
            self.federation.tidal_login = None;
        }

        let previous = self.federation.sources[index].clone();
        if !sotf_audio_player::clear_tidal_tokens(&mut self.federation.sources[index]) {
            return;
        }

        if self.save_federation_source_or_revert(index, previous, "log out of Tidal") {
            self.ui_state.toast_message = Some(ToastMessage::success("Logged out of Tidal."));
            sotf_audio_player::reset_service_sessions();
        }
    }

    // -----------------------------------------------------------------
    // Spotify OAuth login
    // -----------------------------------------------------------------

    /// Start the Spotify OAuth (PKCE) login flow for the given source. The
    /// worker opens the system browser itself; the UI also shows the URL as a
    /// fallback once the worker reports it.
    #[cfg(feature = "spotify")]
    pub fn start_spotify_login(&mut self, index: usize) {
        use sotf_audio_player::federation_config::SourceConnectionConfig;

        if self.federation.spotify_login.is_some() {
            self.ui_state.toast_message = Some(ToastMessage::warning(
                "A Spotify login is already in progress.",
            ));
            return;
        }

        let Some(source) = self.federation.sources.get(index) else {
            return;
        };
        if !matches!(source.connection, SourceConnectionConfig::Spotify { .. }) {
            return;
        }
        let source_id = source.source_id.clone();

        let Some(cache_dir) = sotf_audio_player::service_login::spotify_cache_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not determine the config directory for Spotify credentials.",
            ));
            return;
        };

        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("spotify-oauth-login".into())
            .spawn(move || spotify_oauth_login_worker(cache_dir, tx));
        if let Err(e) = spawned {
            self.ui_state.toast_message = Some(ToastMessage::error(format!(
                "Failed to start Spotify login: {e}"
            )));
            return;
        }

        self.federation.spotify_login = Some(SpotifyLoginState {
            source_id,
            receiver: rx,
            authorize_url: None,
        });
    }

    /// Cancel a running Spotify login flow. The worker keeps waiting on the
    /// loopback listener until its own timeout, but its result is ignored.
    #[cfg(feature = "spotify")]
    pub fn cancel_spotify_login(&mut self) {
        self.federation.spotify_login = None;
    }

    /// Drain Spotify login messages.
    #[cfg(feature = "spotify")]
    fn update_spotify_login(&mut self) {
        let Some(login) = &mut self.federation.spotify_login else {
            return;
        };

        let mut messages = Vec::new();
        let mut disconnected = false;
        loop {
            match login.receiver.try_recv() {
                Ok(message) => messages.push(message),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }

        let mut finished = false;
        for message in messages {
            match message {
                SpotifyLoginMessage::AuthorizeUrl(url) => {
                    login.authorize_url = Some(url);
                }
                SpotifyLoginMessage::Completed => {
                    self.ui_state.toast_message =
                        Some(ToastMessage::success("Spotify login successful."));
                    sotf_audio_player::reset_service_sessions();
                    finished = true;
                }
                SpotifyLoginMessage::Failed(error) => {
                    self.ui_state.toast_message = Some(ToastMessage::error(format!(
                        "Spotify login failed: {error}"
                    )));
                    finished = true;
                }
            }
        }

        if disconnected && !finished {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Spotify login worker stopped unexpectedly.",
            ));
            finished = true;
        }

        if finished {
            self.federation.spotify_login = None;
        }
    }

    /// Log out of Spotify: delete the cached librespot credentials.
    #[cfg(feature = "spotify")]
    pub fn spotify_logout(&mut self, index: usize) {
        use sotf_audio_player::federation_config::SourceConnectionConfig;

        let Some(source) = self.federation.sources.get(index) else {
            return;
        };
        if !matches!(source.connection, SourceConnectionConfig::Spotify { .. }) {
            return;
        }

        // A running login flow for this source is obsolete now.
        if self
            .federation
            .spotify_login
            .as_ref()
            .is_some_and(|l| l.source_id == source.source_id)
        {
            self.federation.spotify_login = None;
        }

        let Some(cache_dir) = sotf_audio_player::service_login::spotify_cache_dir() else {
            self.ui_state.toast_message = Some(ToastMessage::error(
                "Could not determine the config directory for Spotify credentials.",
            ));
            return;
        };

        match sotf_audio_player::service_login::clear_spotify_cached_credentials(&cache_dir) {
            Ok(true) => {
                self.ui_state.toast_message = Some(ToastMessage::success("Logged out of Spotify."));
                sotf_audio_player::reset_service_sessions();
            }
            Ok(false) => {
                self.ui_state.toast_message =
                    Some(ToastMessage::info("No Spotify credentials were cached."));
            }
            Err(e) => {
                self.ui_state.toast_message = Some(ToastMessage::error(format!(
                    "Failed to delete Spotify credentials: {e}"
                )));
            }
        }
    }
}

/// Background worker for the Tidal device-code flow: obtain a device code,
/// report the prompt, then poll until completion, expiry, or failure. The
/// provider's `poll_device_auth` enforces the code's own expiry, so the loop
/// always terminates.
#[cfg(feature = "tidal")]
fn tidal_device_login_worker(
    client_id: &str,
    country_code: &str,
    tx: &std::sync::mpsc::Sender<TidalLoginMessage>,
) {
    use sotf_service_tidal::{DeviceAuthPoll, TidalService};

    let mut service = TidalService::new().with_country_code(country_code);
    if !client_id.trim().is_empty() {
        service = service.with_client_id(client_id);
    }

    let prompt = match service.begin_device_auth() {
        Ok(prompt) => prompt,
        Err(e) => {
            let _ = tx.send(TidalLoginMessage::Failed(e.to_string()));
            return;
        }
    };

    if tx
        .send(TidalLoginMessage::Prompt {
            verification_url: prompt.verification_url,
            user_code: prompt.user_code,
            expires_in_secs: prompt.expires_in_secs,
        })
        .is_err()
    {
        return; // UI went away
    }

    loop {
        std::thread::sleep(TIDAL_POLL_INTERVAL);
        match service.poll_device_auth() {
            Ok(DeviceAuthPoll::Pending) => {}
            Ok(DeviceAuthPoll::Complete) => {
                let access_token = service.access_token().unwrap_or_default().to_string();
                let refresh_token = service.refresh_token().unwrap_or_default().to_string();
                let _ = tx.send(TidalLoginMessage::Completed {
                    access_token,
                    refresh_token,
                });
                return;
            }
            Ok(DeviceAuthPoll::Expired) => {
                let _ = tx.send(TidalLoginMessage::Expired);
                return;
            }
            Err(e) => {
                let _ = tx.send(TidalLoginMessage::Failed(e.to_string()));
                return;
            }
        }
    }
}

/// Background worker for the Spotify OAuth flow. `login_with_oauth` blocks on
/// a loopback listener and requires an ambient tokio runtime (librespot
/// `Session::new` panics without one), hence the entered current-thread
/// runtime. The browser is opened from here via the `open_url` callback; the
/// URL is also forwarded to the UI as a clickable fallback.
#[cfg(feature = "spotify")]
fn spotify_oauth_login_worker(
    cache_dir: std::path::PathBuf,
    tx: std::sync::mpsc::Sender<SpotifyLoginMessage>,
) {
    let send_failure = |error: String| {
        let _ = tx.send(SpotifyLoginMessage::Failed(error));
    };

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            send_failure(format!("failed to start the async runtime: {e}"));
            return;
        }
    };
    let _runtime_guard = runtime.enter();

    let mut service = sotf_service_spotify::SpotifyService::new();
    let tx_open = tx.clone();
    let result = service.login_with_oauth(&cache_dir, move |url| {
        // Best-effort browser open; the UI shows the URL as fallback.
        if let Err(e) = sotf_audio_player::service_login::open_url_in_browser(url) {
            log::warn!("[Spotify] could not open the browser automatically: {e}");
        }
        let _ = tx_open.send(SpotifyLoginMessage::AuthorizeUrl(url.to_string()));
    });

    match result {
        Ok(()) => {
            let _ = tx.send(SpotifyLoginMessage::Completed);
        }
        Err(e) => send_failure(e.to_string()),
    }
}
