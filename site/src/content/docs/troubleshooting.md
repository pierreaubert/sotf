---
title: Troubleshooting
description: Common issues and their solutions.
---

## Audio Issues

### No sound output

1. Check that the correct output device is selected (Devices screen or `--device` CLI flag).
2. Verify volume is not muted (`m` to toggle mute).
3. Check that no plugin in the chain has zeroed gain.

### Crackling or dropouts

1. Increase the buffer size in engine configuration (`buffer_ms`).
2. Reduce the number of active plugins — especially convolution and FFT-based plugins.
3. Close CPU-intensive background applications.
4. On macOS, ensure the sample rate matches between SotF and your audio device.

### Audio sounds distorted

1. Check the limiter plugin — if absent, peaks above 0 dBFS will clip.
2. Review EQ filters for excessive boost (> 6 dB).
3. Check the gain plugin for values above 0 dB without a downstream limiter.

## Library Issues

### Albums not appearing after scan

1. Ensure the directory is added in Configure > Directories.
2. Check that audio files have proper metadata tags (artist, album, title).
3. Supported formats: FLAC, MP3, AAC, ALAC, OGG, WAV, M4A.
4. Re-trigger a scan from the Directories screen.

### Duplicate albums

1. SotF deduplicates by album title + artist. Check for metadata inconsistencies.
2. Remove and re-add the directory to force a clean rescan.

## Plugin Issues

### Plugin shows "channel conflict"

This happens when a plugin expects a different channel count than the current signal.
For example, the Upmixer requires stereo (2ch) input — it won't work with mono or multichannel sources.

**Solutions:**
- Let SotF auto-suspend the conflicting plugin (default behavior).
- Reorder plugins so channel counts match at each stage.
- Use the Matrix plugin to convert channels before the conflicting plugin.

### EQ filters have no effect

1. Verify the EQ plugin is not bypassed/suspended.
2. Check that filter gain is non-zero.
3. Ensure the filter frequency is within the audible range (20 Hz - 20 kHz).

## macOS System Audio

### HAL driver not appearing in Sound settings

1. Restart CoreAudio: `sudo killall coreaudiod`
2. Verify the driver is installed: `ls ~/Library/Audio/Plug-Ins/HAL/`
3. Check Console.app for driver loading errors.

### Daemon won't start

1. Check if another instance is running: `ps aux | grep sotf_daemon`
2. Remove stale lock file if the daemon crashed: check the socket path.
3. Verify permissions on the socket directory.
