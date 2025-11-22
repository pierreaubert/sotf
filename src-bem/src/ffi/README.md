# NumCalc FFI Wrapper

This module provides a safe Rust wrapper around the NumCalc C++ BEM solver.

## Architecture

### Subprocess Approach

This wrapper uses a **subprocess-based FFI approach** rather than direct C++ FFI:

```
┌─────────────────┐
│   Rust Code     │
│  (bem-rs)       │
└────────┬────────┘
         │ spawn
         ▼
┌─────────────────┐
│   NumCalc       │
│  (C++ executable)│
└────────┬────────┘
         │ writes
         ▼
┌─────────────────┐
│  Output Files   │
│  (be.out/, etc) │
└─────────────────┘
```

**Why subprocess instead of direct FFI?**

1. **ABI Safety**: No C++ ABI compatibility issues across compilers/platforms
2. **Memory Safety**: Process isolation prevents memory corruption
3. **Portability**: Works with any NumCalc build (no recompilation needed)
4. **Robustness**: Process crashes don't take down Rust code
5. **Flexibility**: Easy to swap NumCalc versions or implementations

**Trade-offs:**

- **Pro**: Zero C++ ABI compatibility issues
- **Pro**: Works with existing NumCalc installations
- **Pro**: Process-level isolation and error recovery
- **Con**: Subprocess overhead (~1-5ms per launch)
- **Con**: Communication via files (not shared memory)

For BEM simulations that take seconds to minutes, the subprocess overhead is negligible.

### Parallelism Strategy

**Rayon-based data parallelism** for frequency sweeps:

```rust
use rayon::prelude::*;

// Run 100 frequencies in parallel (CPU-bound workload)
(0..100).into_par_iter().for_each(|freq_idx| {
    let config = NumCalcConfig::single_frequency(freq_idx);
    runner.run(&config)?;
});
```

**Why Rayon (not tokio)?**

1. **CPU-Bound**: BEM is 100% CPU computation (no I/O waiting)
2. **Work Stealing**: Rayon's work-stealing scheduler handles variable-duration tasks
3. **Simplicity**: No async/await complexity for CPU-bound work
4. **Memory Efficiency**: Shared memory between threads (vs async task overhead)

**Why NOT tokio?**

- Tokio is designed for **I/O-bound** workloads (network, disk)
- Async overhead adds no value for CPU-bound BEM
- Blocking CPU work in tokio requires `spawn_blocking` (extra complexity)
- Work-stealing is less efficient than Rayon for parallel CPU tasks

## Module Structure

### Core Components

#### `config.rs` - Configuration Types

Defines configuration for NumCalc execution:

```rust
pub struct NumCalcConfig {
    pub freq_start_idx: Option<usize>,
    pub freq_end_idx: Option<usize>,
    pub max_iterations: usize,
    pub estimate_ram: bool,
    pub check_normals: bool,
    pub timeout: Option<Duration>,
    pub working_dir: Option<PathBuf>,
}

pub struct NumCalcOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub output_files: Vec<PathBuf>,
    pub execution_time: Duration,
    pub peak_memory_mb: Option<f64>,
    pub frequency_index: Option<usize>,
}

pub struct MemoryEstimate {
    pub total_mb: f64,
    pub per_frequency_mb: Vec<f64>,
    pub num_frequencies: usize,
    pub safety_factor: f64,
}
```

#### `runner.rs` - Subprocess Execution

Handles NumCalc subprocess execution:

```rust
pub struct NumCalcRunner {
    executable: PathBuf,
    project_dir: PathBuf,
}

impl NumCalcRunner {
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self>;
    pub fn run(&self, config: &NumCalcConfig) -> Result<NumCalcOutput>;
    pub fn estimate_memory(&self) -> Result<MemoryEstimate>;
}
```

**Executable Discovery Order:**

1. `NUMCALC_PATH` environment variable
2. `./NumCalc/bin/NumCalc` (relative to crate root)
3. System PATH

#### `resources.rs` - System Monitoring

Tracks RAM and CPU usage:

```rust
pub struct SystemResources {
    pub total_ram_mb: f64,
    pub available_ram_mb: f64,
    pub used_ram_mb: f64,
    pub cpu_usage_percent: f64,
    pub num_cpus: usize,
    pub load_average: Option<f64>,
}

pub struct ResourceMonitor {
    // Monitors resource usage over time
    pub fn wait_for_resources(&mut self, required_ram_mb: f64) -> Result<()>;
    pub fn avg_cpu_usage(&self) -> Option<f64>;
    pub fn peak_memory_mb(&self) -> Option<f64>;
}
```

#### `parallel.rs` - Parallel Execution

Rayon-based parallel frequency sweeps with resource management:

```rust
pub struct ParallelBemRunner {
    runner: NumCalcRunner,
    max_concurrent: usize,
    max_ram_gb: f64,
    max_cpu_percent: f64,
    resource_monitor: Arc<Mutex<ResourceMonitor>>,
}

impl ParallelBemRunner {
    pub fn new(project_dir: impl AsRef<Path>) -> Result<Self>;

    pub fn with_max_concurrent(self, max: usize) -> Self;
    pub fn with_max_ram_gb(self, gb: f64) -> Self;
    pub fn with_max_cpu_percent(self, percent: f64) -> Self;

    pub fn run_all_frequencies(&self, num_frequencies: usize) -> Result<Vec<NumCalcOutput>>;
    pub fn run_frequency_range(&self, start: usize, end: usize) -> Result<Vec<NumCalcOutput>>;
}
```

**Resource Management:**

- Monitors RAM/CPU before launching each task
- Blocks new tasks if system is overloaded
- Prevents OOM kills and system thrashing
- Configurable thresholds for safety

## Usage Examples

### Single Frequency Execution

```rust
use bem::ffi::{NumCalcRunner, NumCalcConfig};

let runner = NumCalcRunner::new("/path/to/project")?;

let config = NumCalcConfig::single_frequency(0)
    .with_max_iterations(250)
    .with_timeout(Duration::from_secs(600));

let output = runner.run(&config)?;

if output.is_success() {
    println!("Success! Generated {} files", output.num_output_files());
    println!("Execution time: {:.2}s", output.execution_time.as_secs_f64());
} else {
    eprintln!("Failed: {:?}", output.exit_code);
    eprintln!("{}", output.stderr);
}
```

### Parallel Frequency Sweep

```rust
use bem::ffi::ParallelBemRunner;

let runner = ParallelBemRunner::new("/path/to/project")?
    .with_max_concurrent(4)      // Use 4 CPU cores
    .with_max_cpu_percent(90.0)  // Keep 10% CPU headroom
    .with_max_ram_gb(16.0);      // Limit to 16GB RAM

let results = runner.run_all_frequencies(100)?;

let successful = results.iter().filter(|r| r.is_success()).count();
println!("Completed {}/{} frequencies", successful, results.len());
```

### Memory Estimation

```rust
use bem::ffi::{NumCalcRunner, SystemResources};

let runner = NumCalcRunner::new("/path/to/project")?;
let estimate = runner.estimate_memory()?;

println!("Total memory required: {:.1} GB", estimate.total_mb / 1024.0);
println!("Max per frequency: {:.1} MB", estimate.max_memory_mb());

let resources = SystemResources::current()?;
if estimate.fits_in_ram(resources.available_ram_mb) {
    println!("✓ Simulation will fit in available RAM");
} else {
    println!("✗ Insufficient RAM - need {:.1} GB, have {:.1} GB",
             estimate.max_memory_mb() * estimate.safety_factor / 1024.0,
             resources.available_ram_mb / 1024.0);
}
```

### Resource Monitoring

```rust
use bem::ffi::{ResourceMonitor, SystemResources};

let resources = SystemResources::current()?;
resources.print_summary();

let mut monitor = ResourceMonitor::new()
    .with_max_cpu(90.0)
    .with_max_ram(85.0);

// Wait for resources to become available
monitor.wait_for_resources(1000.0)?; // Wait for 1GB free

// Run computation
let output = runner.run(&config)?;

// Get statistics
if let Some(avg_cpu) = monitor.avg_cpu_usage() {
    println!("Average CPU: {:.1}%", avg_cpu);
}
```

## Testing

See [TESTING.md](../../TESTING.md) for comprehensive testing guide.

### Quick Test

```bash
# Set up test environment
./scripts/setup_test_project.sh
source /tmp/mesh2hrtf_test/test_env.sh

# Run demo
cargo run --release --example numcalc_ffi_demo --features ffi

# Run integration tests
cargo test --test test_numcalc_integration --features ffi -- --ignored --nocapture
```

### Unit Tests

```bash
cargo test --features ffi ffi::config
cargo test --features ffi ffi::resources
```

## Build System Integration

The `build.rs` script handles NumCalc compilation:

### Build Strategies

1. **Use existing NumCalc** (if `NUMCALC_PATH` set or in PATH)
2. **Compile from source** (if `NumCalc/src/` exists)
3. **Skip build** (if `SKIP_NUMCALC_BUILD=1`)

### Environment Variables

- `NUMCALC_PATH`: Path to pre-built NumCalc executable
- `NUMCALC_SOURCE_DIR`: Path to NumCalc source directory
- `SKIP_NUMCALC_BUILD`: Set to `1` to skip compilation
- `GIT_HASH`: Auto-set by build script for version tracking

### Build Examples

```bash
# Use pre-built NumCalc
export NUMCALC_PATH=/usr/local/bin/NumCalc
cargo build --features ffi

# Build from custom source location
export NUMCALC_SOURCE_DIR=/path/to/Mesh2HRTF/mesh2hrtf/NumCalc/src
cargo build --features ffi

# Skip NumCalc build (use system installation)
export SKIP_NUMCALC_BUILD=1
cargo build --features ffi
```

## Performance Considerations

### Subprocess Overhead

- **Launch time**: ~1-5ms per subprocess
- **Negligible for BEM**: Simulations take seconds to minutes
- **Batch if needed**: Use frequency ranges instead of single frequencies

### Parallel Scaling

Typical scaling for frequency sweeps:

- **1 core**: 100 frequencies × 10s = 1000s (16.7 minutes)
- **4 cores**: 100 frequencies × 10s / 4 = 250s (4.2 minutes)
- **8 cores**: 100 frequencies × 10s / 8 = 125s (2.1 minutes)

**Scalability factors:**

- Nearly linear scaling for CPU-bound BEM
- Limited by RAM (each frequency needs memory)
- Resource monitoring prevents overload

### Memory Management

**Memory estimate before execution:**

```rust
let estimate = runner.estimate_memory()?;
let max_concurrent = (available_ram_mb / estimate.max_memory_mb()).floor() as usize;
let runner = ParallelBemRunner::new(project_dir)?
    .with_max_concurrent(max_concurrent.min(num_cpus));
```

## Error Handling

All operations return `anyhow::Result` with context:

```rust
use anyhow::Context;

let runner = NumCalcRunner::new(project_dir)
    .context("Failed to initialize NumCalc runner")?;

let output = runner.run(&config)
    .context("NumCalc execution failed")?;
```

**Common error scenarios:**

1. **NumCalc not found**: Check `NUMCALC_PATH`, install NumCalc
2. **NC.inp missing**: Verify project directory structure
3. **Execution timeout**: Increase timeout or reduce mesh size
4. **Insufficient memory**: Reduce concurrent tasks or mesh resolution
5. **Invalid parameters**: Check NC.inp file format

## Future Enhancements

### Planned Features

- [ ] Parse NumCalc output for detailed error messages
- [ ] Implement Memory.txt parsing for accurate estimates
- [ ] Add progress tracking for long simulations
- [ ] Support for cancellation/interruption
- [ ] Benchmark suite for performance validation
- [ ] Direct memory monitoring via process stats

### Not Planned

- ✗ Direct C++ FFI (subprocess approach is working well)
- ✗ Tokio integration (Rayon is optimal for CPU-bound work)
- ✗ Shared memory IPC (files are sufficient, portable)

## References

- [NumCalc Documentation](https://www.mesh2hrtf.org/open-source/documentation/numcalc-documentation.html)
- [Mesh2HRTF GitHub](https://github.com/Any2HRTF/Mesh2HRTF)
- [Rayon Documentation](https://docs.rs/rayon/latest/rayon/)
- [sysinfo Documentation](https://docs.rs/sysinfo/latest/sysinfo/)
