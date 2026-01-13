<!-- markdownlint-disable-file MD013 -->

# SotF: an automatic eq for your speakers or headsets

The software helps you to get better sound from your speakers or your headsets. It can run as a TUI app (in a terminal) or a classical UI.
What can you do with it?
- play music :)
- add audio plugins to customise the experience, an EQ, an upmixer for spatial audio or a binaural rendered? a limiter or a multiband compressor? a denoiser?
- create an EQ for your headphone!
- create an EQ for anechoic speaker measurements that you find on AudioScienceReview or ErinsAudioCorner (among others)
- create an room optimiser for your room, from simple to use to you are in control of every steps: a state of the art optimiser.
- compare one EQ with another. Customise to taste. Which one do you prefer?

*Sound of the Future* or *SotF* in short comes from the song from [Giorgio Moroder](https://en.wikipedia.org/wiki/Giorgio_Moroder) made popular by [Daft Punk](https://en.wikipedia.org/wiki/Daft_Punk). You can find many versions on Youtube. Here is an [official one](https://youtu.be/zhl-Cs1-sG4?si=H4hgakoEdQn-HMH6&t=73).

## A picture is worth a thousand words.

### Player

![sotf](./docs/images/sotf-0.5.3-light.png)

## How to use?

Download a release from our [repo](https://github.com/pierreaubert/sotf) on Github. If you like it, star the directory please. If you dont, please let us know why? All feedback is welcome: you can leave a comment on [github](https://github.com/pierreaubert/sotf/discussions/116) or on [AudioScienceReview](https://www.audiosciencereview.com/forum/index.php?threads/autoeq-for-speaker-and-headphone.66460/).

## Install

### Cargo

Install [rustup](https://rustup.rs/) first.

If you already have cargo / rustup, you can jump to:

```shell
cargo install just
just
```

On Linux or MacOS, select the correct install just command for your platform:
```shell
just install-...
```
On Windows,
```shell
.\player\windows\build-windows.bat
```

Then run post-install and download various data files
```shell
just post-install
just download-once
```

You can build or test with a simple:
```shell
just build
just test
```

In order to build the TUI version
```shell
just sotf-tui
```
and for the UI version:
```shell
just SotF
```

## Where is the code?

The code is in four parts:

- [math-audio](https://github.com/pierreaubert/math-audio) : a toolkit for DSP processing, FEM and BEM simulations
- [autoeq](https://github.com/pierreaubert/autoeq) : a toolkit for generating EQ from measurements (IIR, FIR, MSO, DSO etc)
- [gpui-toolkit](https://github.com/pierreaubert/gpui-toolkit) : a toolkit built on top of GPUI with widgets and graph support
- this repository [sotf](https://github.com/pierreaubert/sotf) which is mostly an audio backend and the UI and TUI. The backend audio has a few components:

  - an [audio engine](crates/engine/README.md) : an audio engine (process streams or files and output pcm to your audio device)
  - an [audio player](crates/player/README.md): a library doing track management and 3 players, a CLI for testing, a TUI (terminal) based one and a desktop one with a native UI.
  - a set of audio [plugins](crates/plugins/README.md):

    - host: a mini DAW that can run plugins in a list (like a rack) or in a graph (like a DAW)
	- visualisation: loudness, spectrum, lufs
	- classical: iir and fir EQ, compressor (and multi-band compressor), limiter, gain, matrix, resampler, multi-band expander, convolution, delay, crossover, loudness compensation
	- spatial: upmixer from 2.0 to 9.1.6, binaural, cross-talk cancelation
	- denoiser, declicker, polyphonic note detection
	- a/b testing


Why did you not reuse more code? The goal was to learn Rust and to learn other things I always wondered about:

- How to write an audio player? I took inspiration from camilladsp and wrote my own. I could have use Camilla (and I did at the beginning)
- Why are plotting library never perfect? I can usually go 90% of the way with most libraries but then I get block and then it gets complicated to get exactly what you want.
- Can I do everything in Rust from backend to fronted? I am not a fan of Typescript and the context switching between Rust and Typescript is not ideal for me. Using GPUI allow me to stay in Rust and be concentrated.
- Did LLM model progress enough to help building a complex app? Answer is yes since Opus 4.5 and Gemini 3.0. It is of course not perfect.
- Can I reuse my old c++ code with audio plugin? Answer is also yes, I translated most of them in Rust now. I am still unclear if I will be able to build AU plugins with GPUI but it is working for CLAP and VST3. I also get the plugins to work as AUplugin with some hacking and a bit of Swift.

## Toolkit

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

- engine: production quality
- plugins: code is good but some plugins need tuning.
- plugins/src-ffi: beta quality
- gpui-au: a bridge between AUv3 plugin and gpui
- player: production quality
- player/app-tui: good quality, can scan by 4k albums and play them with an TUI interface. It is good for testing parameters and plugins.
- player/app-gpui: experimental status


### MacOS specific: sotf-macos-hal

sotf-macos-hal crate builds a HAL (Audio Driver on MacOS) such that you can redirect all your music to this driver and benefit from corrected sounds all the time.

Status: experimental for HAL and ok for confbar.

## More pictures

### Upmixer

![Upmixer 2.0->5.1.4](./docs/images/upmixer.png)

