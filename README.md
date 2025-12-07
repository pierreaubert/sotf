<!-- markdownlint-disable-file MD013 -->

# SotF: an automatic eq for your speakers or headsets

The software either an app or a cli helps you to get better sound from your speakers or your headsets. It is also a toolkit with many components helping to build audio related applications: mathematical crates to do DSP or BEM, graphical crates to do GUI, audio crates to do audio, and so on.

*Sound of the Future* or *SotF* in short comes from the song from [Giorgio Moroder](https://en.wikipedia.org/wiki/Giorgio_Moroder) made popular by [Daft Punk](https://en.wikipedia.org/wiki/Daft_Punk). You can find many versions on Youtube. Here is an [official one](https://youtu.be/zhl-Cs1-sG4?si=H4hgakoEdQn-HMH6&t=73).

## Install

### Cargo

Install [rustup](https://rustup.rs/) first.

If you already have cargo / rustup, you can jump to:

```shell
cargo install just
export AUTOEQ_DIR=`pwd`
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

## Toolkit

### math-testfunctions

A [set of functions](math-testfunctions/README.md) for testing non linear optimisation algorithms used in the next crate.

### math-de

A implementation of [differential evolution algorithm](math-de/README.md) (forked from Scipy) with an interface to NLopt and MetaHeuristics two libraries that also provide various optimisation algorithms. DE support linear and non-linear constraints and implement other features like JADE or adaptative behaviour.

Status: good for speaker equalisation. Not tested enough for other use cases.

### math-iir

An IIR and FIR filter implementation in rust. Does what you expect. Compatible with Equalizer APO. It can generate various output formats.

Status: stable and working well.

### math-solvers

A set of classical solvers with preconditionners that use LAPACK, BLAS and rayon for parallelisation. Support sparse matrices.
Also can work in WASM which is convenient for web demos.

Status: correct and relatively fast but not optimised to death. WASM needs rust nightly to run in parallel.

### math-wave

A set of functions to compute know analytical solution of the wave equation.

Status: correct.

### math-bem and math-fem

Implement BEM and FEM for the Helmotz and wave equations. Support multigrid for both system.

Status: unknown, results match analytical results on simple mesh. Needs more testing esp. for the advance features.

### autoeq-cea2034

A implementation of CEA2034 aka [Spinorama](https://spinorama.org): a set of metrics and curves that describe a loudspeaker performance.

Status: mature.

### autoeq

A [CLI](autoeq/README.md) to optimise the response of your headset or headphone.

Status: good up to very good depending what you optimise for.

### autoeq-roomsim

A room simulator to help you to understand the response of your speakers on your room. [Available online](https://roomsim.spinorama.org) try it out!

Status: getting good.

### autoeq-env

A small set of functions and constants used by the other crates but you are unlikely to be interested.


### GPUI support

#### gpui-ui-kit

A set of components to make it easier to develop UI. See the showcase application to see what is available.

```shell
just demo-ui-kit
```
or
```shell
cargo run --release --example showcase -p gpui-ui-kit
```

Status: ok

#### gpui-d3rs

A port of the famous d3js library in rust with support for the GPU. You get a similar library but dont need a web-browser.
See the showcase application :
```shell
cargo run --release --bin d3rs-showcase --features="gpui"
```
and the spinorama demo:
```shell
cargo run --release --bin d3rs-spinorama --features="spinorama, gpu-3d"
```

Status: ok, not everything is GPU accelerated yet!

#### gpui-px

A high level plotting library similar to plotly express.
See the showcase application :
```shell
cargo run --release --bin px-showcase
```

Status: getting ok, not everything is GPU accelerated yet!

### sotf-audio-*

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

Status:
- src-audio-engine: production quality
- src-audio-plugins: code is good but some plugins need tuning.
- src-audio-plugins-ffi: beta quality
- src-audio-plugins-au: an attempt at generating AUplugins
- src-audio-player: production quality
- src-audio-player-tui: good quality, can scan by 4k albums and play them with an TUI interface. It is good for testing parameters and plugins.
- src-audio-player-gpui: experimental status


### MacOS specifig: src-hal and src-confbar

src-hal crate builds a HAL (Audio Driver on MacOS) such that you can redirect all your music to this driver and benefit from corrected sounds all the time.
src-confbar crate allows you to configure the above driver and is conveniently available from the menubar.

Status: experimental for HAL and ok for confbar.

### math-convexhull3d

This crate computes a convex hull in 3d.

Status: good quality aka no known bug.

### math-bem

This crate implements a BEM (Boundary Element Solver) for the HRTF computation. It will also be used for optimal shape of speaker waveguide.
This part is not on GH yet. It is a merge of previous projects in python and will come step by step. BEM is the first crate but the control theory part is
not converted yet (adjoint computation etc).

Status: ok-ish

### src-head-scanner

An experimental app to scan your head and do all the computations to get an HRTF. There is a long way to go but we are making progress.

### src-tauri and and src-ui-frontend

The Tauri backend for the frontend. Noting special here, just a wrapper around src-audio and src-autoeq.
The UI frontend :) Nothing special here, just a boring UI.

Status: working but unpolished

