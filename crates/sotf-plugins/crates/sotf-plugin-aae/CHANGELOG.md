# 0.5.2

- ER delay storage now provisions max preset delay plus max modulation headroom, uses fixed tap/state arrays, and supports preset changes without reallocating.
- Delay reads are clamped to capacity so out-of-range modulation cannot panic.
- FDN delay lines allocate for max room size up front; room_size updates now adjust delay lengths in place.
- Process buffer sizes now return clean errors, AAE reports zero host latency, content-aware dialogue ducking is implemented, ER stale tap summing is fixed, routing gain buffers are reused, and cached parameter updates avoid rebuilding the whole parameter list on every set.

# 0.5.1

Bug fixes

# 0.5.0

Initial version

