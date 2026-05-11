# 0.5.5

## Fixes

- **CRITICAL**: Fixed reversed attack/release semantics. Attack now correctly controls gate opening speed and Release controls closing speed.
- **CRITICAL**: Fixed linked stereo monitoring cache `is_open` always reporting `true` because stale envelope values for channels 1+ were never updated.

# 0.5.4

## New

- The sidechain is not steep enough: add steeper crossover
- Added missing parameters for new plugins

## Fixes

- Fixed again parameters for plugins. TODO: think about doing it the hard way with a trait per plugin
- Did a round of test fixing
- Fixed a lot of tests and then the corresponing code

## Changes

- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details (wave 3)
- Massive update to plugins, see individual markdown plan for details
