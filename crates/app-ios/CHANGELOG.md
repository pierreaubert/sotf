# 0.5.3 (unreleased)

## iOS / GPUI P1 hardening

- The canonical iOS Xcode target and app source directory are now `SotFPlayer`;
  host tests guard against stale `SotFApp` paths while still allowing ignored
  local `lib/libsotf_ios.a` build artifacts.
- iOS now forwards Dynamic Type changes, memory warnings, and Low Power Mode
  changes into the GPUI app so text scale, transient caches, and reduced motion
  respond to platform state.
- Audio settings on iOS can open the native AirPlay/Bluetooth route picker.

## Remote SOTF connections

- Remote SOTF API bearer tokens are now persisted through the iOS Keychain
  bridge, so saved servers can reconnect after app restart without losing the
  SSE event stream token.
- The Connections settings flow now treats the SOTF API URL and bearer token as
  separate from MPD port 6600 credentials, matching the server-mode setup shown
  by the TUI.
- Connections can scan SOTF server QR codes with the camera and pass the
  encoded API URL plus bearer token into the shared remote-server store.
- Preferences on iOS hide local-only Library and Keybindings tabs; if a
  connected remote library does not match the local database identity, the app
  clears the stale local database and disposable album/artwork cache.

# 0.5.2

## New

- Ios: added support for svg icons, added settings in menu, can now move the IIR in the EQ plugin

## Changes

- AU plugins are working and I can load them but without a proper UI
- More or less working version on iOS
