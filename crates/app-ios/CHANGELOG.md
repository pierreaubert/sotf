# 0.5.3 (unreleased)

## Remote SOTF connections

- Remote SOTF API bearer tokens are now persisted through the iOS Keychain
  bridge, so saved servers can reconnect after app restart without losing the
  SSE event stream token.
- The Connections settings flow now treats the SOTF API URL and bearer token as
  separate from MPD port 6600 credentials, matching the server-mode setup shown
  by the TUI.

# 0.5.2

## New

- Ios: added support for svg icons, added settings in menu, can now move the IIR in the EQ plugin

## Changes

- AU plugins are working and I can load them but without a proper UI
- More or less working version on iOS
