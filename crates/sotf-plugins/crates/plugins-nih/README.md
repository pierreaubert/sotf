# plugins-nih

VST3/CLAP plugin wrappers for SOTF audio plugins via nih-plug.

## What It Does

Wraps every SOTF audio plugin as both a VST3 and CLAP plugin using the nih-plug framework. Each plugin is built as a separate dynamic library (cdylib) selected by feature flag, producing a `.dylib` that exports both VST3 and CLAP entry points for use in any DAW.

## Features

- 29 audio plugins available as VST3 and CLAP
- One cdylib per plugin (feature-flag selected)
- Automatic parameter mapping from SOTF ParamSpec to nih-plug parameters
- Buffer format conversion (nih-plug planar to SOTF interleaved)
- Allocation detection in audio thread (`assert_process_allocs`)

## Usage

```bash
# Build the EQ plugin
cargo build --release -p plugins-nih --features eq --no-default-features

# Build the compressor
cargo build --release -p plugins-nih --features compressor --no-default-features

# Build all plugins
just build-nih-plugins
```

The resulting `.dylib` files can be loaded by any VST3 or CLAP compatible DAW.

## Available Plugins

| Feature | Plugin Name | CLAP ID |
|---------|-------------|---------|
| `eq` | SOTF: Parametric EQ | `org.spinorama.sotf.eq` |
| `compressor` | SOTF: Compressor | `org.spinorama.sotf.compressor` |
| `limiter` | SOTF: Limiter | `org.spinorama.sotf.limiter` |
| `gate` | SOTF: Gate | `org.spinorama.sotf.gate` |
| `gain` | SOTF: Gain | `org.spinorama.sotf.gain` |
| `delay` | SOTF: Delay | `org.spinorama.sotf.delay` |
| `upmixer` | SOTF: Upmixer | `org.spinorama.sotf.upmixer` |
| `binaural` | SOTF: Binaural | `org.spinorama.sotf.binaural` |
| `xtc` | SOTF: Crosstalk Cancellation | `org.spinorama.sotf.xtc` |
| ... | *(29 total)* | |

## Architecture

```
lib.rs      -- Feature-gated plugin definitions (one per feature)
params.rs   -- DynamicParams: runtime nih-plug parameter generation
wrapper.rs  -- sotf_nih_plugin! macro: generates full nih-plug impl
```

## Ownership and lifetime

Each VST3/CLAP plugin wraps a SOTF plugin instance created through
`plugins-bridge::create_plugin()`. The generated wrapper owns the instance via
`Box<dyn Plugin>` and uses `plugins-bridge::state` for preset serialization.
All audio buffers are converted from nih-plug's planar layout to SOTF's
interleaved layout inside pre-allocated scratch memory; no heap allocations
occur on the process thread.

## Testing

```bash
cargo check -p plugins-nih --features eq
```

## License

Part of the SOTF (Sound of the Future) project.
