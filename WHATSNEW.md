# 0.6.6

## Apple Store and Microsoft Store availability

- Stable releases are now available on the Apple App Store:
<https://apps.apple.com/ch/app/sound-of-the-future/id6754237332> and
Windows App Store
- Beta macOS releases and command-line artifacts remain available on GitHub Releases.

## New features

### UI

- Added skins for plugins that can have distinct looks: Graphite / Studio Cream / Brutalist
- Fixed UI for most plugins but still not pro looking
- Hopefully fixed full DAW mode (graph of plugins)

### RoomEQ

- Added support for continuous area for optimisation (wrt to per measurement points)
- Added support for pre and post ringing control -> IIR (Q) and FIR
- Added support for warped and Klauz filters -> also supported in EQ (rare so not portable)
- Added support for linear-phase FIR crossover
- Added bayesian optimisation for expensive calls: faster optimisation
- Engine + player now preserve warped/Kautz filter topology end-to-end (RoomEQ
  output was previously silently downgraded to plain biquads)
- Per-channel FIR temporal-masking metrics surfaced in the Review step
  (pre/post peak + audible dB, penalty per channel)
- Aggregate FIR pre/post-ringing audibility and penalty in the Optimization
  Summary card
- EPA temporal-masking knobs (enabled/weight/profile + FIR IR enabled/weight)
  surfaced in the Step-3 configuration screen
- Linear-phase crossover taps now editable per channel (inline FIR-taps +
  latency readout under the crossover dropdown)
- Added RoomEQ perceptual policy presets for reference, music, cinema, night,
  and speech use cases, including JND deadbands, high-frequency guardrails,
  early-cue/FIR advisories, bootstrap uncertainty masks, CTC cue diagnostics,
  and validation bundle descriptors.

### Audio Plugins

- Added a auto gain mode to AAE
- Added support for warped and Klauz filters in EQ
- EQ plugin UI now lets you cycle a band's topology (Biquad → Warped → Kautz),
  cycle the Warped λ preset, and add/remove Kautz pole sections
- EQ preview curve correctly renders warped-biquad and Kautz magnitude

### Math

- Added support for binaural loudness

## Bug fixes

### Recording

- fix: recording spl calibration or delays fails with "failed to to load wav, unsupported format" on some interface (too many channels)

### RoomEQ

- fix: user selected optimisation algo is now used everywhere (except 1d optimisation)

### UI Library

- fix: sliders behaviour
- fix: spacing between sliders that prevented to level meters to be fully visible
- fix; now search both tags and in-memory information, deduplication is done.

### UI Plugins

- Hw interface UI was not activated properly (currently supporting only 2 hw interfaces but it is easy to add more, next is likely avid s1/s3)
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
