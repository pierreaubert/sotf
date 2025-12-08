# Testing Guide for gpui-ui-kit

## Overview

This document describes the testing strategy for the gpui-ui-kit library to prevent regressions and ensure quality.

## Test Structure

Tests are organized into the `tests/` directory:

```
tests/
├── i18n_tests.rs          # Translation and internationalization tests
├── component_tests.rs     # Component behavior and state tests
└── integration_tests.rs   # Full integration tests (future)
```

## Running Tests

### Run All Tests

```bash
cargo test
```

### Run Specific Test Suite

```bash
# Run only i18n tests
cargo test --test i18n_tests

# Run only component tests
cargo test --test component_tests
```

### Run Specific Test

```bash
cargo test test_language_switching
```

### Run Tests with Output

```bash
cargo test -- --nocapture
```

## Test Categories

### 1. I18n Tests (`tests/i18n_tests.rs`)

These tests verify that internationalization works correctly across all languages.

**What They Test:**
- All translation keys exist in all languages
- No missing translations (all should be non-"???")
- Language switching works correctly
- Fallback to English works when needed
- Translation consistency (e.g., button labels are short)

**Example Test Case:**
```rust
#[test]
fn test_language_switching() {
    let mut state = I18nState::new();

    // Default should be English
    assert_eq!(state.language, Language::English);
    assert_eq!(state.t(TranslationKey::AppTitle), "GPUI UI Kit Showcase");

    // Switch to French
    state.set_language(Language::French);
    assert_eq!(state.t(TranslationKey::AppTitle), "Vitrine du UI Kit GPUI");
}
```

**Critical Test: Language Change Updates All Text**

When the user changes the language in the showcase menu, all text in both the menu and content sections should update. This is tested implicitly by verifying:
1. All translation keys exist in all languages
2. The I18n state management works correctly
3. The showcase uses `cx.t()` for all displayed text

### 2. Component Tests (`tests/component_tests.rs`)

These tests verify component behavior and state management.

**What They Test:**
- Component creation with different variants
- Size variants work correctly
- State management (disabled, loading, selected)
- Component configuration options
- Theme application

**Example Test Case:**
```rust
#[test]
fn test_accordion_modes() {
    let single = Accordion::new().mode(AccordionMode::Single);
    assert_eq!(single.mode, AccordionMode::Single);

    let multiple = Accordion::new().mode(AccordionMode::Multiple);
    assert_eq!(multiple.mode, AccordionMode::Multiple);
}
```

### 3. Integration Tests (Future)

Future integration tests will verify:
- Full showcase application behavior
- User interaction flows
- Component composition
- Theme switching
- Language switching in full UI context

## Test Requirements

### For New Components

When adding a new component, you must:

1. **Add Component Tests** (`tests/component_tests.rs`)
   - Test all variants
   - Test all size options
   - Test state management
   - Test configuration options

2. **Add I18n Tests** (if component uses translations)
   - Add translation keys to `TranslationKey` enum
   - Add translations for all languages
   - Add test to verify translations exist

3. **Update This Documentation**
   - Document what tests cover
   - Add examples of test cases

### For New Translation Keys

When adding a new translation key:

1. Add to `TranslationKey` enum in `src/i18n.rs`
2. Add translation for ALL languages in `Translations::add_*` methods
3. Add test case in `tests/i18n_tests.rs` to verify all languages have the key
4. Run `cargo test --test i18n_tests` to verify

## Preventing Regressions

### The Problem We're Solving

The showcase application has many components and features:
- Multiple sections (Buttons, Accordion, Potentiometers, etc.)
- Multiple languages (English, French, German, Spanish, Japanese)
- Multiple themes (Dark, Light)
- Complex state management

**Without tests**, changes to one part can break another part, leading to:
- Circular fixes (fixing one thing breaks another)
- Missing translations
- Broken component states
- Inconsistent behavior

### How Tests Prevent This

1. **Translation Coverage Tests**
   - Ensure all languages have all translations
   - Catch missing translations before they reach users
   - Verify language switching updates everything

2. **Component State Tests**
   - Ensure components maintain correct state
   - Catch broken configuration options
   - Verify component variants work

3. **Continuous Verification**
   - Run `cargo test` before every commit
   - Tests run automatically in CI/CD
   - Quick feedback on breaking changes

## Testing Best Practices

### DO

✅ Write tests for every new component
✅ Test all variants and configurations
✅ Test state changes
✅ Run tests before committing
✅ Keep tests simple and focused
✅ Use descriptive test names
✅ Test both success and failure cases

### DON'T

❌ Skip tests for "simple" changes
❌ Test implementation details
❌ Write overly complex tests
❌ Ignore test failures
❌ Remove tests without good reason
❌ Test private implementation details

## Common Test Patterns

### Testing Component Creation

```rust
#[test]
fn test_component_creation() {
    let component = Component::new("id")
        .variant(Variant::Primary)
        .size(Size::Large)
        .disabled(false);

    assert_eq!(component.variant, Variant::Primary);
    assert_eq!(component.size, Size::Large);
    assert!(!component.disabled);
}
```

### Testing State Changes

```rust
#[test]
fn test_state_change() {
    let mut state = State::new();
    assert_eq!(state.value, default_value);

    state.update(new_value);
    assert_eq!(state.value, new_value);
}
```

### Testing Translations

```rust
#[test]
fn test_translation_exists() {
    let translations = Translations::new();

    for lang in Language::all() {
        let text = translations.get(*lang, TranslationKey::NewKey);
        assert_ne!(text, "???", "Missing translation for {:?}", lang);
    }
}
```

## Test Coverage Goals

### Current Coverage

- ✅ All translation keys tested across all languages
- ✅ Component creation and configuration
- ✅ State management basics
- ✅ Theme system

### Future Coverage

- 🚧 Full user interaction flows
- 🚧 Component composition
- 🚧 Event handling
- 🚧 Animation behavior
- 🚧 Accessibility features

## CI/CD Integration

Tests should run automatically in CI/CD:

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test
```

## Debugging Failed Tests

### When Tests Fail

1. **Read the error message carefully**
   - Tells you what failed and why
   - Shows expected vs actual values

2. **Run single test**
   ```bash
   cargo test test_name -- --nocapture
   ```

3. **Check recent changes**
   - Did you add a new translation key?
   - Did you change component behavior?
   - Did you modify state management?

4. **Fix the root cause**
   - Don't just make tests pass
   - Understand why they failed
   - Fix the underlying issue

### Common Issues

**Missing Translations**
```
Language French missing translation for SectionNewFeature
```
→ Add translation in `Translations::add_french()`

**Component State Mismatch**
```
assertion failed: component.disabled
```
→ Check component initialization or state update

**Type Mismatch**
```
expected Variant::Primary, found Variant::Secondary
```
→ Verify component configuration

## Test Maintenance

### Regular Tasks

- [ ] Run full test suite weekly
- [ ] Update tests when adding features
- [ ] Remove obsolete tests
- [ ] Review test coverage
- [ ] Update documentation

### When to Update Tests

- Adding new components → Add component tests
- Adding translation keys → Add i18n tests
- Changing behavior → Update affected tests
- Fixing bugs → Add regression test
- Refactoring → Ensure tests still pass

## Getting Help

If you have questions about testing:

1. Read this documentation
2. Look at existing test examples
3. Run tests to see what's expected
4. Ask for code review

## Summary

**The Goal**: Prevent regressions by automatically verifying that:
- All translations exist in all languages
- All components work correctly
- State management functions properly
- Changes don't break existing functionality

**The Method**: Write focused unit tests that verify specific behaviors and run them automatically before every commit.

**The Result**: Confidence that changes don't break existing functionality, reducing circular fixes and improving code quality.
