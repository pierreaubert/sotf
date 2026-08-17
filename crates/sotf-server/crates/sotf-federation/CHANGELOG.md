# Unreleased

## Fixed

- Tidal: the library-scan path no longer burns the single-use rotated
  refresh token — `TidalProvider::new_with_token_persister` /
  `connect_with` accept a `TidalTokenPersister` callback invoked with the
  rotated access/refresh pair after a successful refresh-token exchange, so
  callers can persist it back to the source config.
- Tidal / Spotify: a failing track listing for one album (404/500) no longer
  aborts the whole scan; the album is skipped with a warning and the rest of
  the source is still returned. Top-level listing failures still error out.
- Tidal: an empty / whitespace `client_id` now keeps the built-in default
  instead of overriding it with an empty string (aligned with
  `ServiceManager::connect_tidal`).

# 0.8.2

## New

- Continue to implement sources and servers for SotF
- QA-SEC-006 negative abuse tests for invalid provider URLs/ports, duplicate provider registration, and source revocation.
