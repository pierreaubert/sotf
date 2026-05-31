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
