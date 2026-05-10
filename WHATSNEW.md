# 0.6.6

## Apple Store and Microsoft Stor availability

- Stable Apple releases are now available on the App Store: <https://apps.apple.com/ch/app/sound-of-the-future/id6754237332>
- Beta macOS releases and command-line artifacts remain available on GitHub Releases.

## New features

### UI

- Added skins for plugins that can have distinct looks: Graphite / Studio Cream / Brutalist

### RoomEQ

- Added support for continuous area for optimisation (wrt to per measurement point)
- Added bayesian optimisation for expensive calls: faster optimisation

## bug fixes

### Recording

- fix: recording spl calibration or delays fails with "failed to to load wav, unsupported format" on some interface (too many channels)

### UI Library

- fix: sliders behaviour
- fix: spacing between sliders that prevented to level meters to be fully visible

### UI Plugins

- Hw interface UI was not activated properly

# 0.6.1 -> 0.6.5

## tweaks to be accepted in the Apple Store

- Apple does not want private symbols to be used (vendored some crates)
- Apple wants only the correct set of permissions (removed camera which is not yet used)
- Apple wants very specific signatures (not the same for DMG and MAS PKG)

## tweaks to be in accepted in the Microsoft Store (success)

- Microsoft wants all runtime libraries to be declared: remove them one by one and rewrote code in Rust to make it easier.
- Microsoft wants a video of the running app.

# 0.6.0

Features freeze


