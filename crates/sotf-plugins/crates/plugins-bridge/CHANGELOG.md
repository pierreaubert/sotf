# 0.5.5

## Fixes (2026-08-12 review follow-up)

- Align the cross-format factory smoke test with MonoToStereo's fixed
  one-channel input contract.

# 0.5.4

## New

- Added missing plugins in docs and AU/Clap/VST3 bridges
- Added missing new-ish plugin to AU plugins repo
- Added an AAE plugin (experimental)
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added playlist support across the board

## Changes

- Long overdue split of denoiser into denoiser+declick+hiss-reducer+speach-denoiser
- Listening + bug hunting session on plugins
- Next ieration on AU plugins
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
