# Demo Audio Files

This directory contains demo audio files for testing the EQ in the AutoEQ UI
and other workspace integration tests.

## Files:

- `classical.wav` - Classical music sample
- `rock.wav` - Rock music sample
- `female_vocal.wav` - Female vocal sample
- `jazz.wav` - Jazz sample
- `piano.wav` - Piano sample
- `edm.wav` - EDM sample
- `country.wav` - Country music sample

Plus `.flac` counterparts for most files and a few alternate containers
(`.m4a`, `.mp4`, `.aac`) for format-coverage tests.

## Format Requirements:

- Format: WAV / FLAC / AAC-in-M4A
- Recommended: 44.1kHz or 48kHz sample rate
- Channels: Stereo (2 channels) or mono
- Duration: 30-60 seconds for demo purposes

## Adding Your Own Files:

Replace the placeholder files with your own demo audio. Make sure they are in
WAV or FLAC format and reasonably short to ensure fast loading in the web
interface and test suite. Update `manifest.toml` when adding or removing files.
