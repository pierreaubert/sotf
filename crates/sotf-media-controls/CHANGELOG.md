# 0.1.3

## Changes

- Added host-independent unit tests for `PlatformConfig`, `MediaMetadata`,
  `MediaPlayback`, `SeekDirection`, and `MediaControlEvent` variants.
- Added `with_dbus_name` and `with_display_name` builder helpers to
  `PlatformConfig` to match the existing `with_window_handle` API.
- Added backend-level tests for `OwnedMetadata` copying and MPRIS time
  conversion, plus a macOS handler-thread shutdown/drop lifetime guard.
- Documented the callback lifetime contract explicitly: handlers are
  `Send + 'static`, owned by the backend, and dropped before app/player state
  because `Drop` joins the handler/runtime thread before the backend is
  destroyed.
- Added `PLATFORM_LIMITATIONS.md` covering the supported-platform matrix,
  macOS main-thread construction requirement, Linux D-Bus requirement,
  cover-artwork omission, process-global command-center behavior, and
  unit-test coverage gaps.

## Testing

- Expanded unit-test coverage for state transitions, metadata formatting,
  event parsing, and lifetime-safe handler teardown.
- No real macOS media-key hardware or D-Bus session is required for the new
  tests.

# 0.1.1

## Changes

- Hardened the platform window handle API with a non-null, lifetime-bound
  `WindowHandle` wrapper instead of accepting a raw HWND pointer directly.
- Enforced macOS main-thread construction for `MPRemoteCommandCenter` wiring
  while keeping metadata/playback updates marshalled onto the main queue.
- Clamped unsafe media-position conversions for NaN, infinity, negative MPRIS
  times, and oversized durations.
- Documented MPRIS `Raise` / `Quit` / `OpenUri` consumer responsibilities and
  clarified that Windows SMTC remains a tracked TODO.

## Testing

- Added regression tests for handle wrapping, unsupported stub behavior, MPRIS
  time conversion, and macOS off-main construction rejection.

# 0.1.0

## Changes

- Change input of systemwide to allow for N channels
