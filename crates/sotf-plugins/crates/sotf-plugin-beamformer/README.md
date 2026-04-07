# sotf-plugin-beamformer

Beamformer plugin — MVDR, superdirective, and GSC beamformers.

## What It Does

Combines signals from multiple microphones to focus on sound from a specific direction while rejecting noise and interference from other directions. Supports three beamforming algorithms for different use cases.

## Features

- **MVDR beamformer**: Minimum Variance Distortionless Response — optimal noise rejection
- **Superdirective beamformer**: Maximum directivity for diffuse noise fields
- **GSC beamformer**: Generalized Sidelobe Canceller — adaptive interference rejection
- **Configurable steering**: Point the beam in any direction

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
