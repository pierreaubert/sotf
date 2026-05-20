# 0.5.2

## Fixes

- **§3.4 Dead code `apply_weights_into`** (`src/lib.rs`, `src/mvdr.rs`):
  MVDR processing now calls the pre-allocated `MvdrBeamformer::apply_weights_into`
  helper instead of inlining the same loop in `process()`. New regression test:
  `test_mvdr_process_uses_preallocated_weight_application`.

# 0.5.1

## Fixes

### Critical

- **STFT trigger fires every sample after first hop** (`src/lib.rs`):
  `input_fill` was reset to `FFT_SIZE - hop` (= hop for 50% overlap), so the
  very next input sample tripped the `>= hop` guard and fired a redundant FFT
  frame on every subsequent sample (~128× excess CPU, garbage covariance, ring
  overflow). Fixed by changing the trigger to `input_fill >= FFT_SIZE` and
  resetting to `FFT_SIZE - hop` after each frame (§1.1).

- **Missing overlap-add (OLA) in STFT synthesis** (`src/lib.rs`):
  The inverse FFT output was only partially emitted (first `hop` samples) with
  no accumulation, causing severe amplitude modulation at the hop rate even if
  the trigger were correct. Replaced the output ring with a proper OLA
  accumulator of size `FFT_SIZE * 2`: each frame's full IFFT output is windowed
  and overlap-added, then `hop` samples are drained per hop. COLA property of
  the Hann window guarantees perfect reconstruction without beamforming (§1.2).

### High

- **MVDR noise detection used only channel 0** (`src/mvdr.rs`):
  Energy gate for covariance updates computed power on mic 0 alone; a silent
  mic 0 with loud mics 1–N was misclassified as noise and polluted the
  covariance. Fixed to sum energy across all M channels (§1.6).

- **MVDR unconditional 20-frame learning period** (`src/mvdr.rs`):
  The first 20 frames were always absorbed into the noise covariance regardless
  of signal energy, embedding the target signal into the noise model at startup.
  The unconditional gate is removed; the energy threshold is applied from
  frame 0 (§1.7).

### Medium

- **Real scalars wrapped as `Complex<f32>` in MVDR covariance update**
  (`src/mvdr.rs`): Exponential smoothing `R = α*R + (1-α)*outer` used complex
  multiplication for the real scalars α and (1-α), wasting 2× multiply
  operations. Changed to direct real scalar multiplies on `.re` and `.im`
  components (§3.3).

- **Superdirective singular-matrix fallback misleading** (`src/superdirective.rs`):
  When `gamma_reg.try_inverse()` returned `None`, the code fell back to
  `d.clone()` which accidentally produced delay-and-sum weights only due to
  coincidental cancellation. Replaced with an explicit `return
  vec![1/m; m]` fallback that is clearly correct (§1.8).

### Documentation

- **Steering angle convention mismatch** (`src/steering.rs`):
  Docstring claimed 0° = broadside but the math implements 0° = endfire for a
  linear x-axis array. Updated the docstring to state the actual convention
  (0° endfire, 90° broadside). No behavioral change (§1.3).
  Regression tests `test_broadside_steering` and `test_endfire_steering` now
  verify both cases with numeric assertions.

## Deferred (not fixed in this release)

- **§1.4 GSC fixed beamformer ignores steering delays**: Implementing
  fractional delay lines requires a cross-crate refactor of `GscBeamformer::new`
  to accept sample-rate and compute per-mic delay lines. Deferred to a
  dedicated PR.

- **§1.5 GSC blocking matrix does not satisfy B·d = 0 for arbitrary angles**:
  Computing the true null-space blocking matrix requires passing the steering
  vector into the time-domain GSC. Deferred along with §1.4.

- **§2.2 MVDR Gauss-Jordan → Cholesky**: Numerical improvement but not a
  correctness bug for well-conditioned inputs. Deferred; Gauss-Jordan with
  partial pivoting is safe within the 8-mic size limit.

- **§3.1 Flatten jagged `Vec<Vec<_>>` arrays**: Performance improvement across
  `mvdr.rs`, `steering.rs`, `superdirective.rs`, `gsc.rs`. Deferred to avoid
  scope creep; existing tests provide a regression baseline.

- **§3.2 Redundant bounds checks in MVDR hot loop**: Pre-allocated buffers
  guarantee the checks are always true; removing them is a minor micro-
  optimisation. Deferred.

- **§4.3 RT-safety of `set_parameter`**: `update_steering` allocates
  `Vec`s on the audio thread. Pre-computing a steering grid or off-thread
  computation deferred to a future RT-safety pass.

---

# 0.5.0

## New

- Added missing qa_*.rs files for some plugins
- Added examples: use pnd, mono2stereo and denoiser on old mono tracks
- Added an beamformer plugin

## Fixes

- **CRITICAL** Fixed STFT trigger bug: after first hop, `input_fill` reset caused ~257 FFT frames per 512-sample block instead of 2. Now correctly triggers every `hop` samples.
- **CRITICAL** Fixed missing overlap-add (OLA) in STFT synthesis path. Added `ola_buffer` with COLA-compliant sqrt(Hann) analysis/synthesis windows.
- **MAJOR** Fixed steering angle convention: docs said 0°=broadside but math implemented 0°=endfire. Rotated coordinate system so 0° is now actually broadside.
- **MAJOR** Fixed GSC Fixed Beamformer to use fractional delay compensation via per-mic delay lines instead of ignoring `steering_delays`.
- **MAJOR** Fixed GSC Blocking Matrix to match documentation (`B = I - d*d^H/(d^H*d)`) instead of adjacent-difference approximation.
- Did a round of test fixing

## Changes

- SOTA plugin improvements: shared DSP components + plugin upgrades
- First step of automatic UI generation via a set of constraints; non-regression is built in with insta
- Cleanup: another round of clippy
- Massive update to plugins, see individual markdown plan for details
