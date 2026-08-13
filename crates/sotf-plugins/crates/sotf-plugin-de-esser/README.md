# sotf-plugin-de-esser

De-esser — sibilance reduction for audio.

## What It Does

Reduces harsh sibilant sounds ("s", "sh", "ch") in vocals and other audio. Applies frequency-selective dynamic compression to the sibilance range (typically 4-10 kHz) without affecting the rest of the spectrum.

## Features

- **Frequency-selective processing**: Targets only the sibilance band
- **Dynamic compression**: Reduces sibilance only when it exceeds the threshold
- **Transparent operation**: Preserves the natural character of the audio

## DSP contract

- `Wideband` applies detector gain reduction to the complete signal.
- `Split-Band` uses an LR4 crossover and applies reduction only to its high
  output. Mix controls reduction depth against the phase-matched low+high sum;
  when gain reduction is zero, every Mix value produces identical output.
- Frequency is the detector centre. Q defines a symmetric octave bandwidth
  once; the highpass and lowpass edge sections use fixed Butterworth pole Q.
- Frequency, Q, and Mode are structural controls. Hosts rebuild the plugin when
  they change; Threshold, Ratio, Attack, Release, and Mix are realtime-safe.
- Processing has no algorithmic latency, overwrites exactly the active frames,
  sanitizes non-finite input, and allocates nothing after initialization.
- Gain-reduction meters publish from elapsed samples at approximately 30 Hz.

## Architecture

```
src/
├── lib.rs                  # crate surface
├── params.rs               # canonical parameter specs and UI layout
└── lib/
    ├── de_esser_plugin.rs  # detector, dynamics, split/wide processing
    ├── de_esser_data.rs    # realtime monitoring snapshot
    ├── types.rs            # strict serialized state
    ├── consts.rs           # DSP constants
    └── tests.rs            # focused DSP/host regressions
```

## Testing

```bash
cargo test -p sotf-plugin-de-esser
cargo run -p sotf-plugin-de-esser --features qa --bin qa-de-esser
```

## License

Part of the SOTF (Sound of the Future) project.
