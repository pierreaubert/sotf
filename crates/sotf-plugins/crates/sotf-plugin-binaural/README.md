# sotf-plugin-binaural

SOTF Binaural Decoder plugin for HRTF-based binaural rendering.

Renders supported multichannel layouts to binaural stereo using causal,
partitioned overlap-add convolution with SOFA HRTFs. The renderer has a fixed
`fft_size` host latency, strict speaker-layout admission, transactional SOFA
replacement/head tracking, source-owned broadband room reflections, optional
diffuse-field EQ, and late reverb.
