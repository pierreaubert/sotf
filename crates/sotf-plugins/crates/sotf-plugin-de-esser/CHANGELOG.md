# 0.5.4

## New

- Added missing qa_*.rs files for some plugins

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
# Unreleased

## Fixes
- Fixed block-constant mix smoother: replaced `next_n(num_frames)` with per-frame linear ramp to prevent zipper noise during mix automation.
- Fixed split-band crossover order: changed from 1st-order (6 dB/octave) to 4th-order (24 dB/octave) for proper band isolation.

