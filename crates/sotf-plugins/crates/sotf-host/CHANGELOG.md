# 0.5.90

## Fixes

- Route sidechain graph edges into extended per-frame input lanes instead of dropping them during input merge.
- Compact extended `InPlacePluginAdapter` output back to audio channels after processing so sidechain lanes are not exposed downstream.
- Grow host scratch input buffers before large-block copies to avoid panics on offline render or high-channel-count blocks.
- Add regression coverage for sidechain routing through an extended in-place plugin and large input blocks.
