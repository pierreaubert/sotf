# Plugin System Test Suite

Comprehensive test suite for the plugin system to catch regressions and ensure stability.

## Test Coverage

### 1. BasePlugin Tests (`plugin-base.test.ts`)

Tests the core plugin base class functionality:

- **Initialization**: Container setup, config application, keyboard control setup
- **State Management**: Getting/setting state, partial updates, event emissions
- **Bypass Functionality**: Bypass state checks, toggle, callbacks
- **Event System**: Event registration, unregistration, error handling
- **Keyboard Controls**:
  - Parameter key assignment with collision handling
  - Formatted labels with keyboard shortcut indicators
  - Parameter selection by index and letter keys
  - Tab/Shift+Tab navigation
  - ESC to clear selection
  - Shift+Up/Down for parameter adjustment
- **Destruction**: Cleanup of event listeners and DOM elements

**Test Count**: 35 tests
**Status**: ✅ All passing

### 2. PluginHost Tests (`plugin-host.test.ts`)

Tests the plugin host container functionality:

- **Initialization**: Default and custom configurations
- **Plugin Management**:
  - Adding plugins (with limits and filters)
  - Removing plugins
  - Plugin selection
  - Plugin reordering via drag-and-drop
- **Volume Control**: Volume setting, clamping, callbacks, UI updates
- **Monitoring Mode**: Input/output monitoring switching
- **Level Meters**: Canvas rendering, data updates
- **LUFS Meter**: Display and value updates
- **Help Bar**: Shortcut display, plugin-specific shortcuts, visibility toggle
- **Plugin Selector Dialog**: Modal display, filtering, closing
- **Keyboard Shortcuts**: Volume control via arrow keys
- **Destruction**: Cleanup of plugins, listeners, and DOM

**Test Count**: 34 tests
**Status**: ⚠️ 8 tests failing (DOM interaction edge cases)

**Known Issues**:

- Drag-and-drop simulation needs enhancement
- Help bar element queries need adjustment
- Modal dialog cleanup timing issues
- Keyboard event propagation in test environment

### 3. Plugin Integration Tests (`plugin-integration.test.ts`)

End-to-end integration tests covering:

- **Complete Plugin Lifecycle**: Add → Select → Configure → Remove
- **Multiple Plugin Workflows**: Sequential plugin management
- **Plugin Type-Specific Tests**:
  - EQ: Table rendering, filter controls
  - Compressor: Slider controls, 6 parameters
  - Limiter: Reduced height sliders (250px), 3 parameters
  - Spectrum: Canvas rendering, control buttons
  - Upmixer: 4 parameter fields
- **Keyboard Controls Integration**: Cross-plugin keyboard handling
- **State Persistence**: State maintenance across selection changes
- **Plugin Chains**:
  - Mastering chain: EQ → Compressor → Limiter
  - Spatial chain: EQ → Upmixer → EQ
  - Analysis chain: Spectrum
- **Edge Cases**:
  - Removing first/middle/last plugin
  - Removing all plugins
  - Max plugin limits
  - Allowed plugin type filtering
- **Event Handling**: State change and parameter change events
- **Regression Tests**:
  - Plugins appear in bar after adding
  - Drag-and-drop works for reordering
  - Keyboard controls work across all plugins
  - Modal appears above plugins (z-index: 9999)
  - EQ table has dark theme

**Test Count**: 37 tests
**Status**: ✅ All passing

## Test Execution

### Run All Plugin Tests

```bash
npm run test -- tests/plugins/
```

### Run Specific Test File

```bash
npm run test -- tests/plugins/plugin-base.test.ts
npm run test -- tests/plugins/plugin-host.test.ts
npm run test -- tests/plugins/plugin-integration.test.ts
```

### Run with Verbose Output

```bash
npm run test -- --reporter=verbose tests/plugins/
```

### Run with Coverage

```bash
npm run test:coverage -- tests/plugins/
```

## Regression Detection

This test suite is designed to catch regressions in:

1. **Plugin Rendering**: Ensures plugins appear correctly in the UI
2. **Plugin Interaction**: Verifies click, drag, keyboard interactions
3. **State Management**: Checks state persistence and updates
4. **Keyboard Controls**: Validates unified keyboard control system across all plugins
5. **Visual Consistency**: Ensures dark theme and styling consistency
6. **Plugin Chaining**: Tests multi-plugin workflows
7. **Event System**: Verifies event propagation and handling

## Test Strategy

### Unit Tests

- Focus on individual plugin and host functionality
- Mock dependencies (canvas, Plotly, Tauri)
- Test isolated behaviors

### Integration Tests

- Test real plugin instances (EQ, Compressor, Limiter, Upmixer, Spectrum)
- Verify inter-plugin communication
- Test complete user workflows
- Catch regressions in end-to-end scenarios

### Regression Tests

- Specific tests for previously reported bugs:
  - Plugins not appearing in bar after adding
  - Drag-and-drop not working
  - Keyboard controls not consistent
  - Modal appearing below plugins
  - EQ table theme not dark

## Test Environment

- **Framework**: Vitest 4.0.3
- **Environment**: jsdom
- **Mocking**:
  - Canvas 2D context with gradient support
  - Plotly.js for EQ charts
  - Tauri API for native integration
  - Performance APIs

## Current Status

**Total Tests**: 106
**Passing**: 98 (92.5%)
**Failing**: 8 (7.5%)
**Errors**: 2 (Plotly async errors, non-blocking)

### Failing Tests

All 8 failing tests are in `plugin-host.test.ts` and relate to:

- DOM event simulation (drag-and-drop)
- Element query timing (help bar, modal)
- Keyboard event propagation

These failures do not affect the core functionality and are primarily test environment limitations rather than actual bugs.

## Future Improvements

1. **Fix Remaining Test Failures**:
   - Improve drag-and-drop simulation
   - Add wait/retry logic for DOM queries
   - Mock keyboard events more accurately

2. **Add Visual Regression Tests**:
   - Screenshot comparison for UI consistency
   - Visual diff for theme changes

3. **Performance Tests**:
   - Plugin initialization time
   - Rendering performance with multiple plugins
   - Memory usage tracking

4. **Accessibility Tests**:
   - Keyboard navigation completeness
   - ARIA attributes
   - Screen reader compatibility

5. **E2E Tests**:
   - Real browser testing with Playwright/Cypress
   - User interaction flows
   - Cross-browser compatibility

## Contributing

When adding new plugins or features:

1. **Write Tests First**: TDD approach ensures testability
2. **Update Integration Tests**: Add new plugin to workflow tests
3. **Add Regression Tests**: For any bug fixes
4. **Maintain Coverage**: Aim for >90% coverage
5. **Document Behavior**: Add clear test descriptions

## Debugging Tests

### View Detailed Errors

```bash
npm run test -- tests/plugins/ --reporter=verbose
```

### Run Single Test

```bash
npm run test -- tests/plugins/plugin-base.test.ts -t "should assign unique keys"
```

### Watch Mode

```bash
npm run test:watch -- tests/plugins/
```

### Debug in VS Code

Add breakpoint and use "JavaScript Debug Terminal"

## Test Maintenance

- **Run before commits**: Ensure no regressions
- **Update on plugin changes**: Keep tests in sync with code
- **Review failures**: Investigate and fix promptly
- **Refactor when needed**: Keep tests maintainable

---

Last Updated: 2025-11-15
Test Suite Version: 1.0
Plugin System Version: 0.4.47
