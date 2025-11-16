<!-- markdownlint-disable-file MD013 -->

# SotF: an automatic eq for your speaker or headset

The software either an app or a cli helps you to get better sound from your speakers or your headsets.
*Sound of the Future* or *SotF* in short comes from the song from [Giorgio Moroder](https://en.wikipedia.org/wiki/Giorgio_Moroder) made popular by [Daft Punk](https://en.wikipedia.org/wiki/Daft_Punk). You can find 100 versions on Youtube. Here is an [official one](https://youtu.be/zhl-Cs1-sG4?si=H4hgakoEdQn-HMH6&t=73).

## Install

### Cargo

Install [rustup](https://rustup.rs/) first.

If you already have cargo / rustup:

```shell
cargo install just
just
```

Select the correct install just command for your platform:
```shell
just install-...
```

You can build or test with a simple:
```shell
just build
just test
```

and you are set up. See this [README](src-autoeq/README.md) for instructions on how to use it.

### Optional Features

#### PNG Export

By default, AutoEQ only generates HTML plots. To enable PNG export functionality (which requires a WebDriver), install with the `plotly_static` feature:

```shell
cargo install autoeq --features plotly_static
```

This feature is disabled by default to reduce dependencies and build complexity. HTML plots provide the same visualization capabilities without requiring additional system dependencies.

## Toolkit

### src-autoeq

A [CLI](src-autoeq/README.md) to optimise the response of your headset or headphone.
A corresponing App is also available at [https://github.com/pierreaubert/autoeq-app](https://github.com/pierreaubert/autoeq-app).

### src-testfunctions

A [set of functions](src-testfunctions/README.md) for testing non linear optimisation algorithms

### src-de

A implementation of [differential evolution algorithm](src-de/README.md) (forked from Scipy) with an interface to NLopt and MetaHeuristics two libraries that also provide various optimisation algorithms. DE support linear and non-linear constraints and implement other features like JADE or adaptative behaviour.

### src-cea2034

A implementation of CEA2034 aka [Spinorama](https://spinorama.org): a set of metrics and curves that describe a loudspeaker performance.

### src-env

A small set of functions and constants used by the other crates but you are unlikely to be interested.

### src-audio

This backend take care of all the Audio activities (from recording to playing). It also provides support for IIR filters, SPL computations etc

#### Microphone Compensation

When recording measurements with `sotf_recorder`, you can compensate for your microphone's non-flat frequency response using `--microphone-compensation`:

```bash
./target/release/sotf_recorder \
  --signal sweep \
  --duration 10 \
  --microphone-compensation mic_calibration.txt
```

**Supported formats:**
- **CSV** (`.csv`): Comma-separated with optional header
  ```csv
  frequency_hz,spl_db
  1000,3.0
  8000,2.0
  ```
- **TXT** (`.txt`): Auto-detects separator (comma, tab, or space)
  ```
  1000 3.0
  8000,2.0
  ```

**TXT format features:**
- **Auto-detects separator** per line: comma, tab, or space
- **Skips non-numeric lines** (headers, notes, etc.)
- **Ignores comments** starting with `#`
- **Handles mixed formats** in the same file

**How it works:**
1. **Pre-compensation (sweeps)**: The playback signal is shaped to cancel the mic's response
2. **Post-compensation (all signals)**: The CSV output is corrected after recording

See example files: `mic_calibration_example.csv` and `mic_calibration_example.txt`

### src-hal

This crate builds a HAL (Audio Driver on MacOS) such that you can redirect all your music to this driver and benefit from corrected sounds all the time.

### src-confbar

This crate allows you to configure the above driver and is conveniently available from the menubar.

### src-ui-frontend

The UI frontend :) Nothing special here, just a boring UI.

### src-tauri

The Tauri backend for the frontend. Noting special here, just a wrapper around src-ui-backend.

