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

### Native Player

![sotf](./site/public/images/0.5/sotf-gpui-v0.5-player.png)

### Terminal Player

![sotf](./site/public/images/0.5/sotf-tui-v0.5-player.png)

## How to use?

- Windows: [download from Microsoft Store](https://apps.microsoft.com/detail/9NXCMV37NXJ7).
- Apple: [download from the App Store](https://apps.apple.com/ch/app/sound-of-the-future/id6754237332).

Stable desktop releases are on the platform stores. Beta releases and command-line artifacts are on [GitHub Releases](https://github.com/pierreaubert/sotf/releases), with another copy on our [website](https://sotf.spinorama.org).

If you like it, please star the directory on GitHub. If you dont, please let us know why? All feedback is welcome: you can leave a comment on [github](https://github.com/pierreaubert/sotf/discussions/116) or on [AudioScienceReview](https://www.audiosciencereview.com/forum/index.php?threads/autoeq-for-speaker-and-headphone.66460/).

### Main functions

- An **audio player** (nothing special but useful to have bundle with the other applications)
- A **audio recorder** to measure your speakers
- A system to **optimise** the sound based on your measurements for speakers and from internet measurements for headset. We claim to have a SOTA optimiser (a bit early for it but improving month after month if you want to a engineer view of it).
- A large set of audio plugins that do spatial audio, binaural, denoiser, limiter, compressor, upmixer, equalizer, room optimiser, etc.
- A systemwide DSP that allow you to run the roomEQ system continously for all applications on your computer. Currently working well on MacOS and needs to be ported/tested on Linux and Windows.

The main UI application shows you the audio player. You can access the other functions via the menu bars.

## For developpers

This is mainly a Rust application with some python and shell scripts. We want to keep it portable so we minimise C/C++ dependencies.

### Cargo

Install [rustup](https://rustup.rs/) first.

If you already have cargo / rustup, you can jump to:

```shell
cargo install just
just
```

On Linux, select the correct install just command for your platform:
```shell
just install-...
```

On MacOS, select the correct install just command for your platform:
```shell
just install-macos
```
On Windows,
```shell
.\scripts\build-windows.bat
```

Then run post-install:
```shell
just post-install
```

Then download various data files
```shell
just download-once
```

You can build or test with a simple:
```shell
just build
just test
```

For QA there is a
```shell
just qa
```
that takes some time to run.

In order to build the audio player, we have 3 versions: a cli (command line interface), a tui (terminal UI) and a desktop.
```shell
cargo build --bin player-cli --release -p app-cli
just tui
just gpui
```
Since it is a rust stack, the binaries are generated in `target/release/` directory and you can execute them from there.

In order to build signed binaries (required on MacOS):
If you are a developper, you can self sign them with
```shell
./scripts/build-systemwide.sh
./scripts/build-dmg-sotf.sh
```

In order to publish them, you need to have an Apple developper ID:
```shell
./scripts/build-systemwide.sh --sign --notorized
./scripts/build-dmg-sotf.sh --sign --notorized
```

## Where is the code?

### SotF

This repository [sotf](https://github.com/pierreaubert/sotf) which is mostly an audio backend and the UI and TUI. The backend audio has a few components:

- an [audio engine](crates/sotf-engine/README.md) : an audio engine (process streams or files and output pcm to your audio device)
- an [audio player](crates/sotf-player/README.md): a library doing track management and 3 players, a CLI for testing, a TUI (terminal) based one and a desktop one with a native UI.
- a set of audio [plugins](crates/sotf-plugins/README.md):

  - host: a mini DAW that can run plugins in a list (like a rack) or in a graph (like a DAW) visualisation: loudness, spectrum, lufs
  - classical: iir and fir EQ, compressor (and multi-band compressor), limiter, gain, matrix, resampler, multi-band expander, convolution, delay, crossover, loudness compensation
  - spatial: upmixer from 2.0 to 9.1.6, binaural, cross-talk cancelation, mono to stereo, dowmixer.
  - denoiser, declicker, polyphonic note detection
  - a/b testing

### Math related audio toolkit

The math related crates (`math-test-functions`, `math-optimisation`, `math-iir-fir`, `math-dsp`, `math-rir`, `math-delaunay`) now live in their own repository:

- [math-audio](https://github.com/pierreaubert/math-audio)

### SOFA reader

The pure-Rust SOFA/HRTF reader now lives in its own repository:

- [sofa-reader](https://github.com/pierreaubert/sofa-reader)

### Automatic EQ

The automatic EQ crates now live in their own repository:

- [autoeq](https://github.com/pierreaubert/autoeq) with CLI binaries and optimization logic: it allows optimising the sound from:
  - `autoeq`: optimise anechoic data for speakers and data from a headphone.
  - `convert-recording`: automatically migrate old recordings to the latest format.
  - `roomeq`: optimise a set of speakers in a room.

The following related crates remain in this repository:

### Symphonia add-ons

The Symphonia-compatible SACD, DSD, DST, and WavPack decoder/format crates now live in their own repository:

- [symphonia-add-ons](https://github.com/pierreaubert/symphonia-add-ons)


### GPUI toolkit

GPUI is a portable UI library written by the team behind the Zed editor. The toolkit [gpui README](https://github.com/pierreaubert/gpui-toolkit) provides everything to build complex applications. Linux, MacOS and Windows are supported. The support for iOS is progressing (working), AU plugins (working), tvOS (in progress), Android (not yet supported).


## FAQ

Why did you not reuse more code? The goal was to learn Rust and to learn other things I always wondered about:

- How to write an audio player? I took inspiration from camilladsp and wrote my own. I could have use Camilla (and I did at the beginning)
- Why are plotting library never perfect? I can usually go 90% of the way with most libraries but then I get block and then it gets complicated to get exactly what you want.
- Can I do everything in Rust from backend to fronted? I am not a fan of Typescript and the context switching between Rust and Typescript is not ideal for me. Using GPUI allow me to stay in Rust and be concentrated.
- Did LLM model progress enough to help building a complex app? Answer is yes since Opus 4.5 and Gemini 3.0. It is of course not perfect.
- Can I reuse my old c++ code with audio plugin? Answer is also yes, I translated most of them in Rust now. I am still unclear if I will be able to build AU plugins with GPUI but it is working for CLAP and VST3. I also get the plugins to work as AUplugin with some hacking and a bit of Swift.

### MacOS specific: systemwide

On MacOS the sound managent system does not make it easy to route audio between applications.

We added a few components:
- driver-hal: a HAL (Hardware Abstraction Layer) to route audio between applications. It is similar to BlackHole (https://github.com/BlackHoleSound/BlackHole) but it allow to route the audio through a DAW with plugins.
- daemon: a Rust deamon software that read audio from the HAL driver, apply a chain of plugins (typically to correct the sound of your headphone or software) and then send it to a hardware audio interface.
- toolbar: a software that allow to configure the daemon and HAL driver. It sits in the macos toolbar.

## More pictures

### Plugins list

![Upmixer 2.0->5.1.4](./site/public/images/0.5/sotf-tui-v0.5-plugins.png)

### EQ

![Parametric EQ](./site/public/images/0.5/sotf-gpui-v0.5-eq.png)

### Upmixer

![Upmixer 2.0->5.1.4](./site/public/images/0.5/sotf-gpui-v0.5-upmixer.png)
![Upmixer 2.0->5.1.4](./site/public/images/0.5/sotf-tui-v0.5-upmixer.png)
