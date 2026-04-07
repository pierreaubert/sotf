# gpui-keybinding

Reusable keybinding framework with Vim/Emacs/VSCode presets for GPUI applications.

## What It Does

Provides a structured way to define, register, and manage keyboard shortcuts in GPUI applications. Ships with built-in presets (Default, Vim, Emacs, VSCode) so users can choose their preferred key mapping style. Includes conflict detection to catch duplicate bindings.

## Features

- **Multiple presets**: Default, Vim, Emacs, VSCode key mappings out of the box
- **Provider pattern**: Apps register keybinding providers; the registry collects them all
- **Conflict detection**: Automatically finds duplicate key+context bindings
- **Platform-aware formatting**: Shows Cmd on macOS, Ctrl on Windows/Linux
- **Documented bindings**: Each binding carries a human-readable description for help UI
- **Category system**: Organize bindings by category (Navigation, Editing, etc.)

## Usage

```rust
use gpui_keybinding::{
    KeymapPreset, KeybindingProvider, KeybindingRegistry,
    DocumentedKeybinding, KeybindingCategory,
};

// Implement the provider trait for your app
struct MyAppBindings;

impl KeybindingProvider for MyAppBindings {
    fn keybindings(&self, preset: KeymapPreset) -> Vec<DocumentedKeybinding> {
        // Return bindings based on the active preset
        vec![]
    }
}

// Register providers and query bindings
let mut registry = KeybindingRegistry::new();
registry.register(Box::new(MyAppBindings));
let bindings = registry.bindings_for(KeymapPreset::Default, None);
```

## Built-in Presets

| Preset | Style | Example Navigation |
|--------|-------|-------------------|
| Default | Standard shortcuts | Arrow keys, Cmd+Z |
| Vim | Modal editing | h/j/k/l, dd, yy |
| Emacs | Chord-based | C-f/C-b, C-n/C-p |
| VSCode | VS Code compatible | Cmd+Shift+P, Cmd+D |

## Architecture

```
src/
├── lib.rs       # Module exports and re-exports
├── preset.rs    # KeymapPreset enum
├── provider.rs  # KeybindingProvider trait, DocumentedKeybinding, KeybindingCategory
├── registry.rs  # KeybindingRegistry — collects and queries bindings
├── conflict.rs  # Conflict detection
├── platform.rs  # Platform-aware key label formatting
└── presets/     # Built-in preset definitions
    ├── default.rs
    ├── vim.rs
    ├── emacs.rs
    └── vscode.rs
```

## Testing

```bash
cargo test -p gpui-keybinding
```

## License

Part of the SOTF (Sound of the Future) project.
