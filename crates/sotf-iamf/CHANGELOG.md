# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-07-08

### Added
- QA-IAMF-001 malformed-input test suite in `tests/qa_iamf_001.rs`:
  truncated headers, oversized descriptors, bad magic bytes, zero-length
  payloads, unknown codec ids, out-of-range counts, and unbounded allocation
  attempts.
- Synthetic end-to-end decode test for a minimal LPCM IAMF stream.
- Additional parser bounds: `num_subblocks` loops in parameter definitions and
  mix-gain configs are now capped by remaining bytes; parameter-block subblock
  allocation is kind-aware (MixGain/DemixingInfo capped by payload, ReconGain
  by `MAX_LEB128_CAPACITY`); mix-presentation rendering and loudness extension
  skips are bounds-checked.

### Changed
- Documented release support level as **Experimental** in new
  `RELEASE_SCOPE.md`; updated `README.md` to reflect actual codec support.

## [0.1.0] - 2025-05-13

### Added
- Initial release of pure-Rust IAMF decoder.
- OBU (Open Bitstream Unit) parsing for IAMF v1.1.0 descriptors and temporal units.
- Codec support for Opus, AAC, FLAC, and PCM substreams.
- Ambisonics and speaker-layout rendering via `sotf-plugin-ambisonics`.
- Pre-allocated decode path with zero heap allocations in the hot loop.
