As of June 29, 2026: neither Spotify nor TIDAL appears to offer a
simple public “sandbox music test account” you can just create. For
real playback/
  integration smoke tests, assume you need real accounts with
subscriptions, gated behind env vars and never in CI by default.

## Spotify

- Create a Spotify developer app in the Spotify Developer
Dashboard (https://developer.spotify.com/dashboard).
- Development-mode apps require the app owner to have Spotify
Premium, and only allow up to 5 authenticated users.
- Add testers through Dashboard → app → Settings → Users Management → Add new user.
- Each tester needs their own Spotify account/email, likely Premium
for playback paths.
- Spotify’s docs also say wider access now requires extended quota
mode, and as of May 15, 2025 they accept extension applications only
from organizations, not individuals.

Source: Spotify quota modes docs: https://developer.spotify.com/documentation/web-api/concepts/quota-modes

## TIDAL

- Use the TIDAL Developer Portal (https://developer.tidal.com/) to register/request app access.
- For our purposes, plan on a normal paid TIDAL account in the target market.
- If you need a non-personal test account or partner credentials, you’ll likely need to request that through TIDAL directly/partner support; I don’t see a     public self-serve sandbox account flow.
- For local smoke tests, use env vars like TIDAL_CLIENT_ID and TIDAL_ACCESS_TOKEN, or go through the device-code auth flow we just made more testable.

## Practical Setup

Use two tiers:

- CI/default: mocked provider APIs, no real credentials.
- Manual/live: real throwaway paid accounts, small sample reads only, env-gated:
  - SOTF_TIDAL_LIVE=1
  - TIDAL_CLIENT_ID
  - TIDAL_ACCESS_TOKEN
  - SOTF_SPOTIFY_LIVE=1
  - SPOTIFY_USERNAME
  - SPOTIFY_PASSWORD

For security, create dedicated “sotf qa” accounts, don’t reuse personal accounts, don’t store tokens in repo/config snapshots, and assert logs never include tokens/passwords.

