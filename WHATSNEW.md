# 0.6.6

## Apple Store and Microsoft Store availability

- Stable releases are now available on the Apple App Store:
<https://apps.apple.com/ch/app/sound-of-the-future/id6754237332> and
Windows App Store 
- Beta macOS releases and command-line artifacts remain available on GitHub Releases.

## New features

### UI

- Added skins for plugins that can have distinct looks: Graphite / Studio Cream / Brutalist
- In stereo mode we plot the width dynamically. Replaced it by peaks for 5.0 and larger.

### RoomEQ

- Added support for continuous area for optimisation (wrt to per measurement point)
- Added bayesian optimisation for expensive calls: faster optimisation

### Audio Plugins

- Added a auto gain mode to AAE

## bug fixes

### Recording

- fix: recording spl calibration or delays fails with "failed to to load wav, unsupported format" on some interface (too many channels)

### RoomEQ

- fix: user selected optimisation algo is now used everywhere (excep 1d optimisation)

### UI Library

- fix: sliders behaviour
- fix: spacing between sliders that prevented to level meters to be fully visible
- fix; now search both tags and in-memory information, deduplication is done.

### UI Plugins

- Hw interface UI was not activated properly
- Fixed auto gain in Upmixer


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


