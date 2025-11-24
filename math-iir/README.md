<!-- markdownlint-disable-file MD013 -->

# AutoEQ IIR Filters

This crate provides IIR (Infinite Impulse Response) filter implementations for audio equalization.

## Features

- **Biquad Filters**: Implementation of common biquad filter types
  - Low-pass filters
  - High-pass filters
  - Peak/notch filters
  - Low/high shelf filters
  - Band-pass filters
- **PEQ (Parametric Equalizer)**: Multi-band parametric equalization with advanced features
  - SPL response computation
  - Preamp gain calculation
  - EqualizerAPO format export
  - PEQ comparison and manipulation
- **Filter Design**: Specialized filter design algorithms
  - Butterworth filters (lowpass/highpass)
  - Linkwitz-Riley filters (lowpass/highpass)
- **Response Computation**: Calculate frequency and phase response
- **Filter Conversion**: Convert between different filter representations

## Filter Types

### Biquad Filter Types

- `BiquadFilterType::Lowpass`: Low-pass filter
- `BiquadFilterType::Highpass`: High-pass filter
- `BiquadFilterType::HighpassVariableQ`: High-pass filter with variable Q
- `BiquadFilterType::Bandpass`: Band-pass filter
- `BiquadFilterType::Peak`: Peak/parametric filter
- `BiquadFilterType::Notch`: Notch filter
- `BiquadFilterType::Lowshelf`: Low-shelf filter
- `BiquadFilterType::Highshelf`: High-shelf filter

## Usage Examples

### Basic Biquad Filter

```rust
use autoeq_iir::{Biquad, BiquadFilterType};

// Create a peak filter at 1kHz with Q=1.0 and 3dB gain
let filter = Biquad::new(
    BiquadFilterType::Peak,
    1000.0, // frequency
    48000.0, // sample rate
    1.0,     // Q factor
    3.0      // gain in dB
);

// Apply filter to audio samples (requires mut for state updates)
// let mut filter = Biquad::new(...); // <- use mut if processing samples
// let output = filter.process(input_sample);

// Calculate frequency response at 1kHz
let response_db = filter.log_result(1000.0);
print!("Response at 1kHz: {:.2} dB", response_db);
```

### Parametric EQ (PEQ)

```rust
use autoeq_iir::{Biquad, BiquadFilterType, Peq, peq_spl, peq_preamp_gain, peq_format_apo};
use ndarray::Array1;

// Create a multi-band EQ
let mut peq: Peq = Vec::new();

// Add a high-pass filter at 80Hz
let hp = Biquad::new(BiquadFilterType::Highpass, 80.0, 48000.0, 0.707, 0.0);
peq.push((1.0, hp));

// Add a peak filter to boost mids at 1kHz
let peak = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.5, 4.0);
peq.push((1.0, peak));

// Add a high-shelf to roll off highs
let hs = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.8, -2.0);
peq.push((1.0, hs));

// Calculate frequency response
let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 1000);
let response = peq_spl(&freqs, &peq);

// Calculate preamp gain to prevent clipping
let preamp = peq_preamp_gain(&peq);
print!("Recommended preamp: {:.1} dB", preamp);

// Export to EqualizerAPO format
let apo_config = peq_format_apo("My Custom EQ", &peq);
print!("{}", apo_config);
```

### Filter Design

```rust
use autoeq_iir::{peq_butterworth_lowpass, peq_linkwitzriley_highpass};

// Create a 4th-order Butterworth lowpass at 2kHz
let lp_filter = peq_butterworth_lowpass(4, 2000.0, 48000.0);
print!("Butterworth LP has {} sections", lp_filter.len());

// Create a 4th-order Linkwitz-Riley highpass at 2kHz
let hp_filter = peq_linkwitzriley_highpass(4, 2000.0, 48000.0);
print!("LR HP has {} sections", hp_filter.len());

// These can be used for crossover design
```

## PEQ Functions Reference

### Core PEQ Operations

- `peq_spl(freq, peq)`: Calculate SPL response across frequencies
- `peq_equal(left, right)`: Compare two PEQs for equality
- `peq_preamp_gain(peq)`: Calculate recommended preamp gain
- `peq_preamp_gain_max(peq)`: Calculate conservative preamp gain with safety margin
- `peq_format_apo(comment, peq)`: Export PEQ to EqualizerAPO format

### Filter Design Functions

- `peq_butterworth_q(order)`: Calculate Q values for Butterworth filters
- `peq_butterworth_lowpass(order, freq, srate)`: Create Butterworth lowpass filter
- `peq_butterworth_highpass(order, freq, srate)`: Create Butterworth highpass filter
- `peq_linkwitzriley_q(order)`: Calculate Q values for Linkwitz-Riley filters
- `peq_linkwitzriley_lowpass(order, freq, srate)`: Create Linkwitz-Riley lowpass filter
- `peq_linkwitzriley_highpass(order, freq, srate)`: Create Linkwitz-Riley highpass filter

### Utility Functions

- `bw2q(bw)`: Convert bandwidth in octaves to Q factor
- `q2bw(q)`: Convert Q factor to bandwidth in octaves

## Advanced Example: Building a Complete Audio Processor

```rust
use autoeq_iir::*;
use ndarray::Array1;

fn create_studio_eq() -> Peq {
    let mut peq = Vec::new();

    // High-pass filter to remove subsonic content
    let hp = peq_butterworth_highpass(2, 20.0, 48000.0);
    peq.extend(hp);

    // Presence boost
    let presence = Biquad::new(BiquadFilterType::Peak, 3000.0, 48000.0, 1.2, 2.5);
    peq.push((1.0, presence));

    // Air band enhancement
    let air = Biquad::new(BiquadFilterType::Highshelf, 10000.0, 48000.0, 0.9, 1.5);
    peq.push((1.0, air));

    peq
}

fn analyze_eq(peq: &Peq) {
    // Generate frequency sweep
    let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 200);

    // Calculate response
    let response = peq_spl(&freqs, peq);

    // Find peak response
    let max_gain = response.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let min_gain = response.iter().fold(f64::INFINITY, |a, &b| a.min(b));

    println!("EQ Analysis:");
    println!("  Peak gain: {:.2} dB", max_gain);
    println!("  Min gain: {:.2} dB", min_gain);
    println!("  Dynamic range: {:.2} dB", max_gain - min_gain);
    println!("  Recommended preamp: {:.2} dB", peq_preamp_gain(peq));
}

fn main() {
    let studio_eq = create_studio_eq();
    analyze_eq(&studio_eq);

    // Export for use in EqualizerAPO
    let config = peq_format_apo("Studio EQ v1.0", &studio_eq);
    println!("\nEqualizerAPO Configuration:");
    println!("{}", config);
}
```

## Key Concepts

### PEQ Type

The `Peq` type is defined as `Vec<(f64, Biquad)>` where:

- The `f64` is the weight/amplitude multiplier for each filter
- The `Biquad` is the individual filter definition
- This allows for flexible filter chaining and weighting

### Filter Order vs. Sections

- **Butterworth filters**: An Nth-order filter uses N/2 biquad sections (rounded up)
- **Linkwitz-Riley filters**: Special case of Butterworth designed for crossovers
- Higher orders provide steeper rolloff but more computational cost

### Q Factor Guidelines

- **Q < 0.5**: Wide, gentle curves
- **Q = 0.707**: Butterworth response (maximally flat)
- **Q = 1.0**: Good compromise for most applications
- **Q > 5**: Very narrow, surgical corrections
- **Q > 10**: Extreme precision, potential ringing

# Fast PEQ Loudness Compensation

## Overview

When applying parametric EQ (PEQ) to audio, the spectral balance and perceived loudness changes. This document describes how to use the fast loudness compensation feature to maintain similar loudness without running full Replay Gain analysis.

## The Problem

Traditional approach (slow):
1. Apply PEQ filters to audio
2. Run full Replay Gain analysis (EBU R128)
3. Decode entire audio file
4. Process all samples through loudness meter
5. Takes seconds for each file

## The Solution

Fast analytical approach (microseconds):
1. Analyze PEQ frequency response
2. Apply perceptual weighting (K-weighting or A-weighting)
3. Compute loudness change analytically
4. Takes microseconds, no audio processing needed

## API Usage

### Basic Example

```rust
use autoeq_iir::{Biquad, BiquadFilterType, Peq, peq_loudness_gain, peq_preamp_gain};

// Create your PEQ filters
let bass_boost = Biquad::new(BiquadFilterType::Peak, 100.0, 48000.0, 1.0, 6.0);
let peq: Peq = vec![(1.0, bass_boost)];

// Get gain adjustments
let anti_clip = peq_preamp_gain(&peq);          // Prevents clipping: -6.00 dB
let loudness_k = peq_loudness_gain(&peq, "k");  // K-weighted: -1.40 dB
let loudness_a = peq_loudness_gain(&peq, "a");  // A-weighted: -0.06 dB

// Apply to your audio pipeline:
// total_gain = anti_clip + loudness_k  // or use loudness_a
```

### Choosing Weighting Method

**K-weighting** (`"k"`):
- Based on EBU R128 standard
- Similar to how Replay Gain works
- Better for general music playback
- Recommended for most use cases

**A-weighting** (`"a"`):
- Classic loudness measurement
- Emphasizes mid-range, de-emphasizes bass
- Better for voice/speech content
- More tolerant of bass boost

### Real-World Examples

From the test output:

1. **Mid-range boost** (+6 dB at 1 kHz):
   - K-weighted: -1.55 dB (compensate by reducing 1.55 dB)
   - A-weighted: -1.36 dB
   - Effect: Similar compensation since 1kHz is important in both curves

2. **Bass boost** (+6 dB at 100 Hz):
   - K-weighted: -1.40 dB
   - A-weighted: -0.06 dB
   - Effect: A-weighting barely compensates (bass less important perceptually)

3. **Treble boost** (+6 dB at 8 kHz):
   - K-weighted: -2.40 dB
   - A-weighted: -0.99 dB
   - Effect: K-weighting has high-frequency boost, so more compensation needed

4. **V-shaped EQ** (bass+4dB, mid-3dB, treble+3dB):
   - K-weighted: -1.76 dB
   - A-weighted: +0.14 dB (slight boost!)
   - Effect: A-weighting sees net reduction in important frequencies

## Integration with Audio Pipeline

### Option 1: Combine with Anti-Clipping

```rust,ignore
// Safest: prevent clipping AND maintain loudness
let total_gain = peq_preamp_gain(&peq) + peq_loudness_gain(&peq, "k");

// Apply PEQ filters + total_gain to audio
```

### Option 2: Loudness Compensation Only

```rust,ignore
// If you know your PEQ won't clip (e.g., cuts only)
let total_gain = peq_loudness_gain(&peq, "k");

// Apply PEQ filters + total_gain to audio
```

### Option 3: Use with CamillaDSP

```yaml
# In your CamillaDSP config:
filters:
  peq_with_compensation:
    type: Conv
    parameters:
      # ... your PEQ biquads ...
      - type: Gain
        parameters:
          gain: -1.55  # from peq_loudness_gain()
```

## Performance Comparison

| Method | Time per file | Requires audio? | Accuracy |
|--------|---------------|-----------------|----------|
| Replay Gain (EBU R128) | 1-10 seconds | Yes | 100% (reference) |
| peq_loudness_gain() | < 1 millisecond | No | ~90-95% |

The analytical method is **1000x faster** while providing good perceptual accuracy.

## When to Use Each Method

### Use `peq_loudness_gain()` when:
- Real-time EQ adjustment
- Previewing EQ changes
- Batch processing many files
- Interactive audio applications
- You want instant results

### Use full Replay Gain when:
- Normalizing existing audio files
- Maximum accuracy required
- One-time analysis acceptable
- Working with non-EQ'd audio

## Technical Details

### How It Works

1. Generate 500 logarithmically-spaced frequency points (20 Hz - 20 kHz)
2. Compute PEQ frequency response at each point
3. Apply perceptual weighting curve (K or A)
4. Integrate weighted energy change
5. Convert to dB gain compensation

### Weighting Curves

**K-weighting** approximation:
- High-pass: 4th order Butterworth at 38 Hz
- High-shelf: +4 dB above 1500 Hz

**A-weighting** (IEC 61672-1):
- Follows standard A-weighting formula
- Peak sensitivity ~4 kHz
- -19 dB at 100 Hz, -9 dB at 500 Hz

## Limitations

1. **Assumes broadband audio**: Works best with music/speech, not pure tones
2. **Ignores phase**: Only analyzes magnitude response
3. **Approximation**: Not identical to sample-by-sample EBU R128
4. **No masking effects**: Doesn't model psychoacoustic masking

Despite these limitations, the method provides good perceptual accuracy for practical use.

## Future Improvements

Potential enhancements:
- [ ] More accurate K-weighting filter implementation
- [ ] Support for custom weighting curves
- [ ] Integration with audio file metadata
- [ ] Automatic weighting selection based on content type

## See Also

- `src-iir/src/mod.rs`: Implementation
- `src-audio/src/replaygain.rs`: Full Replay Gain implementation
- EBU R128 standard: https://tech.ebu.ch/docs/r/r128.pdf
- A-weighting standard: IEC 61672-1
## License

GPL-3.0-or-later
