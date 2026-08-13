# sotf-plugin-beamformer

Beamformer plugin — MVDR, superdirective, and GSC beamformers.

## What It Does

Combines signals from multiple microphones to focus on sound from a specific direction while rejecting noise and interference from other directions. Supports three beamforming algorithms for different use cases.

## Features

- **MVDR beamformer**: Minimum Variance Distortionless Response with scale-independent look-source protection and adaptive interference covariance
- **Superdirective beamformer**: Maximum directivity for diffuse noise fields
- **GSC beamformer**: Generalized Sidelobe Canceller — adaptive interference rejection
- **Configurable steering**: Point the beam in any direction

## Array and realtime contract

The exposed geometry is a 2–8 microphone linear array. Looking from above:

```text
               0° broadside
                    ↑
mic 0 — mic 1 — … — mic N-1  → +90° endfire
                    ↓
              180° broadside
```

Microphone count, spacing, steering angle, and algorithm are construction-time
graph state. Change them by rebuilding the plugin; live setters reject them so
the audio thread never allocates, replans FFT state, resets adaptation, or
changes latency unexpectedly. Serialized algorithms use `"MVDR"`,
`"Superdirective"`, and `"GSC"`; legacy indices 0, 1, and 2 are accepted on
load.

MVDR learns covariance only when a frame is not dominated by the configured
look direction. This estimator is normalized by frame energy, so its decision
does not depend on microphone gain or absolute FFT scaling. GSC aligns every
microphone before both fixed summation and blocking, and protects target-only
frames from adaptive cancellation.

MVDR and superdirective report 512 samples of STFT latency. GSC reports the
ceiling of its maximum fractional steering-compensation delay. All warmed
processing paths are allocation-free.

## Architecture

```
src/
├── lib.rs              # BeamformerPlugin
├── mvdr.rs             # MVDR beamformer
├── superdirective.rs   # Superdirective beamformer
├── gsc.rs              # Generalized Sidelobe Canceller
├── steering.rs         # Steering vector computation
└── params.rs           # Parameters
```

## Testing

```bash
cargo test -p sotf-plugin-beamformer
```

## License

Part of the SOTF (Sound of the Future) project.
