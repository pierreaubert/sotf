# GPUI Audio Player Tests

Comprehensive test suite for the GPUI audio player application, covering all new features and regression testing.

## Test Coverage

### 1. Toast Message System (`test_toast_messages.rs`)

Tests for the enhanced toast notification system:

- **Creation Tests**: Success, Error, Info, Warning message creation
- **Auto-dismiss**: Verification of 5-second auto-dismiss functionality
- **Persistent Messages**: Messages that don't auto-dismiss (e.g., long-running scans)
- **Type Safety**: Toast type equality and differentiation
- **String Conversion**: Support for `String`, `&str`, and `format!()` inputs
- **Cloning**: Toast message cloning for state management

**Coverage**: 10 unit tests

### 2. App State Management (`test_app_state.rs`)

Tests for core application state:

- **Initial State**: Default values on app creation
- **Screen Transitions**: Library, Queue, Plugins, Devices, DirectoryManager
- **Input Mode Transitions**: All valid mode changes
- **Toast Updates**: Message setting, updating, and dismissing
- **Search Functionality**: Search query management
- **File Input State**: APO and SOFA file input tracking
- **Directory Input**: Directory path input management
- **Autocomplete**: Suggestion management and clearing
- **Plugin Selection**: Plugin edit mode state
- **Scan Progress**: Library scan state tracking
- **Playback State**: Play/pause state management
- **Volume State**: Volume level management
- **Plugin Update Flags**: Change tracking for audio engine updates

**Coverage**: 18 unit tests

### 3. File Loading Integration (`test_file_loading.rs`)

Tests for APO and SOFA file loading:

#### APO File Loading (EQ Plugins):
- **Success Case**: Loading valid APO file into EQ plugin
- **Invalid File**: Error handling for non-existent files
- **Wrong Plugin Type**: Error when loading APO for non-EQ plugins
- **No Plugin Editing**: Error when no plugin is being edited
- **Filter Verification**: Ensure filters are correctly loaded from file

#### SOFA File Loading (Binaural Decoder):
- **Success Case**: Loading SOFA file path into Binaural Decoder
- **Wrong Plugin Type**: Error when loading SOFA for non-Binaural plugins
- **No Plugin Editing**: Error when no plugin is being edited
- **Path Verification**: Ensure SOFA file path is correctly set

#### General:
- **File Input Clearing**: Input field state management
- **Input Mode Transitions**: Proper mode changes during file loading

**Coverage**: 11 integration tests

### 4. Input Mode Transitions (`test_input_modes.rs`)

Tests for input mode state machine:

- **Default Mode**: Normal mode on startup
- **Search Mode**: Enter/exit search with query preservation
- **Add Directory Mode**: Directory input management
- **Edit Plugin Mode**: Plugin editing state
- **Load APO File Mode**: APO file input flow
- **Load SOFA File Mode**: SOFA file input flow
- **Help Mode**: Help modal display
- **Save/Load Plugins Modes**: Plugin preset management
- **Nested Transitions**: Multi-step mode changes
- **State Isolation**: Independent state for each mode
- **Screen + Mode Combinations**: Mode behavior across different screens
- **Mode Equality**: Input mode comparison
- **Toast Dismissal**: Toast clearing with ESC
- **Autocomplete Clearing**: Suggestion state reset

**Coverage**: 17 unit tests

### 5. User Scenario Tests (`test_scenarios.rs`)

High-level integration tests simulating complete user workflows:

#### Scenario 1: Library Setup and Scan
- Navigate to Directory Manager
- Add directory with text input
- Start library scan
- Verify toast messages at each step
- Return to Library screen

#### Scenario 2: Search and Filter Workflow
- Enter search mode
- Type search query
- Filter results
- Cycle channel filters
- Cycle sort orders
- Clear search and return to normal

#### Scenario 3: Plugin Chain Building and Editing
- Navigate to Plugins screen
- Add EQ plugin (Shift-1)
- Add Upmixer plugin (Shift-2)
- Select and edit EQ plugin
- Navigate parameters with arrows
- Load APO file from disk
- Verify plugin updates

#### Scenario 4: Binaural Decoder SOFA Loading
- Add Binaural Decoder plugin
- Enter edit mode
- Trigger SOFA file input (press 'f')
- Enter SOFA file path
- Verify file path is set
- Exit edit mode cleanly

#### Scenario 5: Error Recovery - Invalid APO File
- Try to load non-existent APO file
- See error toast
- Dismiss error
- Retry with valid file
- Verify successful recovery

#### Scenario 6: Error Recovery - Wrong Plugin Type
- Try to load APO file for Compressor plugin
- Get appropriate error message
- See warning toast
- Cancel operation cleanly

#### Scenario 7: Multi-Screen Navigation
- Start on Library with search active
- Navigate through Queue, Plugins, Devices, Directory Manager
- Verify all state is preserved across screens
- Return to Library with state intact

#### Scenario 8: Complete Plugin Chain Workflow
- Build 3-plugin chain (EQ → Compressor → Limiter)
- Edit middle plugin parameters
- Adjust multiple parameter values
- Disable middle plugin
- Verify entire chain state

#### Scenario 9: Toast Message Lifecycle
- Trigger success toast (add directory)
- Dismiss toast
- Trigger error toast (scan failed)
- Dismiss error
- Trigger persistent info toast (scanning)
- Verify persistent toast doesn't auto-dismiss
- Manual dismiss

#### Scenario 10: Help Modal Usage
- Open help from Library screen
- Close help
- Switch to Plugins screen
- Open help again from Plugins
- Verify help works on all screens

**Coverage**: 10 scenario tests covering end-to-end workflows

## Test Fixtures

### `fixtures/test_eq.txt`

Sample APO file for EQ testing containing:
- Preamp setting (-6.0 dB)
- Peak filters (100 Hz, 1000 Hz)
- Low shelf (50 Hz)
- High shelf (8000 Hz)

## Running Tests

```bash
# Run all GPUI tests
cargo test -p sotf-audio-player-gpui

# Run specific test file
cargo test -p sotf-audio-player-gpui --test test_toast_messages
cargo test -p sotf-audio-player-gpui --test test_scenarios

# Run with output
cargo test -p sotf-audio-player-gpui -- --nocapture

# Run specific test
cargo test -p sotf-audio-player-gpui test_toast_message_success_creation
cargo test -p sotf-audio-player-gpui scenario_build_and_edit_plugin_chain

# Run only scenario tests
cargo test -p sotf-audio-player-gpui --test test_scenarios

# Run only unit tests
cargo test -p sotf-audio-player-gpui --test test_toast_messages
cargo test -p sotf-audio-player-gpui --test test_app_state
cargo test -p sotf-audio-player-gpui --test test_input_modes
```

## Test Organization

Tests follow the standard Rust testing pattern:
- `/tests/` directory for integration tests
- `/tests/fixtures/` for test data files
- Test modules are separate files for clarity
- Each test is focused on a single behavior

## Coverage Summary

- **Total Tests**: 66 tests
- **Unit Tests**: 45
  - Toast messages: 10
  - App state: 18
  - Input modes: 17
- **Integration Tests**: 11
  - File loading workflows
- **Scenario Tests**: 10
  - End-to-end user workflows
- **Test Fixtures**: 1 APO file

## Test Categories

### Unit Tests (45 tests)
Fast, isolated tests for individual components and functions.

### Integration Tests (11 tests)
Tests for interactions between components (file loading, plugin updates).

### Scenario Tests (10 tests)
High-level workflow tests simulating real user interactions across multiple screens and features.

## CI/CD Integration

These tests should be run as part of:
1. Pre-commit hooks
2. Pull request checks
3. Release verification
4. Regression testing after updates

## Notes

- Tests use `parking_lot::Mutex` for thread-safe Player access
- App state tests create isolated instances for each test
- File loading tests verify both success and error paths
- Input mode tests ensure proper state machine behavior
- **Scenario tests** simulate complete user workflows step-by-step
- **Error recovery scenarios** ensure graceful handling of invalid inputs
- **State preservation scenarios** verify data persists across screen changes
- All tests are deterministic and don't require external resources
- Tests cover both happy paths and error cases
- Toast message lifecycle is validated across all scenarios
