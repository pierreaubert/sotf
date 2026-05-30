# 0.5.3

## Fixes

- Collinear retry jitter now uses a deterministic coordinate hash instead of trigonometric coordinate patterns.
- `Delaunay::find` now documents its local greedy-descent contract.
- Voronoi clipping and corner walking now have debug guards for unexpected safety-limit and edge-code states.
- Voronoi corner insertion now de-duplicates with the same bounds-relative epsilon used by cell polygon cleanup.
- Voronoi bounds now have a documented finite, ordered precondition with a debug assertion.

# 0.5.2

## Fixes

- `is_collinear` now uses `cross.abs()` instead of one-sided `cross > 1e-10`, preventing misclassification of CW-oriented triangles.
- Collinearity and circumcenter thresholds are now scaled by coordinate magnitude, fixing false positives/negatives for very small or very large coordinates.
- `Voronoi::edgecode` and duplicate removal in `cell_polygon` now use epsilon comparisons relative to bounding-box size instead of exact floating-point equality.
- `Voronoi::simplify` now removes diagonal collinear points using a 2D cross-product test instead of only axis-aligned collinearity.
- NaN coordinates in collinear sorting are now handled deterministically via `total_cmp`.

## Tests

- Added tests for micro-scale and large-scale collinearity detection.
- Added tests for diagonal collinear simplification and near-boundary edgecode behavior.
- Added test verifying NaN inputs do not panic.

# 0.5.1

## New

- Added/updated documentation to autoeq and math-audio crates
- Added a delaunay and voronoi algo

## Fixes

- Fixed a delaunay error with too flat triangle

## Changes

- Bumped math crates to 0.5: iir-fir now also work with f32, rir is band limited and linear phase
