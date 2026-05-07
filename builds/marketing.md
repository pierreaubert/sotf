# SotF -- Sound of the future

A modern music player built for people who care about how their music sounds.

SotF combines a fast, lightweight player with a complete audio-processing toolkit so you can shape every part of the listening experience: room correction for your speakers, headphone EQ for your cans, stereo-to-surround upmixing, and a chain of professional-grade plugins — all running locally on your machine, in real time.

## Play your library, your way

- Browse music, organise albums and playlists, and play any common format: FLAC, MP3, WAV, OGG, M4A, AAC, ALAC, and MP4. ReplayGain keeps loudness consistent across tracks. The library scales to tens of thousands of files without slowing down.
- Make any room sound right. Most rooms ruin good speakers — bass nulls, harsh peaks, comb filtering. SotF's guided room-correction wizard uses measurement-based EQ to flatten your in-room response. Bring your own measurements, or capture them with a USB microphone using the built-in recorder.
- Headphones that actually sound neutral. Every headphone has a frequency-response personality, and almost none match a neutral target. The
 headphone-EQ wizard pulls measured response curves from a public database and computes a parametric EQ that takes your cans to the target curve of your choice — Harman, flat, or custom. One click, audibly different.

## Real-time DSP plugin chain

Slot processing into a free-form chain: parametric and dynamic EQ, linear-phase EQ, multiband compressor and expander, transient shaper, saturation, dither, crossfeed, stereo imager, de-esser, hiss reducer,  speech denoiser, convolution reverb, beamformer, ambisonics rendering, and more. Build presets, A/B them, and audition changes against the original signal instantly.

## Stereo, but bigger

The upmixer turns any stereo source into 5.0 up to 9.1.6 surround using psychoacoustically-grounded matrix decoding — clean dialogue centering, ambient rear extraction, no phasey artifacts. Great for music, films, and live recordings on any multi-speaker setup.

## Engineered for serious listeners

Under the hood, SotF runs a four-thread native audio engine (decode → process → playback → manager) written in Rust for low-latency, zero-compromise real-time performance. CEA2034 / Spinorama metrics for objective speaker evaluation. Differential-evolution optimiser for AutoEQ filter design. Room-impulse-response analysis tools.

Everything is offline-capable and local — your music never leaves your machine.

## Two faces, one engine

Use the polished desktop UI with charts, plugin chains, and visual EQ. Or live in the terminal — the full TUI gives you the same feature set with mouse and keyboard, perfect for headless servers and minimal setups.

## Cross-platform

Native builds for macOS (Apple Silicon and Intel), Linux (x86_64 and ARM64), and Windows. The same DSP wherever you listen.

## Open and accountable

Source-available on GitHub. Reproducible builds. Cosign-signed Linux artefacts and Apple-notarised macOS releases — you can verify exactly what you're running.

Whether you've spent thousands on hi-fi or just want your everyday headphones to sound their best, SotF gives you the tools audio engineers use, in a player you'll want to live in.

Free download. No account. No telemetry.
sotf.spinorama.org
