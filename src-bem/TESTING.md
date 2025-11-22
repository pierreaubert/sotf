# NumCalc FFI Wrapper Testing Guide

This guide explains how to test the NumCalc FFI wrapper with a real Mesh2HRTF project.

## Quick Start

```bash
# 1. Set up test project (downloads Mesh2HRTF and builds NumCalc)
./scripts/setup_test_project.sh

# 2. Source the environment file
source /tmp/mesh2hrtf_test/test_env.sh

# 3. Run the demo
cargo run --release --example numcalc_ffi_demo --features ffi

# 4. Run integration tests
cargo test --test test_numcalc_integration --features ffi -- --ignored --nocapture
```

## Detailed Testing Instructions

### Option 1: Automated Setup (Recommended)

The automated setup script downloads Mesh2HRTF, builds NumCalc, and configures everything:

```bash
# Run setup script
./scripts/setup_test_project.sh [output_dir]

# Default output directory: /tmp/mesh2hrtf_test
# Custom directory: ./scripts/setup_test_project.sh ~/my_test_dir
```

The script will:
1. Clone the Mesh2HRTF repository
2. Find an example project with NC.inp
3. Build NumCalc from source (if possible)
4. Create environment file for easy setup
5. Print instructions for running tests

After setup, source the environment file:

```bash
source /tmp/mesh2hrtf_test/test_env.sh
```

### Option 2: Manual Setup

If you prefer to set up manually or already have Mesh2HRTF installed:

#### Step 1: Get Mesh2HRTF

```bash
# Clone the repository
git clone https://github.com/Any2HRTF/Mesh2HRTF.git /tmp/Mesh2HRTF

# OR download a specific example
wget https://github.com/Any2HRTF/Mesh2HRTF/raw/master/mesh2hrtf/NumCalc/data/reference_hrtfs/KU100.zip
unzip KU100.zip -d /tmp/
```

#### Step 2: Build NumCalc

```bash
cd /tmp/Mesh2HRTF/mesh2hrtf/NumCalc/src
make

# Verify executable
ls -la NumCalc
./NumCalc --help
```

#### Step 3: Set Environment Variables

```bash
# Point to project directory (must contain NC.inp)
export TEST_PROJECT_DIR=/tmp/Mesh2HRTF/mesh2hrtf/NumCalc/data/reference_hrtfs/KU100

# Point to NumCalc executable
export NUMCALC_PATH=/tmp/Mesh2HRTF/mesh2hrtf/NumCalc/src/NumCalc
```

#### Step 4: Verify Setup

```bash
# Check NC.inp exists
ls -la $TEST_PROJECT_DIR/NC.inp

# Check NumCalc is executable
$NUMCALC_PATH --help
```

## Running Tests

### Integration Tests

The integration test suite (`tests/test_numcalc_integration.rs`) contains comprehensive tests for all FFI functionality:

```bash
# Run all integration tests
cargo test --test test_numcalc_integration --features ffi -- --ignored --nocapture

# Run specific test
cargo test --test test_numcalc_integration test_single_frequency_execution --features ffi -- --ignored --nocapture
```

**Available integration tests:**

1. **test_system_resources** - Verify system resource monitoring
2. **test_numcalc_executable_discovery** - Test NumCalc executable finding logic
3. **test_runner_creation** - Test NumCalcRunner initialization
4. **test_single_frequency_execution** - Run single frequency BEM simulation
5. **test_memory_estimation** - Test memory requirement estimation
6. **test_parallel_execution_small** - Run 3 frequencies in parallel
7. **test_resource_monitoring** - Test ResourceMonitor functionality
8. **test_can_run_task** - Test task feasibility checking

### Demo Example

The `numcalc_ffi_demo` example provides an interactive demonstration:

```bash
cargo run --release --example numcalc_ffi_demo --features ffi
```

This demo showcases:
- System resource monitoring
- Single frequency execution
- Memory estimation
- Parallel execution (5 frequencies)

### Unit Tests

Unit tests don't require NumCalc or a test project:

```bash
# Run all unit tests
cargo test --features ffi

# Run specific module tests
cargo test --features ffi ffi::config
cargo test --features ffi ffi::resources
```

## Test Output

### Console Output

Both the demo and integration tests provide detailed console output:

```
╔════════════════════════════════════════════════════════╗
║         NumCalc FFI Wrapper Demonstration             ║
╚════════════════════════════════════════════════════════╝

Project directory: /tmp/mesh2hrtf_test/test_project

=== Part 1: System Resources ===

System Resources:
  RAM: 8192.5 / 16384.0 MB (50.0% used)
  Available RAM: 8191.5 MB
  CPU Usage: 15.3%
  CPU Cores: 8
  Load Average (1m): 2.45

...
```

### Expected Results

**Successful single frequency execution:**
- Exit code: 0
- Execution time: 1-60 seconds (depends on mesh complexity)
- Output files: be.out/, fe.out/, NC.out created
- No error messages in stderr

**Successful parallel execution:**
- All frequencies complete
- Results properly ordered by frequency index
- Total time < serial time (demonstrates parallelism)
- Resource monitoring prevents system overload

## Troubleshooting

### NumCalc Not Found

```
Error: NumCalc executable not found. Set NUMCALC_PATH or install to PATH
```

**Solutions:**
1. Set `NUMCALC_PATH` environment variable
2. Add NumCalc directory to PATH
3. Run setup script to build NumCalc automatically

### NC.inp Not Found

```
Error: NC.inp not found in project directory
```

**Solutions:**
1. Verify `TEST_PROJECT_DIR` points to correct directory
2. Check that NC.inp exists: `ls $TEST_PROJECT_DIR/NC.inp`
3. Run setup script to download example project

### NumCalc Execution Fails

```
Exit code: 1
Error: [NumCalc error messages]
```

**Common causes:**
1. **Missing mesh files** - NC.inp references non-existent .msh files
2. **Invalid parameters** - Frequency range or solver settings invalid
3. **Insufficient memory** - BEM simulation exceeds available RAM

**Solutions:**
1. Use a known-good example project (run setup script)
2. Check NC.inp file is valid
3. Reduce frequency range or mesh resolution
4. Increase available RAM or use `with_max_ram_gb()` limit

### Compilation Errors

```
error: linking with `cc` failed
```

**Solutions:**
1. Install C++ compiler: `sudo apt install g++` (Linux) or Xcode (macOS)
2. Install make: `sudo apt install make`
3. Check build dependencies in `build.rs`

### Build Script Issues

If NumCalc build fails during `cargo build`:

```bash
# Skip NumCalc build (use pre-built executable)
export SKIP_NUMCALC_BUILD=1
cargo build --features ffi

# Point to pre-built NumCalc
export NUMCALC_PATH=/path/to/NumCalc
cargo build --features ffi

# Point to NumCalc source directory
export NUMCALC_SOURCE_DIR=/path/to/Mesh2HRTF/mesh2hrtf/NumCalc/src
cargo build --features ffi
```

## Performance Testing

### Memory Usage

Test memory monitoring with different project sizes:

```bash
# Small project (fast, ~100MB RAM)
export TEST_PROJECT_DIR=/tmp/mesh2hrtf_test/test_project_small

# Medium project (~1GB RAM)
export TEST_PROJECT_DIR=/tmp/mesh2hrtf_test/test_project_medium

# Large project (>5GB RAM)
export TEST_PROJECT_DIR=/tmp/mesh2hrtf_test/test_project_large

cargo test --test test_numcalc_integration test_memory_estimation --features ffi -- --ignored --nocapture
```

### Parallel Scaling

Test parallel execution with different concurrency levels:

```rust
// Edit tests/test_numcalc_integration.rs

let runner = ParallelBemRunner::new(&project_dir)?
    .with_max_concurrent(4)  // Try 1, 2, 4, 8
    .with_max_cpu_percent(90.0)
    .with_max_ram_gb(16.0);

runner.run_all_frequencies(10)?;  // Try 5, 10, 20 frequencies
```

### Benchmarking

For rigorous benchmarking:

```bash
# Install criterion benchmarks (TODO)
cargo bench --features ffi

# Or manual timing
time cargo run --release --example numcalc_ffi_demo --features ffi
```

## Validation Against Analytical Solutions

After getting NumCalc working, validate against analytical solutions:

```bash
# Run analytical tests (generate JSON output)
cargo test --release -- --nocapture

# Start visualization server
python3 -m http.server 8000 --directory plotting

# Open browser to http://localhost:8000
# Compare BEM results with analytical solutions
```

The analytical test suite provides reference solutions for:
- **1D**: Plane waves, standing waves, damped waves
- **2D**: Rigid cylinder scattering (Bessel/Hankel functions)
- **3D**: Sphere scattering (Mie theory, spherical harmonics)

## CI/CD Integration

For automated testing in CI:

```yaml
# Example GitHub Actions workflow
- name: Setup NumCalc Test Project
  run: ./scripts/setup_test_project.sh /tmp/test

- name: Run FFI Tests
  env:
    TEST_PROJECT_DIR: /tmp/test/test_project
  run: |
    source /tmp/test/test_env.sh
    cargo test --test test_numcalc_integration --features ffi -- --ignored
```

## Additional Resources

- [Mesh2HRTF GitHub Repository](https://github.com/Any2HRTF/Mesh2HRTF)
- [NumCalc Documentation](https://www.mesh2hrtf.org/open-source/documentation/numcalc-documentation.html)
- [BEM Theory Background](../README.md#mathematical-background)
- [FFI Architecture](src/ffi/mod.rs)

## Support

If you encounter issues:

1. Check this guide's troubleshooting section
2. Review integration test output for detailed error messages
3. Verify environment variables are set correctly
4. Check that NumCalc works standalone (outside Rust FFI)
5. Open an issue with full error messages and environment details
