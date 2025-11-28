# Room Acoustics BEM Simulation Configurations

This directory contains pre-configured room models for boundary element method (BEM) acoustic simulations.

## Available Configurations

### 1. Near-Field Stereo (`nearfield_stereo.json`)

**Scenario**: Desktop/studio monitoring setup with near-field speakers

**Room Dimensions**:
- Width: 3.5m
- Depth: 3.0m  
- Height: 2.4m
- Volume: 25.2 m³

**Speaker Setup**:
- **Left Speaker**: (0.9m, 0.5m, 1.0m) - Full range
- **Right Speaker**: (2.6m, 0.5m, 1.0m) - Full range
- **Speaker spacing**: 1.7m
- **Listening distance**: 1.5m from front wall
- **Listening position**: (1.75m, 2.0m, 1.0m)

**Frequency Range**: 40-500 Hz (30 logarithmic points)

**Simulation Results**:
- Adaptive mesh: 889 nodes, 1502 triangular elements
- Optimized for near-field monitoring accuracy
- Captures early reflections and room modes

**Use Cases**:
- Studio monitor placement optimization
- Desktop audio setup analysis
- Critical listening environment design

---

### 2. 5.1.4 Dolby Atmos Home Theater (`home_theater_5_1_4.json`)

**Scenario**: Dedicated home theater with immersive audio

**Room Dimensions**:
- Width: 5.5m
- Depth: 7.0m
- Height: 2.6m
- Volume: 100.1 m³

**Speaker Setup** (10 channels total):

**Bed Layer** (5.1):
- **Front Left**: (1.2m, 0.3m, 1.1m) - Highpass 80Hz, LR4
- **Center**: (2.75m, 0.3m, 0.9m) - Highpass 80Hz, LR4
- **Front Right**: (4.3m, 0.3m, 1.1m) - Highpass 80Hz, LR4
- **Surround Left**: (0.8m, 4.5m, 1.5m) - Highpass 80Hz, LR4
- **Surround Right**: (4.7m, 4.5m, 1.5m) - Highpass 80Hz, LR4
- **Subwoofer**: (0.5m, 0.5m, 0.3m) - Lowpass 80Hz, LR4

**Height Layer** (.4):
- **Height Front Left**: (1.5m, 1.0m, 2.4m) - Highpass 100Hz, LR4
- **Height Front Right**: (4.0m, 1.0m, 2.4m) - Highpass 100Hz, LR4
- **Height Rear Left**: (1.5m, 5.5m, 2.4m) - Highpass 100Hz, LR4
- **Height Rear Right**: (4.0m, 5.5m, 2.4m) - Highpass 100Hz, LR4

**Listening Position**: (2.75m, 4.0m, 1.2m)
- Distance from screen: 3.7m
- Centered laterally

**Frequency Range**: 20-300 Hz (40 logarithmic points)

**Simulation Results**:
- Adaptive mesh: 2686 nodes, 4864 triangular elements
- Full bass management modeling with 80Hz/100Hz crossovers
- Height channel integration analysis
- Room mode identification across full spectrum

**Use Cases**:
- Dolby Atmos speaker placement
- Subwoofer position optimization  
- Bass management tuning
- Room treatment planning

---

## Running Simulations

```bash
# Near-field stereo
/Users/pierre/src/sotf/target/release/room-simulator-bem \
  --config configs/nearfield_stereo.json \
  --output output_nearfield.json \
  --verbose

# 5.1.4 Home theater
/Users/pierre/src/sotf/target/release/room-simulator-bem \
  --config configs/home_theater_5_1_4.json \
  --output output_home_theater.json \
  --verbose
```

## Configuration Features

All configurations include:

- **Adaptive mesh refinement**: λ/8 element sizing with 2× refinement near sources
- **Adaptive integration**: Frequency-dependent quadrature for accuracy
- **ILU preconditioning**: Fast GMRES convergence
- **Butterworth crossovers**: Linkwitz-Riley 4th order (24dB/oct)
- **Source proximity refinement**: Finer mesh near speakers
- **Corner grading**: Enhanced resolution at geometric features

## Output Format

Simulations generate JSON output containing:
- Frequency response at listening position (SPL vs frequency)
- Room geometry (edges for visualization)
- Source positions and configurations
- Mesh statistics
- Solver metadata

## Customization

To create your own configurations, copy an existing file and modify:

1. **Room dimensions**: `room.width`, `room.depth`, `room.height`
2. **Speaker positions**: Add/modify entries in `sources[]`
3. **Crossovers**: Adjust `cutoff_freq` and `order` for bass management
4. **Frequency range**: Set `min_freq`, `max_freq`, `num_points`
5. **Solver settings**: Tune `mesh_resolution`, GMRES parameters

## Technical Notes

### Crossover Types
- `fullrange`: No filtering (monitors, full-range speakers)
- `lowpass`: Subwoofers (typically 80Hz LR4)
- `highpass`: Satellites/mains (typically 80-100Hz LR4)
- `bandpass`: Midrange drivers (specify both cutoffs)

### Mesh Resolution
- `mesh_resolution: 1`: Coarse (fast, low accuracy)
- `mesh_resolution: 2`: Medium (good balance) **← Recommended**
- `mesh_resolution: 3`: Fine (slow, high accuracy)

### Adaptive Meshing
- When `adaptive_meshing: true`, resolution scales with frequency
- Surfaces near sources get 2× refinement automatically
- Corners get quadratic grading for singularity capture
- Typical mesh: 500-5000 elements depending on room size and frequency

---

**Created**: 2025-01-28  
**Software**: SOTF BEM Room Acoustics Simulator  
**Version**: 0.1.1
