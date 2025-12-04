# End-to-End (E2E) Tests

## Overview

These tests validate the complete optimization pipeline using the real Tauri backend. They cover all 5 input source scenarios to ensure no regressions occur.

## Test Coverage

### Input Sources Tested

1. **Speaker with CEA2034/Spinorama** - Uses Spinorama API
2. **Speaker without CEA2034** - On-axis only (currently skipped)
3. **Headphone + Target** - Uses Harman target curves
4. **File ± Target** - CSV files with optional target
5. **Captured Audio** - Simulated microphone sweep data

### What Each Test Validates

- ✅ Optimization completes successfully
- ✅ Filter parameters generated (correct count)
- ✅ All required curves present:
  - `input_curve`
  - `deviation_curve`
  - `filter_response`
  - `filter_plots`
- ✅ Spinorama data only when applicable
- ✅ Plot generation works (4 plots for speakers, 1 for others)

## Running E2E Tests

### Prerequisites

- Tauri backend must be built
- Node modules installed (`npm install`)
- **Tauri application must be running** (dev mode)

### Important: E2E Tests Auto-Skip

**E2E tests are automatically skipped when run via `npm test`** because they require a running Tauri backend.

To run E2E tests, you need to:

1. **Start Tauri in dev mode** (in one terminal):

   ```bash
   npm run tauri dev
   ```

2. **Run tests in Tauri environment** (the tests run inside the Tauri webview)

### Current Behavior

```bash
# Regular test run - E2E tests are SKIPPED
npm test
# Output: 97 passed | 9 skipped (E2E tests auto-skipped)

# Run only unit tests (fast)
npm run test:unit
# Output: 97 passed | 2 skipped

# E2E tests require running Tauri app (auto-skipped)
npm run test:e2e
# Output: 9 skipped (no Tauri backend available in test environment)

# Force E2E tests to run (attempts to invoke backend)
npm run test:e2e:force
# Output: Will try to run tests, may fail if backend unavailable
```

### Running E2E Tests

**Option 1: Force Run E2E Tests (Recommended)**

This attempts to run E2E tests by bypassing the Tauri environment check:

```bash
npm run test:e2e:force
```

Or set the environment variable manually:

```bash
FORCE_E2E=true npm run test:e2e
```

⚠️ **Note:** Tests will fail if they cannot invoke the Tauri backend. If you see errors like "failed to invoke", the backend is not available in the test environment.

**Option 2: Run with Tauri Dev Mode**

```bash
# Terminal 1: Start Tauri in dev mode
npm run tauri dev

# Terminal 2: In the Tauri webview, open DevTools console and run:
# (This is for future WebDriver integration)
```

**Option 3: Tauri WebDriver (Future)**

```bash
# Not yet implemented
tauri-driver test
```

**Option 4: CI with Tauri CLI (Future)**

```yaml
- run: cargo tauri test
```

### Performance

E2E tests take longer than unit tests:

- Each test has 30-second timeout
- Total suite: ~2-5 minutes
- Use reduced `maxeval: 500` for faster execution

## Fixtures

Test data files are located in `fixtures/`:

```
fixtures/
├── headphone/
│   └── sample_headphone.csv       # Realistic headphone FR
├── file/
│   ├── input.csv                  # General input curve
│   └── target.csv                 # Flat target curve
└── capture/
    └── sweep_response.json        # Simulated capture data
```

### Modifying Fixtures

To add new test cases:

1. Add CSV/JSON file to appropriate `fixtures/` subdirectory
2. Add new test in `optimization-e2e.test.ts`
3. Use helper functions for validation

Example CSV format:

```csv
frequency,spl
20,70.5
25,71.2
...
```

## CI Integration

### GitHub Actions (future)

```yaml
- name: Run E2E Tests
  run: npm run test:e2e
  timeout-minutes: 10
```

### Test Isolation

E2E tests:

- Use real backend (not mocked)
- Read from filesystem fixtures
- Network access for speaker API tests
- Independent of unit tests

## Debugging

### Enable Verbose Logging

Tests output progress messages:

```
✅ Speaker+CEA2034: Generated 5 filters
   Preference score: 5.42 → 7.18
✅ Headphone: Generated 5 filters
✅ File+Target: Generated 4 filters
⏱️  Optimization completed in 8234ms
```

### Common Issues

**Test timeout:**

- Increase `maxeval` value reduces test time
- Check backend is running in dev mode

**File not found:**

- Verify fixture paths are correct
- Check `FIXTURES_DIR` resolves properly

**Backend errors:**

- Check Rust compilation succeeded
- Verify Tauri bindings are up to date

## Performance Benchmarks

Baseline performance targets:

| Test Case             | Target Time | Max Time |
| --------------------- | ----------- | -------- |
| File (3 filters)      | <5s         | <10s     |
| Headphone (5 filters) | <8s         | <15s     |
| Speaker (5 filters)   | <10s        | <20s     |

## Future Enhancements

- [ ] Add speaker cache to avoid API rate limits
- [ ] Parallel test execution
- [ ] Screenshot comparison for plots
- [ ] Performance regression detection
- [ ] Test matrix (different algorithms)

## Test Data Sources

- **Headphone curves**: Based on realistic measurements
- **Speaker data**: KEF LS50 Meta from Spinorama.org
- **Capture data**: Simulated pink noise sweep

All test data is synthetic and can be freely modified.
