# 0.5.8

## New

- Added missing qa_*.rs files for some plugins
- Added missing parameters for new plugins

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- Next iteration on UI and testing for plugins this time with native look&feel
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Assed ISP mode for the limiter
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 5)
- Massive update to plugins, see individual markdown plan for details (wave 3)
# Unreleased

## Fixes
- Fixed catastrophic CPU waste in feed-forward lookahead scan: replaced O(lookahead_len × channels) per-sample scan with amortized O(1) running-max update.
- Fixed 32-channel hard cap: `ch_peaks` now dynamically sized to `channels`, so all channels are analyzed.
- Fixed ISP correction decay operating in wrong domain: decay now happens in linear gain space before converting back to dB, matching the release time constant.

