# End-to-End UI Tests with WebDriver

This directory contains true end-to-end UI tests for the SOTF (Sound of the Future) Tauri application using WebDriverIO and tauri-driver.

## Overview

Unlike the integration tests in `../e2e/` which test Tauri commands directly, these tests **interact with the actual UI** by:

- Clicking buttons and UI elements
- Filling form fields
- Navigating through the multi-step workflow
- Verifying visual feedback and state changes
- Testing complete user workflows

## Architecture

```
┌─────────────────────┐
│   WebDriverIO       │  Test runner
│   (Mocha)           │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│   tauri-driver      │  WebDriver server
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│  WebKitWebDriver    │  Platform WebDriver (Linux)
│  (webkit2gtk)       │  EdgeDriver (Windows)
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│   SOTF Tauri App    │  Your application
│   (Release Build)   │
└─────────────────────┘
```

## Prerequisites

### System Requirements

**Linux (Debian/Ubuntu):**
```bash
sudo apt-get install webkit2gtk-driver
```

**Windows:**
- Download Microsoft Edge Driver matching your Edge version
- Add to PATH or place in project directory

**macOS:**
⚠️ **Not supported** - macOS lacks WKWebView driver tools

### Node Dependencies

All dependencies are installed via npm:
```bash
npm install
```

This installs:
- `webdriverio` - Test framework
- `@wdio/cli` - WebDriverIO CLI
- `@wdio/local-runner` - Local test runner
- `@wdio/mocha-framework` - Mocha test framework
- `@wdio/spec-reporter` - Console reporter
- `@types/mocha` - TypeScript types

### Rust Dependencies

The `tauri-driver` binary is required and installed via:
```bash
cargo install tauri-driver
```

This is already done if you've followed the setup.

## Running Tests

### Quick Start

```bash
# Run all e2e UI tests
npm run test:e2e-ui
```

This will:
1. Build the Tauri app in release mode (`npm run tauri build`)
2. Start tauri-driver
3. Run all test specs in `src-ui-frontend/tests/e2e-ui/`
4. Generate test reports
5. Save screenshots on failures
6. Cleanup and exit

### Test Scripts

```bash
# Standard run with visual window
npm run test:e2e-ui

# Headless mode (if supported)
npm run test:e2e-ui:headless

# Run all tests (unit + e2e-ui)
npm run test:all

# Run specific test file
npx wdio run wdio.conf.ts --spec ./src-ui-frontend/tests/e2e-ui/app-launch.spec.ts
```

### Watch Mode

WebDriverIO doesn't have built-in watch mode, but you can use:

```bash
# Terminal 1: Keep tauri in dev mode running
npm run tauri dev

# Terminal 2: Re-run tests manually when needed
npm run test:e2e-ui
```

## Test Structure

### Test Files

```
src-ui-frontend/tests/e2e-ui/
├── README.md                       # This file
├── app-launch.spec.ts              # App initialization and navigation
├── optimization-workflow.spec.ts   # Full optimization workflows
└── audio-player.spec.ts            # Audio playback and controls
```

### Test Naming Convention

- **File names:** `*.spec.ts` (required by wdio.conf.ts)
- **Describe blocks:** Feature or workflow name
- **Test cases:** `it("should ...")` format

### Example Test

```typescript
describe("My Feature", () => {
  before(async function () {
    this.timeout(10000);
    // Setup
  });

  it("should perform action X", async () => {
    const button = await $("#my_button");
    await button.click();

    const result = await $("#result");
    await expect(result).toBeDisplayed();
    const text = await result.getText();
    expect(text).to.equal("Success");
  });
});
```

## Configuration

Main configuration is in `/wdio.conf.ts`:

```typescript
{
  specs: ["./src-ui-frontend/tests/e2e-ui/**/*.spec.ts"],
  maxInstances: 1,  // Run tests serially
  capabilities: [{
    "tauri:options": {
      application: "./target/release/sotf"
    }
  }],
  framework: "mocha",
  reporters: ["spec"],
  timeout: 60000  // 60 seconds
}
```

### Key Configuration Options

- **`application`**: Path to Tauri binary (built before tests)
- **`maxInstances: 1`**: Tests run one at a time (required for stability)
- **`timeout`**: Default test timeout (can be overridden per test)
- **`onPrepare`**: Builds the app before testing
- **`afterTest`**: Captures screenshots on failure

## Writing Tests

### Selecting Elements

```typescript
// By ID
const button = await $("#optimize_btn");

// By CSS selector
const header = await $(".step-title");

// By attribute
const card = await $('[data-use-case="speaker"]');

// Multiple elements
const buttons = await $$("button");

// Find within element
const parent = await $("#parent");
const child = await parent.$("#child");
```

### Interacting with Elements

```typescript
// Click
await button.click();

// Set input value
await input.setValue("KEF LS50 Meta");
await input.clearValue();

// Select dropdown
const select = await $("#version");
await select.selectByVisibleText("vendor");
await select.selectByValue("vendor");
await select.selectByIndex(1);

// Check/uncheck
await checkbox.click();

// Move slider
await slider.setValue(50);
```

### Assertions

```typescript
// Element state
await expect(button).toBeDisplayed();
await expect(button).toBeEnabled();
await expect(button).toExist();

// Text content
const text = await element.getText();
expect(text).to.equal("Expected");
expect(text).to.include("partial");

// Attributes
const classes = await element.getAttribute("class");
expect(classes).to.include("active");

// CSS properties
const color = await element.getCSSProperty("color");
expect(color.value).to.equal("rgb(255, 0, 0)");
```

### Waiting

```typescript
// Wait for element to be displayed
await button.waitForDisplayed({ timeout: 5000 });

// Wait for custom condition
await browser.waitUntil(
  async () => {
    const text = await $("#status").getText();
    return text.includes("Complete");
  },
  {
    timeout: 10000,
    timeoutMsg: "Status did not update"
  }
);

// Simple pause (use sparingly)
await browser.pause(1000);
```

### Timeouts

```typescript
// Per-test timeout
it("slow test", async function () {
  this.timeout(120000); // 2 minutes
  // ... test code
});

// Per-suite timeout
describe("Slow Suite", function () {
  this.timeout(120000);

  it("test 1", async () => { });
  it("test 2", async () => { });
});
```

## Test Coverage

### Current Test Coverage

| Feature | Status | File |
|---------|--------|------|
| App Launch | ✅ | `app-launch.spec.ts` |
| Use Case Selection | ✅ | `app-launch.spec.ts` |
| Step Navigation | ✅ | `app-launch.spec.ts` |
| Speaker Optimization | ✅ | `optimization-workflow.spec.ts` |
| File-Based Optimization | ⚠️ Partial | `optimization-workflow.spec.ts` |
| Parameter Validation | ✅ | `optimization-workflow.spec.ts` |
| Cancellation | ✅ | `optimization-workflow.spec.ts` |
| Audio Player Controls | ✅ | `audio-player.spec.ts` |
| EQ Toggle | ✅ | `audio-player.spec.ts` |
| Spectrum Analyzer | ✅ | `audio-player.spec.ts` |
| Export/Download | ⚠️ Basic | `optimization-workflow.spec.ts` |

**Legend:**
- ✅ Fully tested
- ⚠️ Partially tested or requires manual interaction
- ❌ Not yet implemented

### Missing Coverage

- [ ] Headphone optimization workflow
- [ ] Audio capture workflow
- [ ] File dialog interactions (requires platform automation)
- [ ] Keyboard shortcuts
- [ ] Error state recovery
- [ ] Cross-browser testing (if applicable)

## Debugging

### Enable Verbose Logging

```bash
# Set log level in wdio.conf.ts
logLevel: 'debug'
```

### Screenshots on Failure

Screenshots are automatically saved to `test-results/screenshots/` when tests fail.

```
test-results/screenshots/
└── should-complete-optimization-2025-11-17T10-30-45.png
```

### REPL Debugging

Add `await browser.debug()` in your test to pause and interact:

```typescript
it("debug test", async () => {
  await $("#button").click();
  await browser.debug();  // Pauses here
  // ... rest of test
});
```

This opens an interactive REPL where you can run WebDriver commands.

### Common Issues

**Issue:** `tauri-driver not found`
```bash
# Solution: Install tauri-driver
cargo install tauri-driver
```

**Issue:** `WebKitWebDriver not found` (Linux)
```bash
# Solution: Install webkit2gtk-driver
sudo apt-get install webkit2gtk-driver
```

**Issue:** Tests hang during build
```bash
# Solution: Increase timeout in wdio.conf.ts
connectionRetryTimeout: 180000  # 3 minutes
```

**Issue:** Element not found
```typescript
// Solution: Add explicit wait
await element.waitForDisplayed({ timeout: 5000 });
```

**Issue:** Test fails intermittently
```typescript
// Solution: Add pause after navigation
await browser.pause(500);
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: E2E UI Tests

on: [push, pull_request]

jobs:
  e2e-ui-tests:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Setup Node
        uses: actions/setup-node@v3
        with:
          node-version: '20'

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y webkit2gtk-driver libwebkit2gtk-4.1-dev

      - name: Install tauri-driver
        run: cargo install tauri-driver

      - name: Install npm dependencies
        run: npm install

      - name: Run E2E UI tests
        run: npm run test:e2e-ui

      - name: Upload screenshots on failure
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: test-screenshots
          path: test-results/screenshots/
```

## Performance Considerations

### Test Duration

- **Build time:** ~30-60 seconds (release build)
- **App launch:** ~2-5 seconds per test
- **Typical test:** 5-30 seconds
- **Full suite:** 5-10 minutes

### Optimization Tips

1. **Use fewer filters** in optimization tests (`num_filters: 3` instead of 5)
2. **Reduce maxeval** for faster convergence (`maxeval: 100` for tests)
3. **Run tests serially** (already configured)
4. **Reuse application instance** where possible (advanced)

## Adding New Tests

### Step-by-Step Guide

1. **Create new spec file:**
   ```bash
   touch src-ui-frontend/tests/e2e-ui/my-feature.spec.ts
   ```

2. **Add test structure:**
   ```typescript
   describe("My Feature", () => {
     before(async function () {
       this.timeout(30000);
       // Setup: navigate to feature
     });

     it("should do something", async () => {
       // Test implementation
     });
   });
   ```

3. **Run tests:**
   ```bash
   npm run test:e2e-ui
   ```

### Best Practices

- ✅ Use descriptive test names
- ✅ Add timeouts for slow operations
- ✅ Wait for elements explicitly
- ✅ Clean up state between tests
- ✅ Use meaningful assertions
- ❌ Don't rely on timing/sleeps
- ❌ Don't test implementation details
- ❌ Don't skip cleanup

## Troubleshooting

### Test Failures

1. **Check screenshot** in `test-results/screenshots/`
2. **Review error message** in console output
3. **Add `browser.debug()`** before failing line
4. **Check element selectors** are still valid
5. **Verify app state** is as expected

### Platform-Specific Issues

**Linux:**
- Ensure `webkit2gtk-driver` is installed
- Check Xvfb is running for headless mode

**Windows:**
- Match Edge Driver version to Edge browser
- Check PATH includes Edge Driver

## Resources

- [WebDriverIO Documentation](https://webdriver.io/)
- [Tauri Testing Guide](https://v2.tauri.app/develop/tests/)
- [Mocha Documentation](https://mochajs.org/)
- [WebDriver Protocol](https://w3c.github.io/webdriver/)

## Support

For issues or questions:
1. Check this README
2. Review test examples
3. Consult WebDriverIO docs
4. Open GitHub issue
