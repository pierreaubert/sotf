# 0.5.2

## Fixes

- `keanes_bump_objective` now remains finite at the origin for both 2D and
  higher-dimensional inputs.
- `keanes_bump_constraint1` treats missing first-four dimensions as zero so
  short inputs violate the product constraint instead of appearing satisfied.
- Corrected stale comments for `step` and `alpine_n2`.

## Documentation

- Documented benchmark aliases and near-duplicates in the README.

## Tests

- Added direct constrained-helper coverage for Keane, Mishra's Bird,
  Binh-Korn, and Rosenbrock disk constraints.
- Added local coverage for Hartman 6D and Lampinen simplified helpers.

# 0.5.1

## Fixes

- **pinter**: Fixed `cos(1 + xi)` → `cos(xi)` to match literature formula; global minimum is now correctly `0.0` at origin.
- **holder_table**: Fixed `sin(x1*x2)` → `sin(x1)*cos(x2)` to match standard formula.
- **cross_in_tray**: Fixed `sin(x1*x2)` → `sin(x1)*sin(x2)` to match standard formula.
- **schaffer_n4**: Fixed `sin²(x1²-x2²)` → `cos²(sin(|x1²-x2²|))` to match standard formula.
- **ackley_n3**: Complete formula rewrite to match literature:
  `-200*exp(-0.02*sqrt(x²+y²)) + 5*exp(cos(3x) + sin(3y))`.
- **vincent**: Fixed `sin(10*xi)` → `sin(10*ln(xi))` to match standard formula.
- **langermann**: Added missing negation; function now returns negative sum as per standard.
- **keanes_bump_objective**: Fixed metadata minimum sign (`0.673668` → `-0.673668`) to match negated implementation.
- **trid**: Fixed metadata minimum point (`[1,2]` → `[2,2]`) to match implementation.
- **dixons_price**: Fixed metadata minimum point (`[1, 0.5]` → `[1, 1/√2]`) to match 2D formula.
- **quadratic**: Fixed metadata minimum (`[0.19388, 0.48513], -3873.7243` → `[0, 0], 0`) to match `sum(xi²)` implementation.
- **bird**: Already fixed in previous session (literature formula).

## Internal

- Expanded `call_function` test dispatcher from ~26 to all 105 functions, exposing metadata/implementation mismatches.
- All 60 lib tests + 3 doc-tests now pass. Clippy clean.

# 0.5.0

## New

- Added/updated documentation for autoeq and math-audio crates.

## Fixes

- Fixed propagation of tolerance and absolute tolerance from the app to the backend (explained why optimisation was fast but not accurate).

## Changes

- Bumped math crates to 0.5 alongside the wider math-audio re-versioning (iir-fir adds f32 support; math-rir becomes band-limited and linear phase).
- Split the autoeq UI out of the gpui UI Kit; this crate stays focused on optimisation test functions.
- Reorganised the workspace `crates/` tree for easier maintenance and crates.io publishing.
