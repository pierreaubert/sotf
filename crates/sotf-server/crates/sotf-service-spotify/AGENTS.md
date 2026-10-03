# sotf-service-spotify

Spotify streaming service provider for SOTF (via librespot). Implements the
`StreamingService` trait from `sotf-services`.

## Key Types

- `SpotifyService` -- Spotify backend implementing `StreamingService` (OAuth2 + PKCE login, PCM streaming via librespot, Web API search/library)

## Public API

- `SpotifyService::new()` / `with_quality()` -- construction
- `SpotifyService::login_with_oauth(cache_dir, open_url)` -- interactive OAuth2
  authorization-code + PKCE login. Builds the authorize URL, hands it to the
  `open_url` callback (UI opens a browser), waits on the loopback listener
  `http://127.0.0.1:8898/login` (180 s timeout, CSRF state verified), exchanges
  the code, connects a librespot `Session`, and persists credentials under
  `cache_dir` plus the Web API token pair in `cache_dir/web_api_token.json`
  (0600 on unix). Works from any thread (see Authentication notes).
- `login_with_cached_credentials(cache_dir)` -- restore a
  session from cached credentials; returns `Ok(false)` when the cache is empty
  (caller should fall back to `login_with_oauth`). Restores Web API access
  from the persisted token file, refreshing it against the token endpoint
  when expired. Works from any thread.
- `with_test_web_api(api_base, access_token)` -- `#[doc(hidden)]` test seam for
  downstream integration tests: injects a Web API client pointed at a mock
  server (streaming/`is_authenticated` still need a real session).
- `SpotifyService::saved_albums()` / `saved_tracks()` -- user's library via the
  Web API (inherent methods, not part of the trait).
- `StreamingService::authenticate(...)` -- `UsernamePassword` is rejected
  (Spotify disabled password auth server-side; error directs to
  `login_with_oauth`); `AccessToken` connects via
  `Credentials::with_access_token`; `CachedSession` / `DeviceCode` are rejected
  with directions to the OAuth entry points.
- `search_tracks` / `search_albums` / `album_tracks` -- backed by the Spotify
  Web API; `AuthError` when no access token is present.
- `start_stream` / `stop_stream` -- librespot `Player` → `ChannelSink` →
  `ChannelReader` PCM path (f32 LE bytes).

## Module Layout

- `lib.rs` -- `SpotifyService`, the librespot `Sink` that captures PCM to a channel (`ChannelSink`), the `Read` adapter (`ChannelReader`), `convert_librespot_samples`; unit tests in `mod tests`
- `oauth.rs` -- PKCE flow on the `oauth2` 4.4 primitives (authorize URL building, loopback callback listener with timeout + state check, code exchange and refresh grant returning `token_store::WebApiToken`). The blocking reqwest 0.13 exchange client has an explicit 30 s timeout and does not follow redirects.
- `token_store.rs` -- `WebApiToken` persistence in `web_api_token.json` under the librespot cache dir (0600 on unix, redacted Debug, 60 s expiry skew)
- `web_api.rs` -- `SpotifyWebApi`: search, album tracks, saved albums/tracks against `api.spotify.com/v1` (serde mapping, bounded paged reads). Retries a request once after a 401 by refreshing the token; pagination `next` links are only followed on the same origin (scheme/host/port) as the API base
- `async_runtime.rs` -- `AsyncRuntime`: drives async HTTP calls from the sync trait interface (copied from `sotf-service-tidal`, kept independent). `Drop` moves the fallback runtime onto a plain thread when dropped inside a tokio context (dropping a `Runtime` there panics)
- `consts.rs` -- API/OAuth endpoints, librespot client ID + redirect URI, scopes, bounded JSON reader
- `misc.rs` -- release-year parsing, log truncation helper
- `test_util.rs` -- `#[cfg(test)]` loopback mock HTTP server (same pattern as `sotf-streaming/tests/integration.rs`)

## Authentication notes

- Spotify disabled username/password auth server-side — OAuth (PKCE) is the
  only working login path.
- `oauth.rs` drives `oauth2` 4.4 primitives directly so the caller controls
  the browser and callback timeout. Only the local Web API token type crosses
  its private module boundary.
- `librespot_core::Session::new` panics without a tokio runtime
  (`Handle::current()`); all session work (`Session::new`, `connect`) runs
  inside `self.rt.block_on`, whose embedded fallback runtime guarantees an
  entered runtime on any calling thread. The fallback is multi-thread so the
  session's long-lived tasks (packet dispatch, keepalive) stay driven after
  `block_on` returns. Never call the service from a current-thread runtime's
  driver thread (`Runtime::block_on` panics there).

## Dependencies

- `sotf-services` -- core trait and shared types
- `librespot-core` / `librespot-playback` / `librespot-protocol` -- Spotify Connect and PCM playback
- `oauth2` (4.4, without its default HTTP client) -- PKCE primitives
- `reqwest` (0.13, async and blocking) / `serde` / `serde_json` -- Spotify Web API and bounded token exchange
- `tokio` -- runtime required by librespot's async connect
- `url` -- OAuth callback query parsing

## Secrets hygiene

Tokens are only logged via `redact_secret`; `Debug` for `SpotifyService` and
`SpotifyWebApi` redacts the access token.

## Testing

```bash
cargo test -p sotf-service-spotify
cargo check -p sotf-service-spotify && cargo clippy -p sotf-service-spotify
```
