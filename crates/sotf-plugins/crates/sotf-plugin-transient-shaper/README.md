# sotf-plugin-transient-shaper

Transient Shaper — SPL Transient Designer approach for attack/sustain control.

## What It Does

Controls the attack (transient) and sustain portions of audio independently, without using a threshold. Based on the SPL Transient Designer approach: the signal's envelope is analyzed to separate the fast-changing transient portion from the slower sustain. Each can be boosted or cut independently — add punch by boosting attack, or smooth out percussive elements by reducing it.

## Features

- **Threshold-independent**: Works on signal shape, not level
- **Attack control**: Boost for punch, cut for smoothness
- **Sustain control**: Boost for fullness, cut for tighter sound
- **SPL Transient Designer approach**: Proven algorithm for natural-sounding results

## Architecture

```
src/
├── lib.rs     # TransientShaperPlugin implementation
└── params.rs  # Parameter definitions
```

## Testing

```bash
cargo test -p sotf-plugin-transient-shaper
```

## License

Part of the SOTF (Sound of the Future) project.
