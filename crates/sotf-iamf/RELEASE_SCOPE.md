# sotf-iamf Release Scope

## Support level: Experimental

`sotf-iamf` is currently **experimental** and is **not release-supported**.

## Rationale

The crate implements IAMF v1.1.0 descriptor and temporal-unit parsing, plus
channel-based and scene-based rendering to SOTF speaker layouts. However, the
following gaps make it unsuitable for a production release:

- **Codec decoding is incomplete.** Only the LPCM substream decoder is
  implemented natively. Opus, AAC-LC, and FLAC substreams explicitly error out
  with `UnsupportedCodec` because they require engine-level Symphonia
  integration that is not yet wired in.
- **No real-world test assets.** There are no `.iamf` sample files in
  `data_tests/` or in the crate, so decode-path coverage is limited to
  synthetic LPCM streams.
- **Simplified parameter handling.** ReconGain parameter blocks are emitted as
  typed variants with no gain values; mix presentation parameters are decoded
  as MixGain only.
- **Seeking is minimal.** Only seek-to-frame-0 is supported.

## What works today

- OBU header, sequence header, codec config, audio element, mix presentation,
  parameter block, and temporal unit parsing.
- Bounded allocation: leb128-derived counts are capped by remaining payload
  bytes and `MAX_LEB128_CAPACITY` (64 MiB).
- LPCM substream decode (16/24/32-bit big-endian).
- Channel-based rendering with correct IAMF → SOTF channel permutation for
  mono, stereo, 5.1, 5.1.2, 5.1.4, 7.1, 7.1.2, 7.1.4, and binaural layouts.
- Scene-based (Ambisonics/HOA) rendering via `sotf-plugin-ambisonics`.
- Cross-reference validation: missing codec configs / audio elements are
  reported as `UnknownCodecConfig` / `UnknownAudioElement`.

## Path to release support

1. Implement Opus, AAC-LC, and FLAC substream decoding (engine/Symphonia
   integration).
2. Add a corpus of real IAMF test files to `data_tests/` covering each codec
   and each supported layout.
3. Implement spec-accurate ReconGain parsing using audio-element
   `recon_gain_is_present` flags.
4. Add seeking and full mix-presentation switching validation.
5. Pass a conformance test suite against the IAMF v1.1.0 specification.

Until these items are complete, the crate should be treated as a parser and
LPCM-only decoder for development and experimentation.
