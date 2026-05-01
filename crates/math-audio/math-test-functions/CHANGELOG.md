# 0.5.0

## New

- Added/updated documentation for autoeq and math-audio crates.

## Fixes

- Fixed propagation of tolerance and absolute tolerance from the app to the backend (explained why optimisation was fast but not accurate).

## Changes

- Bumped math crates to 0.5 alongside the wider math-audio re-versioning (iir-fir adds f32 support; math-rir becomes band-limited and linear phase).
- Split the autoeq UI out of the gpui UI Kit; this crate stays focused on optimisation test functions.
- Reorganised the workspace `crates/` tree for easier maintenance and crates.io publishing.
