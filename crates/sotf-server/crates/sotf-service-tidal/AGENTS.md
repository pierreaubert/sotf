# sotf-service-tidal

Tidal streaming service provider for SOTF. Implements the `StreamingService`
trait from `sotf-services`.

## Key Types

- `TidalService` -- Tidal backend implementing `StreamingService` (token and device-code auth, search, stream URL resolution)
- `DeviceAuthPrompt` -- structured device-code login prompt for UIs (`verification_url`, `user_code`, `expires_in_secs`)
- `DeviceAuthPoll` -- device-code poll outcome (`Pending` / `Complete` / `Expired`)

## Public API (inherent on `TidalService`, beyond the `StreamingService` trait)

- `begin_device_auth()` / `poll_device_auth()` -- structured device-code login; the `authenticate(DeviceCode)` trait arm is built on these and still surfaces the prompt as an `AuthError` string
- `authenticate_refresh()` -- exchange the stored refresh token (handles rotation)
- `access_token()` / `refresh_token()` / `user_id()` / `set_tokens()` -- session getters + restore from a persisted session
- `favorites_albums()` / `favorites_tracks()` -- paged user library (requires auth + known `user_id`)
- Builder/config: `with_client_id`, `with_country_code`, `with_quality`, `with_api_base` / `with_auth_base` (test seams)

## Module Layout

- `lib.rs` -- module declarations and `pub use tidal_service::*`
- `tidal_service.rs` -- `TidalService` implementation (auth, refresh, search, favorites, album tracks, stream URLs)
- `async_runtime.rs` -- `AsyncRuntime`: drives async HTTP calls from the sync trait interface. `Drop` moves the fallback runtime onto a plain thread when dropped inside a tokio context (dropping a `Runtime` there panics)
- `consts.rs` -- API base URLs, client ID, bounded JSON reader
- `misc.rs` -- release-year parsing, cover-art URL builder, log truncation helpers
- `types.rs` -- Tidal API response types (serde), incl. `TidalFavoritesResponse<T>` / `TidalFavoriteItem<T>` wrappers
- `tests.rs` -- unit tests
- `tests/mock_api.rs` -- integration tests against a local mock HTTP server (std-only, no dev-deps)

## Dependencies

- `sotf-services` -- core trait and shared types
- `reqwest` / `serde` / `serde_json` -- Tidal HTTP API
- `tokio` -- runtime for blocking-on-async inside the sync trait

## Testing

```bash
cargo test -p sotf-service-tidal
cargo check -p sotf-service-tidal && cargo clippy -p sotf-service-tidal
```
