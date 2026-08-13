# sotf-plugin-denoiser

SOTF broadband spectral denoiser using IMCRA/MCRA noise estimation and Wiener filtering.

It provides 2048-point quality and 512-point low-latency STFT modes, decision-directed SNR,
captured noise profiles, psychoacoustic masking, spectral subtraction, formant and transient
protection, and optional dual-resolution analysis. Reported latency is one FFT (2048 or 512
samples); processing is preallocated and allocation-free after construction.

Spatial mode processes coherent channel pairs. Stereo and 3.0 use the front L/R pair. Standard
5.1 adds side L/R, and standard 7.1 adds side and rear L/R. Centre, LFE, and any unmatched channel
remain on the ordinary per-channel denoising path.

Serialized percentage-like values are normalized fractions (`0.70` means 70%); UI scaling is
presentation only. Noise-profile learning lasts approximately one second at every sample rate and
in both FFT modes.

See `USAGE.md` for parameters and examples and `UI.md` for the compiled layout contract.
