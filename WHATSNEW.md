# 0.6.8

## New features

### UI

- Added skins for plugins that can have distinct looks: Graphite / Studio Cream / Brutalist

### RoomEQ

- Added support for continuous area for optimisation (wrt to per measurement point)
- Added bayesian optimisation for expensive calls: faster optimisation

## bug fixes

### Recording

- fix: recording spl calibration fails with "failed to to load wav, unsupported format"

### UI Library

- fix: sliders behaviour
- fix: spacing between sliders that prevented to level meters to be fully visible

### UI Plugins

- Hw interface UI was not activated properly

# 0.6.3 -> 0.6.7

## tweaks to be in accepted in the Apple Store (pending)

- Apple does not want private symbols to be used (vendored some crates)
- Apple wants only the correct set of permissions (removed camera which is not yet used)
- Apple wants very specific signatures (not the same for DMG and MAS PKG)

## tweaks to be in accepted in the Microsoft Store (success)

- Microsoft wants all runtime libraries to be declared: remove them one by one and rewrote code in Rust to make it easier.
- Microsoft wants a video of the running app.

# 0.6.2



