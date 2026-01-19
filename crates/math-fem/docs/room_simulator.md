# Room Acoustics FEM Simulator

The `roomsim-fem` tool simulates low-frequency room acoustics using the Finite Element Method (FEM). It solves the Helmholtz equation in the frequency domain to predict the sound pressure field and frequency response at listening positions.

## Physics

The simulator solves the time-harmonic Helmholtz equation:

$$ \nabla^2 p + k^2 p = 0 \quad \text{in } \Omega $$

where $p$ is the sound pressure, $k = \omega/c$ is the wavenumber, and $\Omega$ is the room volume.

### Boundary Conditions

The simulator supports Robin boundary conditions to model wall impedance:

$$ \frac{\partial p}{\partial n} + i k \frac{\rho c}{Z} p = 0 \quad \text{on } \Gamma $$

where $Z$ is the specific acoustic impedance of the wall surface. This allows modeling:
*   **Rigid Walls:** $Z \to \infty$ (equivalent to $\partial p/\partial n = 0$).
*   **Absorptive Walls:** Using an energy absorption coefficient $\alpha$. The simulator maps $\alpha$ to a locally reacting impedance $Z$ using the relation for normal incidence.

$$ R = \sqrt{1-\alpha}, \quad Z = \rho c \frac{1+R}{1-R} $$

## Usage

### Command Line

```bash
# Run with a configuration file
cargo run --release --bin roomsim-fem --features "cli native" -- --config room.json

# View help and options
cargo run --release --bin roomsim-fem --features "cli native" -- --help
```

### Options

*   `--config <FILE>`: Path to JSON configuration.
*   `--output <FILE>`: Path to output JSON (default: `output_fem.json`).
*   `--threads <N>`: Number of threads (default: all cores).
*   `--batch-size <N>`: Number of frequencies to solve concurrently. Set to 0 for auto-tuning based on memory.
*   `--memory-gb <GB>`: Override system memory detection for batch size planning.
*   `--warm-start`: Enable hierarchical warm starting (solve anchors, then interpolate guesses).

## Configuration

The input file is a JSON object. See `room_config_schema.json` for the formal schema.

### Example

```json
{
  "room": {
    "type": "rectangular",
    "width": 5.0,
    "depth": 4.0,
    "height": 2.8
  },
  "boundaries": {
    "floor": { "type": "rigid" },
    "ceiling": { "type": "absorption", "coefficient": 0.3 },
    "walls": { "type": "absorption", "coefficient": 0.1 },
    "back_wall": { "type": "absorption", "coefficient": 0.5 }
  },
  "sources": [
    {
      "name": "Subwoofer",
      "position": { "x": 2.5, "y": 0.2, "z": 0.2 },
      "amplitude": 1.0
    }
  ],
  "listening_positions": [
    { "x": 2.5, "y": 2.0, "z": 1.2 }
  ],
  "frequencies": {
    "min_freq": 20.0,
    "max_freq": 200.0,
    "num_points": 100,
    "spacing": "logarithmic"
  },
  "solver": {
    "mesh_resolution": 4
  }
}
```

## Performance & Accuracy

### Mesh Resolution
The FEM solution is accurate only when the element size is small compared to the wavelength. A rule of thumb is at least 6-10 elements per wavelength.
*   `mesh_resolution` (elements per meter) determines the max accurate frequency.
*   Max Freq $\approx \frac{c}{6 \times (1/\text{resolution})}$.
*   Example: Resolution 4 (elem size 0.25m) $\to$ Max Freq $\approx 343 / (6 \times 0.25) \approx 228$ Hz.

### Parallelism & Memory
The simulator is highly optimized for multi-core CPUs.
*   **Matrices:** Stiffness ($K$) and Mass ($M$) are assembled once (using `f64` for efficiency).
*   **Assembly:** For each frequency, the system matrix $A = K - k^2 M + C_{boundary}$ is assembled in parallel without reallocating sparsity structure.
*   **Solver:** GMRES with ILU preconditioning is used.
*   **Batching:** Multiple frequencies are solved concurrently. Use `--batch-size` or let the auto-tuner manage memory.

### Warm Starting
The `--warm-start` flag enables a two-pass solver:
1.  **Anchor Pass:** Solves every Nth frequency (stride 4 by default).
2.  **Interpolation Pass:** Solves intermediate frequencies using solutions from anchors as initial guesses.
This can reduce iteration count by 30-50% for fine frequency sweeps.

```