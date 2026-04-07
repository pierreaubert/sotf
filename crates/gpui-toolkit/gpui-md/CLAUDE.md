# gpui-md

Markdown editor with live preview for GPUI applications.

## Architecture

Full-featured editor demonstrating the gpui-toolkit ecosystem. Split into document model, markdown processing, import/export, and views.

- `document/` — Ropey-backed document buffer:
  - `buffer.rs` — `DocumentBuffer`: text storage, insert/delete, line operations
  - `cursor.rs` — Cursor position and selection
  - `history.rs` — Undo/redo stack
  - `kill_ring.rs` — Emacs-style kill ring
- `markdown/` — Markdown processing:
  - `parser.rs` — GFM parsing via comrak
  - `renderer.rs` — GPUI element rendering from AST
  - `source_map.rs` — Click-to-locate (preview position → editor line)
  - `syntax_highlight.rs` — Code block syntax highlighting
  - `text_layout.rs` — Text layout for rendered markdown
  - `theme_colors.rs` — Markdown-specific theme colors
- `export/` — Export formats:
  - `docx.rs` — Word (.docx) export
  - `pdf.rs` — PDF export (behind `pdf-export` feature)
- `import/` — Import formats:
  - `docx.rs` — Word (.docx) import
- `views/` — GPUI views:
  - `main_view.rs` — `MainView`: top-level editor view
  - `editor_pane.rs` — Text editing pane
  - `preview_pane.rs` — Live markdown preview
  - `toolbar_view.rs` — Toolbar with actions
  - `find_bar.rs` — Find/replace bar
- `actions.rs` — GPUI action definitions
- `keybindings.rs` — `MdKeybindingProvider`: keyboard shortcuts via gpui-keybinding
- `state.rs` — `MdAppState`: application state management

## Key Public API

- `MainView` — top-level GPUI view for the editor (`views/main_view.rs`)
- `MdAppState` — application state (`state.rs`)
- `MdKeybindingProvider` — keybinding provider for gpui-keybinding integration (`keybindings.rs`)

## Features

- `pdf-export` — enables PDF export via genpdf
- `google-docs` — enables Google Docs integration via reqwest

## Binaries

- `gpui-md-editor` — standalone markdown editor binary

## Testing

```bash
cargo test -p gpui-md
cargo check -p gpui-md --features pdf-export
```

## Important Notes

- Uses comrak for GFM parsing (tables, task lists, strikethrough, autolinks)
- Document buffer backed by ropey for efficient large-file editing
- Source map enables click-to-locate: clicking in preview jumps to the corresponding editor line
- Depends on most gpui-toolkit crates (builder, design, keybinding, pretext, ui-kit)
