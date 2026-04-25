# 0.5.22

## Fixes

- Account for multi-chunk input in `output_frames_for_input()` so hosts allocate enough output space.
- Include pending residual frames when estimating output capacity.
- Add regression coverage for multi-chunk resampling estimates.
