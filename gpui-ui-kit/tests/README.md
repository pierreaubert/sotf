# gpui-ui-kit Test Suite

## Quick Start

```bash
# Run all tests
cargo test --lib --tests

# Run specific test suite
cargo test --test i18n_tests
cargo test --test component_tests

# Run single test
cargo test test_language_switching
```

## Test Suites

### 1. I18n Tests (`i18n_tests.rs`)

**Purpose**: Prevent regressions in translations and internationalization.

**What it tests**:
- ✅ All translation keys exist in all languages (English, French, German, Spanish, Japanese)
- ✅ No missing translations (catches "???" fallbacks)
- ✅ Language switching works correctly
- ✅ Menu and section translations complete
- ✅ Translation consistency (button labels are short, etc.)

**Example test case**:
```rust
#[test]
fn test_language_switching() {
    let mut state = I18nState::new();

    // Switch to French
    state.set_language(Language::French);
    assert_eq!(state.t(TranslationKey::AppTitle), "Vitrine du UI Kit GPUI");

    // Switch to Spanish
    state.set_language(Language::Spanish);
    assert_eq!(state.t(TranslationKey::AppTitle), "Galeria del UI Kit GPUI");
}
```

**When to update**:
- ➕ Adding new translation keys → Add test case
- 🌍 Adding new language → Test all keys exist
- ✏️ Changing translation → Tests ensure consistency

### 2. Component Tests (`component_tests.rs`)

**Purpose**: Verify component APIs work correctly and configuration is valid.

**What it tests**:
- ✅ Components can be created with all variants
- ✅ All size options work
- ✅ Configuration methods chain correctly
- ✅ Themes have correct properties
- ✅ Component state management

**Example test case**:
```rust
#[test]
fn test_button_configuration() {
    let button = Button::new("test", "Test")
        .variant(ButtonVariant::Primary)
        .size(ButtonSize::Lg)
        .disabled(true)
        .selected(true);
    // If this compiles and runs, the API works correctly
}
```

**When to update**:
- ➕ Adding new component → Add test suite
- 🔧 Changing API → Update affected tests
- 🐛 Fixing bug → Add regression test

## Test Statistics

- **Total Tests**: 25 tests
- **I18n Tests**: 11 tests
- **Component Tests**: 14 tests
- **Languages Covered**: 5 (English, French, German, Spanish, Japanese)
- **Components Covered**: Button, Badge, Select, Accordion, Theme

## Critical Test Scenarios

### Language Change Updates All Text

**Problem**: When user changes language in showcase menu, all text should update.

**How We Test**:
```rust
// Test that all translation keys exist in all languages
test_all_languages_have_menu_translations()
test_all_languages_have_section_translations()
test_all_translation_keys_have_entries()

// Test that language switching works
test_language_switching()
```

**What This Prevents**:
- Missing translations causing "???" to appear
- Menu text not updating when language changes
- Section content staying in old language

### Component Configuration Doesn't Break

**Problem**: Changes to one component can break others due to shared state or types.

**How We Test**:
```rust
// Test all variants exist and work
test_button_variants()
test_accordion_modes()
test_accordion_orientations()

// Test configuration chains correctly
test_button_configuration()
test_accordion_configuration()
```

**What This Prevents**:
- Breaking changes to component APIs
- Removed variants causing compilation errors
- Invalid configuration combinations

## Running Tests in Development

### Before Committing

```bash
# Run full test suite
cargo test --lib --tests

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy --all-targets -- -D warnings
```

### During Development

```bash
# Run tests in watch mode (requires cargo-watch)
cargo watch -x "test --lib --tests"

# Run specific test file
cargo test --test i18n_tests

# Run with verbose output
cargo test -- --nocapture
```

### In CI/CD

Tests run automatically on:
- Every push to main/master/develop
- Every pull request
- See `.github/workflows/test.yml`

## Test Coverage Goals

### Current Coverage ✅

- [x] All translation keys verified across all languages
- [x] Component creation and configuration
- [x] Basic state management
- [x] Theme system

### Future Coverage 🚧

- [ ] Full user interaction flows (requires UI testing framework)
- [ ] Component event handling
- [ ] Animation behavior
- [ ] Accessibility features
- [ ] Performance benchmarks

## Adding New Tests

### When to Add Tests

1. **New Translation Key**:
   ```rust
   // Add to i18n_tests.rs
   #[test]
   fn test_new_translation_key() {
       let translations = Translations::new();
       for lang in Language::all() {
           let text = translations.get(*lang, TranslationKey::NewKey);
           assert_ne!(text, "???");
       }
   }
   ```

2. **New Component**:
   ```rust
   // Add to component_tests.rs
   #[test]
   fn test_new_component_creation() {
       let component = NewComponent::new("id")
           .variant(Variant::Primary)
           .size(Size::Large);
       drop(component); // Verifies it compiles and creates
   }
   ```

3. **Bug Fix**:
   ```rust
   // Add regression test
   #[test]
   fn test_bug_123_fixed() {
       // Setup that reproduces the bug
       // Assert that bug is fixed
   }
   ```

## Troubleshooting

### Tests Fail After Changes

1. **Read the error message** - it tells you exactly what failed
2. **Run single test** - `cargo test test_name -- --nocapture`
3. **Check what you changed** - did you add/remove translation keys?
4. **Fix root cause** - don't just make tests pass

### Common Failures

**Missing Translation**:
```
Language French missing translation for SectionNewFeature
```
→ Add translation in `src/i18n.rs` `Translations::add_french()`

**Component API Changed**:
```
no method named `loading` found for struct `Button`
```
→ Update test to use correct API or add missing method

**Type Mismatch**:
```
expected ButtonSize::Large, found ButtonSize::Lg
```
→ Use correct enum variant name

## Best Practices

### DO ✅

- Write tests for every new component
- Test all variants and configurations
- Run tests before committing
- Keep tests simple and focused
- Use descriptive test names

### DON'T ❌

- Skip tests for "simple" changes
- Test private implementation details
- Write overly complex tests
- Ignore test failures
- Remove tests without good reason

## Documentation

For detailed testing guide, see [`TESTING.md`](../TESTING.md) in the repository root.

## Summary

**Tests prevent regressions** by automatically verifying:
- ✅ All translations exist in all languages
- ✅ Components work correctly
- ✅ State management functions properly
- ✅ Changes don't break existing functionality

Run `cargo test --lib --tests` before every commit to catch issues early!
