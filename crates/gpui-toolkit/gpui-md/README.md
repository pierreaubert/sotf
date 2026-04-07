# gpui-md

Markdown editor with live preview for GPUI applications.

## What It Does

A full-featured markdown editor built with the GPUI toolkit. Write markdown in a split-pane editor with a live preview that updates as you type. Supports GitHub Flavored Markdown, import/export to Word documents, and optional PDF export.

## Features

- **Split-pane editing**: Side-by-side editor and live preview
- **GitHub Flavored Markdown**: Tables, task lists, strikethrough, autolinks via comrak
- **Click-to-locate**: Click in preview to jump to the corresponding editor line
- **Undo/redo**: Full history with Emacs-style kill ring
- **Import/Export**: Word (.docx) import and export
- **PDF export**: Optional PDF generation (enable `pdf-export` feature)
- **Platform keybindings**: Vim/Emacs/VSCode presets via gpui-keybinding
- **Syntax highlighting**: Code block syntax coloring
- **Find/Replace**: Built-in search and replace bar
- **Theme support**: Respects the active GPUI theme

## Usage

### Standalone Editor

```bash
cargo run -p gpui-md --bin gpui-md-editor
```

### Embedding in Your App

```rust
use gpui_md::{MainView, MdAppState, MdKeybindingProvider};

// Register the keybinding provider
registry.register(Box::new(MdKeybindingProvider));

// Create the main view in your GPUI app
let view = cx.new(|cx| MainView::new(cx));
```

## Feature Flags

| Feature | Description | Default |
|---------|-------------|---------|
| `pdf-export` | PDF export via genpdf | No |
| `google-docs` | Google Docs integration via reqwest | No |

## Architecture

```
src/
├── lib.rs           # Public exports
├── state.rs         # MdAppState — application state
├── actions.rs       # GPUI action definitions
├── keybindings.rs   # Keyboard shortcut definitions
├── document/        # Document model
│   ├── buffer.rs    # Ropey-backed text buffer
│   ├── cursor.rs    # Cursor and selection
│   ├── history.rs   # Undo/redo stack
│   └── kill_ring.rs # Emacs-style kill ring
├── markdown/        # Markdown processing
│   ├── parser.rs    # GFM parsing via comrak
│   ├── renderer.rs  # GPUI element rendering
│   ├── source_map.rs # Preview → editor position mapping
│   └── ...
├── views/           # GPUI views
│   ├── main_view.rs # Top-level view
│   ├── editor_pane.rs
│   ├── preview_pane.rs
│   ├── toolbar_view.rs
│   └── find_bar.rs
├── export/          # Word, PDF export
└── import/          # Word import
```

## Testing

```bash
cargo test -p gpui-md
cargo check -p gpui-md --features pdf-export
```

## License

Part of the SOTF (Sound of the Future) project.
