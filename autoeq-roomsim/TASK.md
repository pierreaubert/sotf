# Room Acoustics Simulator - Improvement Tasks

## Phase 1: Quick Wins

### 1. Frequency-dependent wall absorption
- [x] Add absorption coefficient configuration per wall
- [x] Add frequency bands for absorption (125, 250, 500, 1000, 2000, 4000 Hz)
- [x] Convert absorption to reflection coefficient: R = sqrt(1 - α)
- [x] Add wall material presets (concrete, drywall, glass, carpet, etc.)
- [x] Add `WallMaterial` struct with preset methods
- [x] Add `WallMaterialConfig` for JSON serialization
- [x] Add `WallMaterialsConfig` for all 6 surfaces
- [x] Update ISM to use per-wall, frequency-dependent reflections
- [x] Add `get_material_presets()` WASM function

### 2. Air absorption
- [x] Add air absorption coefficient calculation based on temperature/humidity
- [x] Apply exponential decay: exp(-m * r) where m is absorption coefficient
- [x] Typical values: ~0.001/m at 1kHz, ~0.01/m at 8kHz
- [x] Add temperature/humidity to SolverConfig
- [x] Add `air_absorption_factor()` method to RoomSimulatorWasm
- [x] Apply to all sound paths (direct, 1st, 2nd, 3rd order reflections)

### 3. Source delay/phase alignment
- [x] Add time delay parameter to source configuration (`delay_ms`)
- [x] Add phase inversion option (`invert_phase`)
- [x] Apply phase shift: exp(-i * omega * delay)
- [x] Apply to all sound paths (direct, 1st, 2nd, 3rd order reflections)
- [x] Useful for multi-driver speaker alignment

### 4. Wall material presets
- [x] Create preset library with common materials (completed in Task 1)
- [x] Include frequency-dependent absorption data (completed in Task 1)
- [ ] Add UI selector for materials (frontend task)

## Phase 2: Accuracy Improvements

### 5. Room mode calculation
- [x] Calculate axial modes: f = c/(2L) * n
- [x] Calculate tangential modes: f = c/2 * sqrt((n/Lx)² + (m/Ly)²)
- [x] Calculate oblique modes: f = c/2 * sqrt((n/Lx)² + (m/Ly)² + (p/Lz)²)
- [x] Add `RoomMode` struct with frequency, indices, type, and description
- [x] Add `calculate_room_modes()` function
- [x] Add `get_room_modes()` WASM function for standalone mode calculation
- [x] Add `get_schroeder_frequency()` WASM function
- [x] Include room_modes in simulation results (for rectangular rooms)
- [ ] Display mode frequencies on frequency response plot (frontend task)

### 6. RT60 estimation
- [x] Implement Sabine formula: RT60 = 0.161 * V / A
- [x] Implement Eyring formula for higher absorption
- [x] Add `RoomAcoustics` struct with RT60, volume, surface area, absorption, Schroeder freq, critical distance
- [x] Add `rt60_sabine()`, `rt60_eyring()`, `critical_distance()` functions
- [x] Add `calculate_room_acoustics()` function
- [x] Add `get_rt60()` WASM function for standalone calculation
- [x] Include room_acoustics in simulation results
- [ ] Display Schroeder frequency warning in UI (frontend task)

### 7. Fix L-shaped room ISM
- [x] Add `contains_xy()` and `contains()` methods to `LShapedRoom`
- [x] Add `is_valid_image_source()` to validate reflection paths
- [x] Add `get_first_order_images()` for proper L-shaped ISM
- [x] Handle variable right wall position (width1 vs width2) based on source location
- [x] Add interior step wall reflections (horizontal step)
- [x] Validate reflection paths don't cross forbidden corner region
- [x] Limit L-shaped rooms to 1st-order reflections (higher orders need complex validation)
- [ ] Add full higher-order ISM validation for L-shaped rooms (complex)

### 8. Edge diffraction (basic)
- [x] Implement simplified UTD/Biot-Tolstoy-Medwin diffraction model
- [x] Add `edge_diffraction_coefficient()` function with wedge angle support
- [x] Add `DiffractionEdge` struct with closest point and contribution methods
- [x] Add `get_rectangular_room_edges()` to generate 12 room corner edges
- [x] Add `edge_diffraction` config option (disabled by default)
- [x] Pre-compute diffraction edges in `RoomSimulatorWasm` constructor
- [x] Add diffraction contributions to `calculate_direct_field()`
- [ ] Implement full BTM edge integral for higher accuracy (complex)

## Phase 3: Advanced Features

### 9. Hybrid solver
- [x] Add modal analysis function for room mode superposition
- [x] Calculate modal pressure from standing wave modes with damping
- [x] Use modal analysis below Schroeder frequency
- [x] Use ISM above Schroeder frequency
- [x] Smooth cosine crossover transition (configurable width in octaves)
- [x] Add `hybrid_crossover_width`, `max_mode_order`, `modal_damping` to SolverConfig
- [x] Support `method: "modal"` for pure modal analysis
- [x] Support `method: "hybrid"` for blended modal/ISM

### 10. Impulse response output
- [x] Add `ImpulseResponse` struct with time, amplitude, sample_rate, duration, energy_decay
- [x] Add `ImpulseResponseConfig` for customization (sample_rate, duration, fft_size)
- [x] Implement `calculate_impulse_response()` with IFFT
- [x] Implement Cooley-Tukey radix-2 IFFT algorithm
- [x] Add `interpolate_complex()` for log-frequency interpolation with phase unwrapping
- [x] Calculate energy decay curve (Schroeder integration)
- [x] Add `generate_impulse_response` flag to VisualizationConfig
- [x] Store complex pressures during simulation when IR generation enabled
- [x] Add `compute_impulse_response()` WASM function for standalone IR computation
- [x] Include impulse_response in SimulationResults

### 11. Binaural rendering
- [x] Add `BinauralResponse` struct with left/right impulse responses, ITD, ILD
- [x] Add `BinauralConfig` with head position, yaw, radius, ear spacing
- [x] Implement `calculate_ear_positions()` from head center and orientation
- [x] Implement `calculate_itd()` using Woodworth's formula
- [x] Implement `approximate_hrtf_magnitude()` for frequency-dependent ILD
- [x] Implement `calculate_binaural_response()` for stereo IR generation
- [x] Add `binaural` config to `VisualizationConfig`
- [x] Compute left/right ear pressures during simulation
- [x] Include `binaural_response` in `SimulationResults`

### 12. Real speaker data import
- [ ] Parse CLF text format (CF1/CF2 binary formats are not publicly documented)
- [ ] ~~Parse EASE GLL files~~ (proprietary binary format, no public specification)
- [x] Support spinorama.org directivity data (JSON format, open)
  - [x] Add `DirectivityData` and `DirectivityCurve` structs to autoeq-cea2034
  - [x] Add `fetch_directivity_data()` to fetch SPL Horizontal/Vertical from API
  - [x] Update download binary to cache directivity measurements
  - [x] Add `SpinoramaCurve` struct and `Spinorama` directivity variant to WASM simulator
  - [x] Add `create_spinorama_pattern()` to convert spinorama data to DirectivityPattern
- [ ] Support generic polar/balloon data in CSV format
- [ ] Import from measurement software (REW, ARTA) export formats

---

## Progress Log

### Current Task: Phase 3 Complete + Task 12 In Progress
Status: Phase 3 Complete, Task 12 partially complete

### Completed:
- Task 1: Frequency-dependent wall absorption ✓
- Task 2: Air absorption ✓
- Task 3: Source delay/phase alignment ✓
- Task 4: Wall material presets ✓ (completed as part of Task 1)
- Task 5: Room mode calculation ✓
- Task 6: RT60 estimation ✓
- Task 7: Fix L-shaped room ISM ✓ (1st order only)
- Task 8: Edge diffraction ✓ (simplified BTM model)
- Task 9: Hybrid solver ✓ (modal + ISM with Schroeder crossover)
- Task 10: Impulse response output ✓ (IFFT with energy decay curve)
- Task 11: Binaural rendering ✓ (stereo IR with ITD/ILD)
- Task 12: spinorama.org directivity data ✓ (backend support in autoeq crate)
