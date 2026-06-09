# Symphonia Add-ons

Local Symphonia-compatible format and codec crates used by SOTF.

These crates live together because they extend Symphonia in areas that are not
covered by the upstream default crate set, or that need local integration before
they are suitable for upstreaming.

## Crates

- `symphonia-format-sacd` -- SACD ISO container reader and extractor. It exposes
  a Symphonia `FormatReader` for stereo and multichannel SACD areas and an
  `extract_sacd` example for DSF/WAV extraction.
- `symphonia-codec-dsd` -- Symphonia audio decoder for DSD packets, converting
  packed DSD to PCM through `rdsd2pcm`.
- `symphonia-codec-dst` -- Symphonia audio decoder for SACD DST packets. It
  expands DST to DSD with `dst-decoder`, then converts DSD to PCM with
  `rdsd2pcm`.
- `symphonia-codec-wavpack` -- Native WavPack reader and decoder following
  Symphonia conventions.
- `dst-decoder` -- Rust DST frame decoder used by the SACD DST codec bridge.
- `rdsd2pcm` -- Rust DSD-to-PCM conversion library used by the DSD and DST
  Symphonia decoders and extraction examples.

## Integration

Consumers should register only the pieces they need:

- SACD format support: `symphonia_format_sacd::register_all(...)`
- DSD decode support: `symphonia_codec_dsd::register_decoders(...)`
- DST decode support: `symphonia_codec_dst::register_decoders(...)`
- WavPack support: register `symphonia_codec_wavpack::WavPackReader` and
  `symphonia_codec_wavpack::WavPackDecoder` with the local Symphonia probe and
  codec registry.

The add-ons are container/codec integration crates. They should keep behavior
close to Symphonia's `FormatReader` and `AudioDecoder` expectations and avoid
SOTF player UI or library-scan policy.

## Verification

Useful focused checks:

```bash
cargo test -p symphonia-format-sacd
cargo test -p symphonia-codec-dsd
cargo test -p symphonia-codec-dst
cargo test -p symphonia-codec-wavpack
cargo test -p dst-decoder
cargo test -p rdsd2pcm
```
