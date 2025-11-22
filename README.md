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

### src-testfunctions

A [set of functions](src-testfunctions/README.md) for testing non linear optimisation algorithms used in the next crate.

### src-de

A implementation of [differential evolution algorithm](src-de/README.md) (forked from Scipy) with an interface to NLopt and MetaHeuristics two libraries that also provide various optimisation algorithms. DE support linear and non-linear constraints and implement other features like JADE or adaptative behaviour.

### src-cea2034

A implementation of CEA2034 aka [Spinorama](https://spinorama.org): a set of metrics and curves that describe a loudspeaker performance.

### src-autoeq

A [CLI](src-autoeq/README.md) to optimise the response of your headset or headphone.

### src-env

A small set of functions and constants used by the other crates but you are unlikely to be interested.

### src-audio

This backend take care of all the Audio activities (from recording to playing). It also provides support for IIR filters, SPL computations etc.

It does provide a lot of features:
- playing:
  - reading from files and audio interfaces
  - computing relay gain, spectrum, lufs etc
- recording:
  - record from N channels
  - play test signals and record on N channels or N times on 1 channel automatically
  - microphone compensation
- plugins:
  - gate
  - limiter
  - compressor
  - eq (iir)
  - convolver (fir)
  - delay
  - crossover (via iir)
  - loudness compensation (via iir)
  - upmixer up to 9.1.6
  - binaural decoder

It does have interfaces to demonstrate how the system works:
- There is a basic CLI
- There is a fun TUI interface that is good enough to use day to day to play music
- A better looking interface is in construction on src-gpui-player but not ready for general use at all.


### MacOS specifig: src-hal and src-confbar

src-hal crate builds a HAL (Audio Driver on MacOS) such that you can redirect all your music to this driver and benefit from corrected sounds all the time.
src-confbar crate allows you to configure the above driver and is conveniently available from the menubar.

### src-convexhull3d

This crate computes a convex hull in 3d.

### src-bem

This experimental crate will be a BEM (Boundary Element Solver) for the HRTF computation. It will also be used for optimal shape of speaker waveguide.
This part is not on GH yet. It is a merge of previous projects in python and will come step by step. BEM is the first crate but the control theory part is
not converted yet (adjoint computation etc).

### src-head-scanner

An experimental app to scan your head and do all the computations to get an HRTF. There is a long way to go but we are making progress.

### src-tauri and and src-ui-frontend

The Tauri backend for the frontend. Noting special here, just a wrapper around src-audio and src-autoeq.
The UI frontend :) Nothing special here, just a boring UI.

