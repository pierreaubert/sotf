# 0.5.15 - beta

Your room, your headphones, your sound — scientifically optimized. SotF is a free, privacy centric, open source code application that runs everywhere.

SotF integrates into one application: an configurable audio player with audio effects, a measurement system, a scientically optimised headphone and room correction system and a system wide sound manager. It strives to find a balance between having a lot of complicated tools and a basic audio system.

## AutoEQ/RoomEQ

- RoomEQ
  - Support for FIR and Mixed mode
  - Support multiple microphones for recording (tested up to 8)
  - Multi-position spatial robustness
  - Pre-Ringing Control
  - Identify room modes, early reflections and direct sound and apply different corrections to each ot them.
  - All pass filter on multi-subwoofer
  - Feature complete from my point of view but need testing for the complicated features.
  - Added export to CamillaDSP (all platform), APO (Windows), EasyEffects, Wavelet, PipeWire (Linux)

A lot of testing is needed but the system shows promises and will soon get close to SOTA products.

## Audio Plugins

Plugins quality is catching up with professional (and expensive) ones. The quality of the UI is where most of
the difference is.

- NEW: Ambisonic plugin with IAFM support
- NEW: Acoustic echo cancelation plugin (for video chat apps): implemented PBFDAF.
- NEW: Beamforming plugin with various options to adapt to rooms
- General
  - Use dual-window STFT to decouple analysis resolution from latency
  - Non uniform partitioned convolution (NUPC)
  - Lock free pattern everywhere, RTRB, basedrop etc to optimise for low latency
  - Pre-allocated processing pipeline with Latency Budget Tracking
  - Deep Learning for predictions (RNNoise: real-time, Demucs: near real time, Diffusion model: not real-time)
  - Better phase estimation (ESPRIT, RTPGHU), added better pitch detection
- AB:
  - Latency compensation, crossfade, autogain, dry/wet mode
- Band split & band merge, crossovers
  - Added LR crossover (not only cascaded BW filters)
- Binaural
  - Enable real time tracking
- Compressor, Gain, Limiter
  - Added RMS detection option
  - Added lookahead
  - Added program dependant release
  - Proper auto make up
  - Sidechain support
- Convolution
  - Support non uniform partitionning
  - Support IR resampling
- EQ:
  - Implemented form TDF-II
  - Corrected Shelves filter to support all Q (no approximation)
  - Improved high frequemmcy cramping via over sampling, orfanidis shelving is implemented but not visible yet.
- Denoiser
  - Added adaptive spectral smoothing
  - Added default noise floor at bootstrap
  - Speech specrtal envelope preservation
- Downmix
  - Now fully ITU-R BS.775 compliant
- Gate and expander
  - Added RMS detection mode
  - Added  measured auto-makeup
  - Allowed 0 Hz sidechain HPF
  - Added lookahead
- Fletcher-Munson
  - Compliand with ISO 226:2003
- Host: Too many features, getting closer to be able to support a full DAW :  Automatic Latency Compensation, Per-Node Bypass, Parallel Node Execution, Lock-Free Graph Updates, Sidechain Port Support, Buffer Safety Guard, Memoize Latency Computation
- Mono to Sterero
  - Frequency-dependent decorrelation
  - Added explicit Haas delay
- PND
  - Per channel analysis
  - Confidence based by-pass
  - Lowered partial minimum
  - Phase vovocoder mode
- Resampler
  - Support for variable chunks
  - Added some quality preset
  - Dynamic ration support for smooth ratio changes
- Upmixer
  - Implemented per time-frequency tile analysis
  - Moved from coarse band resolution to per STFT bin
  - Estimate diffuseness properly (and it is conservative of energy)
  - Improved ambient extraction: from left/right to use of diffuse field estimation
  - Moved from PCA to DSU for dialogue
  - Improved spatial analysis (still work to do, ML model coming soon)
- XTC
  - Added proper support for HRTF
  - Improved regulatization of 2x2 matrices
  - Changed tanh estimation that hard limit too much to distributed gain reduction.
  - Added modeling for delays (need testing)


## Audio engine

- Gapless playback
- RT thread priority
- Fire & forget commands
- Support playlist queues
- Latency compensation

## Player

- Preference windows supports a lot of configuration (language, themes, performance, ...)
- TUI now support most use cases and is surprising easy to use even on a remote configuration.
- Works well on MacOS, need testing on Linux and Windows

## Website

Did you notice? we do have a website now [jump here](https://sotf.spinorama.org)

## What is coming?

- System wide correction: working on MacOS, to be tested on Windows (WASAPI for now) and Linux (PipeWire, Alsa).
- ML model to improve predictions in plugins: there are fully open source training available with public data. Medium quality for now.
- iOS support: iPad and iPhone.
- AU, VST and other formats for plugins: mostly a time questions than a technical question
- Streaming in and streaming out (UpnP, MDP, etc)

## What is not ready yet!


- AppleTV support but I dont have one so testing is complicated :) but that would be great for the home cinema people. It currently work on the simulator.
- Android support (need a device, ordered a cheap one)
- DAW mode (far away, but not far away from a simple one)


