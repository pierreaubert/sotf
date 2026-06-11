---
title: Plugin Reference
description: Complete reference for all SotF audio plugins.
---

SotF includes 45 audio processing plugins. Click any plugin for its full parameter reference.

## All Plugins

| Plugin | Description |
|--------|-------------|
| [Parametric EQ](/reference/plugins/eq/) | Biquad-based parametric equalizer with peak, shelf, and pass filters. Supports multiple filter bands for precise frequency response shaping. |
| [Gain](/reference/plugins/gain/) | Simple volume control with smooth gain ramping to prevent clicks. |
| [Compressor](/reference/plugins/compressor/) | Dynamic range compression with configurable threshold, ratio, attack, release, and makeup gain. |
| [Expander](/reference/plugins/expander/) | Dynamic range expansion with configurable threshold, ratio, attack, release, and range. Opens up dynamics below the threshold. |
| [FIR Designer](/reference/plugins/fir-designer/) | Linear-phase and minimum-phase FIR equalizer designed from parametric EQ bands. |
| [Multiband Compressor](/reference/plugins/multiband-compressor/) | Per-band dynamic range compression with 2-5 frequency bands and independent compressor settings per band. |
| [Multiband Expander](/reference/plugins/multiband-expander/) | Per-band dynamic range expansion with 2-5 frequency bands and independent expander settings per band. |
| [Gate](/reference/plugins/gate/) | Noise gate that silences audio below a configurable threshold. |
| [HAL Input](/reference/plugins/hal-input/) | Reads audio from a system HAL device into the engine. Used by the macOS system-wide daemon. |
| [HAL Output](/reference/plugins/hal-output/) | Writes audio from the engine to a system HAL device. Pair with HAL Input to route system audio through a plugin chain. |
| [Limiter](/reference/plugins/limiter/) | Peak limiter to prevent clipping. Ensures output never exceeds the ceiling level. |
| [Delay](/reference/plugins/delay/) | Audio delay with configurable delay time per channel. |
| [Convolution](/reference/plugins/convolution/) | FFT-based convolution engine for applying impulse responses (room correction, cabinet simulation, reverb). |
| [Matrix Mixer](/reference/plugins/matrix/) | Channel matrix mixing with per-routing gain control. Route any input channel to any output channel. |
| [Channel Mute/Solo](/reference/plugins/channel-mute-solo/) | Per-channel mute, solo, and dim controls with smooth fade transitions. |
| [Upmixer](/reference/plugins/upmixer/) | Stereo to surround upmixing (2ch to 5.0/5.1/7.1) using FFT-based spatial decomposition and VBAP panning. |
| [Downmix](/reference/plugins/downmix/) | Surround to stereo downmixing with configurable channel contributions. |
| [Binaural Renderer](/reference/plugins/binaural/) | HRTF-based 3D spatial audio rendering. Converts multichannel audio to binaural headphone output using SOFA files. |
| [Crossfeed](/reference/plugins/crossfeed/) | Headphone crossfeed that simulates speaker spacing. Supports Bauer, Meier, and multiband algorithms. |
| [Crossover](/reference/plugins/crossover/) | N-way crossover with selectable filter family (LR24 or linear-phase FIR) and per-channel configuration. |
| [Crosstalk Cancellation (XTC)](/reference/plugins/xtc/) | Crosstalk cancellation for speaker playback. Creates a wider stereo image by cancelling inter-speaker interference. |
| [Perceptual Noise Diffusion](/reference/plugins/pnd/) | Perceptual noise diffusion (PND) for improving perceived audio quality through controlled noise shaping. |
| [Resampler](/reference/plugins/resampler/) | High-quality sample-rate converter using sinc interpolation. Supports fixed and dynamic resampling ratios. |
| [Loudness Compensation](/reference/plugins/loudness-compensation/) | Equal-loudness contour compensation (Fletcher-Munson). Adjusts frequency response based on playback volume to maintain perceived tonal balance. |
| [Mono to Stereo](/reference/plugins/mono-to-stereo/) | Converts mono signals to stereo output. |
| [Denoiser](/reference/plugins/denoiser/) | Audio denoising using MCRA (Minima Controlled Recursive Averaging) and Wiener filtering. |
| [A/B Compare](/reference/plugins/ab-compare/) | Side-by-side A/B comparison. Instantly toggle between processed and bypass to evaluate your plugin chain. |
| [Band Split](/reference/plugins/band-split/) | Splits the audio signal into separate frequency bands for independent processing. |
| [Band Merge](/reference/plugins/band-merge/) | Merges previously split frequency bands back into a single signal. |
| [Acoustic Echo Cancellation](/reference/plugins/aec/) | Cancels acoustic echoes from microphone input using reference signal correlation. |
| [Beamformer](/reference/plugins/beamformer/) | Microphone array beamforming for directional audio capture. |
| [De-Esser](/reference/plugins/de-esser/) | Sibilance reduction targeting harsh high-frequency content (s, t, sh sounds). |
| [Dither](/reference/plugins/dither/) | Adds dither noise for bit-depth reduction, minimizing quantization distortion. |
| [Dynamic EQ](/reference/plugins/dynamic-eq/) | Frequency-dependent dynamic equalizer that adjusts filter gain based on signal level. |
| [Linear Phase EQ](/reference/plugins/linear-phase-eq/) | Zero-phase parametric equalizer using FFT convolution. No phase shift, but adds latency. |
| [Saturation](/reference/plugins/saturation/) | Harmonic saturation and soft clipping for adding warmth and character. |
| [Spectral Compressor](/reference/plugins/spectral-compressor/) | Frequency-dependent compression operating in the spectral domain for transparent dynamic control. |
| [Stereo Imager](/reference/plugins/stereo-imager/) | Controls stereo width from mono to extra-wide, using mid/side processing. |
| [Transient Shaper](/reference/plugins/transient-shaper/) | Shapes attack and sustain characteristics of audio transients. |
| [Ambisonics](/reference/plugins/ambisonics/) | Ambisonics encoding and decoding for immersive spatial audio. |
| [Spectrum Analyzer](/reference/plugins/spectrum-analyzer/) | FFT-based spectrum analysis with configurable bin count, frequency range, smoothing, and tilt correction. |
| [Active Acoustic Enhancement](/reference/plugins/aae/) | Active acoustic enhancement using psychoacoustic processing to improve perceived clarity, presence, and depth. |
| [Declick](/reference/plugins/declick/) | Removes clicks, pops, and short transient defects from audio (vinyl restoration, recording artifacts). |
| [Hiss Reducer](/reference/plugins/hiss-reducer/) | Reduces high-frequency hiss and tape noise while preserving program content. |
| [Speech Denoiser](/reference/plugins/speech-denoiser/) | Neural speech-focused denoiser (RNNoise-derived) optimized for voice and dialogue cleanup. |
