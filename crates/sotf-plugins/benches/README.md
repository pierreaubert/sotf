# Audio Plugin Benchmarks

This directory contains performance benchmarks for audio plugins in the `sotf_audio` crate.

Currently covered:
- **Binaural decoder** (multichannel -> binaural) - `binaural-decoder-benchmark`
- **Upmixer** (stereo -> surround) - `upmixer-benchmark`
- **Compressor** (dynamic range compression) - `compressor-benchmark`
- **Gain / Host chain** (basic plugin and host performance) - `plugin-benchmark`
- **All other plugins** (EQ, delay, gate, limiter, expander, crossover, matrix, analyzers, loudness, channel mute/solo) - `all-plugins-benchmark`

## Running Benchmarks

```bash
# Run all plugin benchmarks
cargo bench -p plugins

# Run binaural decoder benchmarks
cargo bench --bench binaural-decoder-benchmark

# Run specific binaural benchmark group
cargo bench --bench binaural-decoder-benchmark -- binaural_process_channels

# Run upmixer benchmarks
cargo bench --bench upmixer-benchmark

# Run compressor benchmarks
cargo bench --bench compressor-benchmark

# Run comprehensive plugin benchmarks (EQ, delay, gate, limiter, etc.)
cargo bench --bench all-plugins-benchmark

# Run specific plugin group from comprehensive suite
cargo bench --bench all-plugins-benchmark -- EqPlugin
cargo bench --bench all-plugins-benchmark -- LimiterPlugin
cargo bench --bench all-plugins-benchmark -- CrossoverPlugin

# Run gain and host chain benchmarks
cargo bench --bench plugin-benchmark
```

## Binaural Decoder Benchmark Groups

### 1. `binaural_process_channels`
Tests processing performance across different channel configurations:
- 2ch (Stereo)
- 5ch (5.0 Surround)
- 6ch (5.1 Surround)
- 8ch (7.1 Surround)

**What it measures:** How well the decoder scales with increasing channel count.

**Expected results:** Linear scaling with number of input channels when optimization is enabled.

### 2. `binaural_fft_sizes`
Tests impact of FFT size on performance (512, 1024, 2048, 4096).

**What it measures:** Trade-off between quality (larger FFT) and performance.

**Expected results:** Roughly quadratic increase in time with FFT size (O(n log n) for FFT).

### 3. `binaural_optimization`
Compares optimized vs. standard processing paths.

**What it measures:** Benefit of sum-before-IFFT optimization.

**Expected results:** Optimized should be ~2-3x faster for multi-channel inputs.

### 4. `binaural_externalization`
Tests overhead of externalization effect at different levels (0.0, 0.5, 1.0).

**What it measures:** Cost of early reflection simulation.

**Expected results:** Small overhead (~5-10%) when externalization is enabled.

### 5. `binaural_large_blocks`
Stress test with large block sizes (512, 1024, 2048, 4096 frames).

**What it measures:** Throughput with realistic audio buffer sizes.

**Expected results:** Higher throughput (samples/sec) with larger blocks due to amortized overhead.

### 6. `binaural_passthrough`
Tests passthrough mode (no SOFA file loaded).

**What it measures:** Overhead of plugin when not actually processing.

**Expected results:** Near-zero overhead for passthrough.

### 7. `binaural_atmos_7_1_4`
Realistic workload for Dolby Atmos 7.1.4 with all features enabled.

**What it measures:** Real-world performance with 12-channel Atmos content.

**Expected results:** Target < 2ms processing time for 512-frame blocks @ 48kHz.

## Upmixer Benchmark Groups

### 1. `upmixer_5_1_block_sizes`
Tests processing performance for 5.1 configuration with different buffer sizes:
- 256 frames
- 512 frames
- 1024 frames
- 2048 frames

**What it measures:**
- Cost of the full upmix chain (FFT, ERB bands, decorrelation, VBAP, overlap-add)
  as a function of real-time buffer size.

### 2. `upmixer_configs`
Tests scaling with different speaker configurations at fixed block size (512 frames):
- `2.0` (stereo passthrough)
- `5.1` (6 channels)
- `7.1.4` (12 channels)
 - `9.1.6` (immersive layout)

**What it measures:**
- How processing cost scales with increasing number of output channels and VBAP targets.

### 3. `upmixer_fft_sizes`
Tests impact of FFT size on 5.1 upmixing performance with 512-frame buffers:
- 1024
- 2048
- 4096

**What it measures:**
- Trade-off between time/frequency resolution and CPU cost for the upmixer.

## All-Plugins Benchmark Groups

The `all-plugins-benchmark` suite covers plugins not in dedicated benchmark files:

### `EqPlugin`
- 1-band and 6-band stereo EQ
- 5.1 surround EQ
- Buffer size scaling (256-2048 frames)

### `DelayPlugin`
- Buffer size scaling (256-1024 frames)
- Feedback level impact (0%, 50%, 90%)

### `GatePlugin`
- Buffer size scaling (256-1024 frames)

### `LimiterPlugin`
- Hard vs soft limiting comparison
- Lookahead impact (0ms, 5ms, 10ms)

### `ExpanderPlugin`
- Buffer size scaling (256-1024 frames)

### `CrossoverPlugin`
- LR24 vs LR48 filter steepness
- Channel count scaling (2, 4, 8 channels)

### `MatrixPlugin`
- Identity (2x2), upmix (2->6), full routing (8x8)

### `Analyzers`
- Spectrum analyzer (30 bins)
- EBU R128 loudness monitor

### `Loudness`
- Fletcher-Munson compensation
- Multi-band loudness compensation

### `ChannelMuteSolo`
- Stereo and 8-channel configurations

## Interpreting Results

### Throughput
- Measured in elements/sec (input samples processed per second)
- Higher is better
- Should sustain at least 10x real-time for 48kHz audio

### Time
- Measured in µs (microseconds) or ms (milliseconds)
- Lower is better
- Target: < 500µs for 512 samples @ 48kHz (real-time factor > 20x)

### Performance Targets

For real-time audio at 48kHz:
- 512 samples = 10.67ms of audio
- Target processing time: < 2ms (5x real-time margin)
- Absolute maximum: < 10ms (1x real-time)

## Example Output

```
binaural_process_channels/2ch
                        time:   [245.32 µs 247.18 µs 249.21 µs]
                        thrpt:  [4.1047 Melem/s 4.1383 Melem/s 4.1697 Melem/s]

binaural_process_channels/6ch
                        time:   [682.45 µs 686.92 µs 691.78 µs]
                        thrpt:  [4.4315 Melem/s 4.4628 Melem/s 4.4920 Melem/s]
```

## Regression Testing

Criterion automatically detects performance regressions:
- Green: Performance improved
- Yellow: Performance similar (within noise)
- Red: Performance regressed

Results are saved to `target/criterion/` for comparison across runs.

## Profiling

For detailed profiling:

```bash
# Generate flamegraph
cargo flamegraph --bench binaural_decoder -- --bench

# Use perf (Linux)
cargo bench --bench binaural_decoder --profile-time=10

# Use Instruments (macOS)
cargo instruments -t "Time Profiler" --bench binaural_decoder
```

## CI Integration

Benchmarks can be run in CI for performance tracking:

```bash
# Run without statistical analysis (faster)
cargo bench --bench binaural_decoder -- --quick

# Compare against baseline
cargo bench --bench binaural_decoder -- --save-baseline main
```
