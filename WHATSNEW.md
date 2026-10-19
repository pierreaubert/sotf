# 0.7.17

## Features

- Capture step now shows a real-time input level meter (green / yellow /
  red bar with peak, RMS and peak-hold in dBFS, plus dB SPL when an SPL
  calibration exists). After each take, the app advises lowering the
  microphone gain when clipping occurred, or raising it when the
  measurement peaked below −30 dBFS.
- The RoomEQ report is now generated natively in Rust (AutoEQ d3rs
  dataset composer) and displayed directly in the app — no Python
  runtime is involved anymore.

## Bug fixes

- SPL calibration: entering a meter reading (e.g. 68 dB) no longer shows
  a misleading "correction" of ~105 dB. The screen now shows the tone
  level in dBFS and labels the stored value for what it is — the SPL of
  a full-scale signal at the current microphone gain — with a
  plausibility warning outside the usual range. The calibration
  instructions no longer mention GD-Opt v2.
- Evaluating: group delay no longer draws every channel twice (the
  stored excess group delay and the recomputed curve were identical),
  the impulse-response amplitude axis now scales to the measured peak
  instead of rendering flat on a fixed −1…1 axis, RT60 and Clarity fall
  back to the impulse reconstructed from the frequency response for
  response-only results, and the spectrogram renders as a proper
  magma cell grid (peak-relative 80 dB window) instead of a blank
  white panel. Vertical axis labels were shortened so they stay
  inside the graph. For recordings re-opened from a saved session,
  distortion/RT60/clarity/group-delay are restored from each take's
  analysis CSV and the spectrogram is recomputed from the impulse
  response, instead of showing "not available".
- iOS: fixed the black screen in the simulator (UIScene lifecycle),
  the Server settings option is hidden on iPhone (kept on iPad), and a
  renderer fix restores intermittently missing letters.
- macOS DMG: third-party license notices are resolved from the pinned
  GitHub gpui-toolkit checkout instead of a sibling directory on disk.

## Testing

- Dev-API scenarios now cover the client/server flows, the recording
  wizard (fake capture, SPL calibration, clipping review) and opening
  the RoomEQ report; measured RoomEQ datasets (2.0 and 5.1.4) are
  exercised in the DSP system across iir/fir/mixed configurations.

## Audit fixes (2026-10-18)

- TUI RoomEQ export now writes the full DSP chain (all EQ stages, gain,
  convolution sidecars) in every supported format instead of flattening
  to a single PEQ stage; PEQ-only formats are rejected explicitly rather
  than silently dropping stages. Export/apply are blocked when the
  result is stale relative to the current inputs.
- The shared RoomEQ model owns input/result/run revisions: both GPUI
  and TUI discard completions from superseded runs, the TUI optimizer
  uses the configured sample rate (not the device default) and can be
  cancelled with `c`, and GPUI progress uses a bounded channel with a
  run generation so a cancelled run cannot overwrite a newer one.
- Recordings persist the actual negotiated capture sample rate and the
  CTC builder uses each take's own rate (mixed-rate takes are rejected
  instead of resampled silently).
- Reconstructed impulse responses (from frequency response only) are
  validity-gated in math-dsp — degenerate grids, non-finite data, or a
  wrapped/silent reconstruction show "not available" with a
  "reconstructed, not measured" caption instead of a fake curve.
- Performance: capture preparation (stimulus synthesis, WAV writing,
  CTC reference) runs off the UI thread; Evaluating borrows recording
  results and memoizes group-delay/RT60/clarity series; RoomEQ
  freshness is a cheap input digest instead of serializing every
  response curve on each render; the library track list reuses cached
  filtered track indices for both the count and the visible rows.
- CI: the GitHub testing-tiers workflow checks out the private sibling
  repositories at pinned revisions so it can run on a clean runner;
  the GPUI test targets are declared explicitly so no suite is
  silently skipped, and the capture-frontend regression test compiles
  against the current capture API again.

# 0.7.12

Preparation for release

# 0.7.11

## Features

- Added a A/B testing app
- Added a Howtolisten like app

# 0.7.10

## Audio engine

- Plugin-chain and graph structural changes are now prepared and validated
  before an atomic processing-boundary commit. Latency changes are reported
  explicitly, same-rate replacements crossfade with latency alignment, and
  retired plugin state is reclaimed by the background GC thread.

# 0.7.9

## Features

- iOS support for iPad and iPhone
- Client / Server (TUI can act as a server, and iPhone app is a client) and support TLS ; QR code allows the passing of keys between the server and the client
- Systemwide is working well enough

## Major changes

Splitted again autoeq, math-audio, gpui-toolkit and soda-reader into there own set of crates.

## Bug fixes

- Lots and lots of them, see detailed CHANGELOG in each crate.

# 0.6.8

## Features

### RoomEQ

- Added support for *atmosphere* to optimised for *hifi*, *cinema*, *reference*, *night* or *speech*
- Added audibility/JND residual deadbands and safer high-frequency correction
- Added EPA temporal masking integration

### Main applications

- New look more modern and less cluttered
- Editor for music metadata

### Server and streaming

- SotF can now act as a server (you can stream from your computer) and can also act as client (you can listen to stream)
- Support for Spotify and other streaming protocols is in but not well tested
- Authentification is simplified with QR code that you can scan on the client side.

### Audio Plugins

- You can now load external plugins. For security reasons (untrusted code) they run in an isolated sandbox, some of them may not work if they need access to external IP or PACE protection. Relax mode is not yet implemented.
- Export to AUv3, VST3 and CLAP is working but not well tested.

### SytemWide MacOS

- Lots of debugging but it sometimes does not work and i dont know why yet. Not ready for general usage.

### iOS support

- Alpha version is working on iPad, need more love on iOS
- Even more alpha on tvOS


## Bug fixes

- too many to name


# 0.6.7

Lots of bug fixes

## Bug fixes

- Allow to write on the disk in Apple allowed directories via the sandbox
- Fix sample rate propagation in roomEQ



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
