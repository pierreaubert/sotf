# E2E WebDriver Testing Setup - Complete Guide

This document provides step-by-step instructions for setting up and running end-to-end UI tests with WebDriver for the SOTF Tauri application.

## ✅ What's Been Set Up

The following has been configured for you:

1. **WebDriverIO installed** - Test framework and dependencies
2. **Configuration file** - `wdio.conf.ts` at project root
3. **Test files** - Three comprehensive test suites in `src-ui-frontend/tests/e2e-ui/`:
   - `app-launch.spec.ts` - App initialization and navigation
   - `optimization-workflow.spec.ts` - Full optimization workflows
   - `audio-player.spec.ts` - Audio player functionality
4. **npm scripts** - Added to `package.json`:
   - `npm run test:e2e-ui`
   - `npm run test:e2e-ui:headless`
   - `npm run test:all`
5. **Documentation** - Comprehensive README and guides
6. **Helper utilities** - Test helper functions in `test-helpers.ts`

## 🔧 Required System Setup

### For Linux (Debian/Ubuntu)

You need to install WebKitWebDriver:

```bash
sudo apt-get update
sudo apt-get install webkit2gtk-driver
```

Verify installation:
```bash
which WebKitWebDriver
# Should output: /usr/bin/WebKitWebDriver
```

### For Linux (Other Distributions)

**Arch Linux:**
```bash
sudo pacman -S webkit2gtk
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel
```

### For Windows

1. Check your Edge version: `edge://version/`
2. Download matching Edge Driver from: https://developer.microsoft.com/en-us/microsoft-edge/tools/webdriver/
3. Extract and add to PATH or place in project root

### For macOS

⚠️ **WebDriver testing is not officially supported on macOS** due to lack of WKWebView driver.

**Workaround options:**
- Use Linux VM or container for testing
- Use CI/CD with Linux runners
- Test manually on macOS

## ✅ Verify Setup

### 1. Check tauri-driver

```bash
tauri-driver --version
# Should output: tauri-driver 2.0.4 or similar
```

If not installed:
```bash
cargo install tauri-driver
```

### 2. Check WebKitWebDriver (Linux only)

```bash
WebKitWebDriver --version
# Should output version information
```

### 3. Verify Node Dependencies

```bash
npm list webdriverio
npm list @wdio/cli
```

All should show as installed.

### 4. Check Tauri Build

Build your app to ensure the binary exists:

```bash
npm run tauri build
```

This creates `target/release/sotf` which the tests will use.

## 🚀 Running Tests

### First Time Setup

1. **Install system dependencies** (see above)
2. **Build the app:**
   ```bash
   npm run tauri build
   ```
3. **Run tests:**
   ```bash
   npm run test:e2e-ui
   ```

### Standard Usage

```bash
# Run all e2e UI tests
npm run test:e2e-ui

# Run specific test file
npx wdio run wdio.conf.ts --spec ./src-ui-frontend/tests/e2e-ui/app-launch.spec.ts

# Run all tests (unit + e2e)
npm run test:all
```

### Expected Output

```
Execution of 3 workers started at 2025-11-17T...

[0-0] RUNNING in chrome - /e2e-ui/app-launch.spec.ts
[0-1] RUNNING in chrome - /e2e-ui/optimization-workflow.spec.ts
[0-2] RUNNING in chrome - /e2e-ui/audio-player.spec.ts

[0-0] PASSED in chrome - /e2e-ui/app-launch.spec.ts (15.2s)
[0-1] PASSED in chrome - /e2e-ui/optimization-workflow.spec.ts (45.8s)
[0-2] PASSED in chrome - /e2e-ui/audio-player.spec.ts (12.1s)

Spec Files:  3 passed, 3 total (100% completed) in 00:01:13
```

## 🐛 Troubleshooting

### Issue: "tauri-driver not found"

```bash
# Install tauri-driver
cargo install tauri-driver

# Verify it's in PATH
which tauri-driver
```

### Issue: "WebKitWebDriver not found" (Linux)

```bash
# Install webkit2gtk-driver
sudo apt-get install webkit2gtk-driver

# Verify installation
which WebKitWebDriver
```

### Issue: "Application binary not found"

```bash
# Rebuild the app
npm run tauri build

# Check binary exists
ls -lh target/release/sotf
```

### Issue: Tests hang during build

The tests automatically build the app before running, which can take time.

**Option 1: Pre-build the app**
```bash
# Build once
npm run tauri build

# Then run tests (they'll skip rebuild)
npm run test:e2e-ui
```

**Option 2: Increase timeout**

Edit `wdio.conf.ts`:
```typescript
connectionRetryTimeout: 300000  // 5 minutes
```

### Issue: Tests fail with "Element not found"

This likely means the UI structure has changed. Options:

1. **Update selectors** in test files
2. **Add data-testid attributes** (see `DATA_TESTID_GUIDE.md`)
3. **Check if app is fully loaded** (add waits if needed)

### Issue: Port already in use

If `tauri-driver` port (4444) is in use:

```bash
# Find and kill process using port 4444
lsof -ti:4444 | xargs kill -9

# Or use different port in wdio.conf.ts
```

## 📊 Test Coverage

### Current Tests

| Test Suite | Tests | Coverage |
|------------|-------|----------|
| `app-launch.spec.ts` | 7 | App initialization, navigation, use case selection |
| `optimization-workflow.spec.ts` | 6 | Full speaker optimization, validation, cancellation |
| `audio-player.spec.ts` | 12 | Player controls, EQ, spectrum analyzer |
| **Total** | **25** | **Core workflows** |

### Missing Coverage

Areas not yet covered by tests:

- ❌ Headphone optimization workflow
- ❌ Audio capture workflow
- ❌ File dialog interactions (platform-dependent)
- ❌ Keyboard shortcuts
- ❌ Error recovery scenarios
- ❌ Export file downloads

## 📝 Adding New Tests

### Quick Start

1. **Create test file:**
   ```bash
   touch src-ui-frontend/tests/e2e-ui/my-feature.spec.ts
   ```

2. **Write test:**
   ```typescript
   import { selectUseCase, runOptimization } from './test-helpers';

   describe("My Feature", () => {
     it("should do something", async () => {
       await selectUseCase("speaker");
       const button = await $("#my_button");
       await button.click();
       // ... assertions
     });
   });
   ```

3. **Run test:**
   ```bash
   npm run test:e2e-ui
   ```

### Using Test Helpers

```typescript
import {
  selectUseCase,
  configureSpeaker,
  setOptimizationParams,
  runOptimization,
  getOptimizationScores,
  navigateToStep,
} from './test-helpers';

describe("Custom Workflow", () => {
  it("should optimize speaker", async () => {
    // Select use case
    await selectUseCase("speaker");

    // Configure speaker
    await configureSpeaker("KEF LS50 Meta", "vendor", "asr");

    // Go to optimization
    await navigateToStep(3);

    // Set parameters
    await setOptimizationParams({
      numFilters: 5,
      maxeval: 500,
      algo: "nlopt:cobyla"
    });

    // Run and wait
    await runOptimization(60000);

    // Verify results
    const scores = await getOptimizationScores();
    expect(scores.before).to.not.equal("-");
  });
});
```

## 🔄 CI/CD Integration

### GitHub Actions

Create `.github/workflows/e2e-tests.yml`:

```yaml
name: E2E WebDriver Tests

on:
  push:
    branches: [main, master]
  pull_request:

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    timeout-minutes: 30

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            webkit2gtk-driver \
            libwebkit2gtk-4.1-dev \
            libgtk-3-dev \
            libayatana-appindicator3-dev

      - name: Install tauri-driver
        run: cargo install tauri-driver

      - name: Install npm dependencies
        run: npm ci

      - name: Run E2E tests
        run: npm run test:e2e-ui

      - name: Upload screenshots on failure
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: e2e-screenshots
          path: test-results/screenshots/
          retention-days: 7
```

## 📚 Documentation

Full documentation is available in:

- **`src-ui-frontend/tests/e2e-ui/README.md`** - Comprehensive testing guide
- **`src-ui-frontend/tests/e2e-ui/DATA_TESTID_GUIDE.md`** - Adding data-testid attributes
- **`src-ui-frontend/tests/e2e-ui/test-helpers.ts`** - Reusable test utilities

## 🎯 Next Steps

1. **Install WebKitWebDriver** (Linux) or Edge Driver (Windows)
2. **Build the Tauri app:** `npm run tauri build`
3. **Run tests:** `npm run test:e2e-ui`
4. **Add data-testid attributes** to UI components (optional, improves reliability)
5. **Write more tests** for uncovered workflows
6. **Set up CI/CD** for automated testing

## 💡 Tips for Success

- ✅ Build app before running tests for faster execution
- ✅ Use `test-helpers.ts` for common operations
- ✅ Add `data-testid` attributes for stable selectors
- ✅ Take screenshots on failures for debugging
- ✅ Use explicit waits instead of `pause()` where possible
- ✅ Run tests on CI to catch regressions
- ❌ Don't rely on timing/sleeps
- ❌ Don't test implementation details
- ❌ Don't skip cleanup between tests

## 🆘 Getting Help

If you encounter issues:

1. Check this document
2. Read `src-ui-frontend/tests/e2e-ui/README.md`
3. Review test examples
4. Check WebDriverIO docs: https://webdriver.io/
5. Check Tauri testing docs: https://v2.tauri.app/develop/tests/

## 📦 Summary

Your e2e testing infrastructure is now ready to use! The setup includes:

✅ WebDriverIO configuration
✅ Three comprehensive test suites
✅ Helper utilities for common operations
✅ Detailed documentation
✅ npm scripts for easy execution
✅ CI/CD integration examples

All you need to do is:
1. Install system dependencies (WebKitWebDriver)
2. Build the app
3. Run the tests

Happy testing! 🎉
