# Regression Tests for Audio Recording

This directory contains regression tests that use hardware loopback to verify the audio recording and playback functionality.

## Test Coverage

The test suite verifies:

1. **Playback Functionality**: Playback works correctly with valid configuration
2. **Signal Generation**: Recording can generate each kind of test signal (tone, sweep, pink noise)
3. **Channel Mapping**: Recording supports channel mapping for both input and output
4. **Loopback Accuracy**: Signals sent through loopback are read back accurately with:
   - SPL measurements close to 0 dB (within 0.5 dB tolerance)
   - Low SPL variation (< 2 dB)
   - Proper phase wrapping to [-180°, 180°]
   - CSV output with 200 log-spaced frequency points
5. **No WAV Corruption**: WAV files don't have duplicate headers (regression test for FLOAT32LE bug)

## Requirements

- Audio interface with loopback capability (physical or virtual)
- CamillaDSP binary in PATH
- Connected loopback between output and input channels

## Setup

### 1. Hardware Loopback

Connect the output channel to the input channel on your audio interface. For example:
- Connect output channel 1 to input channel 1 using a cable
- Or configure virtual loopback in your audio interface software

### 2. Environment Variables

Set the following environment variables before running tests:

```bash
export AEQ_E2E=1                    # Enable E2E tests
export AEQ_E2E_SEND_CH=1            # Hardware channel to send to
export AEQ_E2E_RECORD_CH=1          # Hardware channel to record from
export AEQ_E2E_SR=48000             # Sample rate (optional, default: 48000)
```

## Running Tests

### Run all regression tests:

```bash
cd src-audio
AEQ_E2E=1 AEQ_E2E_SEND_CH=1 AEQ_E2E_RECORD_CH=1 cargo test --test regression_loopback
```

### Run a specific test:

```bash
# Test playback
AEQ_E2E=1 AEQ_E2E_SEND_CH=1 AEQ_E2E_RECORD_CH=1 \
  cargo test --test regression_loopback test_playback_valid_config

# Test signal types
AEQ_E2E=1 AEQ_E2E_SEND_CH=1 AEQ_E2E_RECORD_CH=1 \
  cargo test --test regression_loopback test_recording_all_signal_types

# Test channel mapping
AEQ_E2E=1 AEQ_E2E_SEND_CH=1 AEQ_E2E_RECORD_CH=1 \
  cargo test --test regression_loopback test_channel_mapping

# Test loopback accuracy
AEQ_E2E=1 AEQ_E2E_SEND_CH=1 AEQ_E2E_RECORD_CH=1 \
  cargo test --test regression_loopback test_loopback_accuracy
```

### Run with output:

```bash
AEQ_E2E=1 AEQ_E2E_SEND_CH=1 AEQ_E2E_RECORD_CH=1 \
  cargo test --test regression_loopback -- --nocapture
```

## Test Output

Tests will create WAV and CSV files in `src-audio/target/regression-tests/`:
- `test_playback_tone.wav` - Playback test file
- `test_signal_*.wav` - Test signals for each type
- `test_record_*.wav` - Recorded signals
- `test_loopback_*.wav` - Loopback test files
- `test_loopback_analysis.csv` - Frequency response analysis

## Expected Results

When all tests pass, you should see output like:

```
=== Test 1: Playback with Valid Config ===
Sample rate: 48000 Hz
Send channel: 1
✓ Playback started successfully
✓ Playback stopped successfully
✓ Test 1 passed: Playback works with valid config

=== Test 2: Recording Each Signal Type ===
Testing signal type: tone
  ✓ tone recorded successfully (85.3% non-zero)
Testing signal type: sweep
  ✓ sweep recorded successfully (91.2% non-zero)
Testing signal type: pink_noise
  ✓ pink_noise recorded successfully (99.8% non-zero)
✓ Test 2 passed: All signal types record successfully

=== Test 3: Channel Mapping ===
Output mapping: send to hardware channel 1
Input mapping: record from hardware channel 1
✓ Recording started with input channel map: [1]
✓ Playback started with output channel map: [1]
✓ Test 3 passed: Channel mapping works correctly

=== Test 4: Loopback Accuracy ===
Playing and recording 5-second sweep...
Analyzing loopback recording...
Analysis complete:
  Estimated lag: 42358 samples (882.46 ms)
  CSV format: 201 lines
  SPL (100Hz-10kHz):
    Mean:  0.001 dB
    Min:   -0.042 dB
    Max:   0.038 dB
    Range: 0.080 dB
  ✓ Mean SPL is close to 0 dB (within 0.5 dB)
  ✓ SPL variation is low (< 2 dB)
  ✓ Phase values properly wrapped to [-180, 180]°
  ✓ Recording differs from reference (0.0% identical samples)
✓ Test 4 passed: Loopback sends and reads back signal accurately
```

## Troubleshooting

### Tests are skipped

If you see "Skipping test (AEQ_E2E!=1)", make sure to set `AEQ_E2E=1`.

### Playback/Recording fails

- Verify CamillaDSP is installed and in PATH: `which camilladsp`
- Check audio interface is connected and recognized
- Verify channel numbers are correct for your hardware

### SPL not close to 0 dB

- Check loopback cable connections
- Verify volume/gain settings on audio interface
- Ensure no processing (EQ, compression) is applied to loopback

### High SPL variation

- May indicate signal alignment issues
- Check for dropouts or glitches in audio interface
- Try increasing recording padding duration

## Continuous Integration

These tests can be integrated into CI/CD if you have:
- CI runners with audio hardware
- Virtual audio devices with loopback support
- Proper environment configuration

Example GitHub Actions workflow:

```yaml
name: Audio Regression Tests

on: [push, pull_request]

jobs:
  audio-tests:
    runs-on: self-hosted  # Requires self-hosted runner with audio hardware
    steps:
      - uses: actions/checkout@v3
      - name: Install CamillaDSP
        run: cargo install camilladsp
      - name: Run regression tests
        env:
          AEQ_E2E: 1
          AEQ_E2E_SEND_CH: 1
          AEQ_E2E_RECORD_CH: 1
        run: |
          cd src-audio
          cargo test --test regression_loopback
```

## Adding New Tests

To add a new regression test:

1. Add a new `#[tokio::test]` function in `regression_loopback.rs`
2. Use `should_run_e2e_tests()` to gate the test
3. Use `get_test_config()` to get channel/SR configuration
4. Follow the pattern of existing tests for playback/recording
5. Add clear assertions with helpful error messages
6. Document the test in this README

## Regression History

### Issues Caught by These Tests

1. **WAV File Corruption (FLOAT32LE)**: Duplicate RIFF headers when using FLOAT32LE format with CamillaDSP 3.0.1
2. **Phase Wrapping Bug**: Phase values accumulating beyond ±180° without proper wrapping
3. **Signal Alignment**: SPL measurements incorrect due to missing time-alignment of signals before FFT
4. **RF64 Placeholders**: Invalid 0xFFFFFFFF size fields in WAV files rejected by hound library
5. **File Copy Bug**: Reference WAV being copied instead of actual recording

These tests ensure these bugs don't reoccur in future changes.
