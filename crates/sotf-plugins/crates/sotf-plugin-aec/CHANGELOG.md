# 0.5.1

## Fixes

- Added copy_adaptive_state_from, so adaptive weights/FDL/error state can be promoted.
- Transfer_bg_to_fg() now copies background state into foreground instead of resetting it.
- Added input/output buffer-size validation and made the output queue track length explicitly, resizing only for oversized host-buffer edge cases so unread output cannot be overwritten.

## New tests:

- Background-to-foreground transfer preserves adaptive state
- Malformed input/output buffers return errors instead of panicking
- Large host blocks preserve every produced output sample
