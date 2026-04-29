# 0.5.91

## Fixes

- Invalidate built graph topology after direct `add_node` and `add_edge` mutations so processing rebuilds stale stages and buffers before the next block.
- Size latency-compensation delay buffers to the routed channel count for channel-mapped graph edges.
- Propagate inner `process_in_place` errors from oversampled plugin wrappers.

## Performance

- Replace per-chunk oversampler residual shifting with input/output cursors and a reusable chunk buffer, reducing hot-path memory traffic for oversampled processing.

# 0.5.90

## Fixes

- Route sidechain graph edges into extended per-frame input lanes instead of dropping them during input merge.
- Compact extended `InPlacePluginAdapter` output back to audio channels after processing so sidechain lanes are not exposed downstream.
- Grow host scratch input buffers before large-block copies to avoid panics on offline render or high-channel-count blocks.
- Add regression coverage for sidechain routing through an extended in-place plugin and large input blocks.
