# GPUI-MD Editor: Architecture & Emacs-Like Features

## Repository Location
- **Crate Path**: `/Users/pierrre/src.local/sotf/crates/gpui-toolkit/gpui-md/`
- **Binary**: `src/bin/gpui_md_editor.rs`
- **Main State**: `src/state.rs` (MdAppState)

---

## Current Emacs-Like Features

### 1. **Incremental Search (Isearch)**
- **State**: `IsearchState` (in `state.rs`, lines 17-36)
- **Fields**: `active`, `direction` (Forward/Backward), `query`, `start_position`, `current_match_start`
- **Keybindings**: 
  - `C-s` → IsearchForward
  - `C-r` → IsearchBackward
  - `C-s/r` while active → next/prev match
  - `C-g` → abort
  - `Enter`/`Escape` → exit
- **Implementation**: Editor pane key handler intercepts (lines 379-402 in editor_pane.rs)
- **Commands**: `isearch_start()`, `isearch_next()`, `isearch_prev()`, `isearch_exit()`, `isearch_add_char()`, `isearch_backspace()`

### 2. **Command Palette (M-x)**
- **State**: `CommandPaletteState` (lines 38-78 in state.rs)
- **Fields**: `visible`, `query`, `commands` (vec), `filtered_indices`, `selected`, `mode`
- **Mode Types**: `Command`, `GotoLine`
- **Keybinding**: `M-x` (Alt+X)
- **Available Commands** (lines 81-99):
  - `goto-line` — Jump to line
  - `load-theme` — Switch theme
  - `dired` — Open file picker (placeholder)
  - `find`/`replace` — Search operations
  - `toggle-preview` — Toggle preview pane
  - `toggle-line-numbers` — Show/hide line numbers
  - `select-all`, `undo`, `redo`
  - `upcase-word`, `downcase-word`
  - `exchange-point-and-mark`
  - `transpose-chars`, `transpose-words`
  - `zap-to-char`
- **Navigation**: Up/Down, Ctrl+N/P, filtered search
- **Implementation**: Editor pane intercepts (lines 404-450 in editor_pane.rs)

### 3. **Universal Argument (C-u)**
- **State**: `universal_arg: Option<usize>`, `universal_arg_accumulating: bool`
- **Keybinding**: `C-u`
- **Behavior**: Prefix argument for numeric repetition (e.g., `C-u 5 C-f` = forward 5 chars)
- **Digit Accumulation**: After C-u, typing digits appends to the number
- **Commands Affected**:
  - `C-f`/`C-b` — forward/backward by N chars
  - `C-n`/`C-p` — forward/backward by N lines
  - `M-f`/`M-b` — forward/backward by N words
  - `M-u`/`M-l` — upcase/downcase N words
- **Implementation**: Lines 452-466 in editor_pane.rs

### 4. **Kill Ring**
- **Module**: `document/kill_ring.rs` (full Emacs-style implementation)
- **Features**:
  - `MAX_RING_SIZE = 60` entries
  - Consecutive kills append to the same ring entry
  - Yank/yank-pop cycling (Alt+Y)
  - Separate `rectangle_buffer` for column operations
  - Flags: `last_was_kill`, `last_was_yank`, `last_yank_range`
- **Key Operations**:
  - `C-k` — kill to end of line
  - `C-w` — kill region
  - `M-w` — copy region (safe for all presets)
  - `M-d` — kill word forward
  - `M-Backspace` — kill word backward
  - `C-y` — yank (paste most recent)
  - `M-y` — yank-pop (cycle through ring)
- **Methods**: `push()`, `push_prepend()`, `yank()`, `yank_pop()`, `mark_yank()`, `clear_kill_flag()`, `reset_flags()`
- **Rectangle Operations** (C-x r <key>):
  - `C-x r k` — kill rectangle
  - `C-x r y` — yank rectangle
  - `C-x r d` — delete rectangle

### 5. **C-x Chord Prefix**
- **State**: `c_x_pending: bool`, `c_x_r_pending: bool`
- **Keybindings** (lines 108-112 in keybindings.rs):
  - `C-x C-s` → SaveFile
  - `C-x C-f` → OpenFile
  - `C-x C-x` → ExchangePointAndMark
  - `C-x C-c` → Quit
  - `C-x r <key>` → Rectangle operations (sub-prefix)
- **Implementation**: Two-key sequence handling in editor_pane (lines 574-587)

### 6. **Mark & Selection**
- **Cursor Structure**: `EditorCursor` (document/cursor.rs, lines 1-69)
  - `position: usize` — current cursor position
  - `anchor: Option<usize>` — selection anchor
  - `preferred_column: usize` — for vertical movement
- **Operations**:
  - `C-Space` → set_mark()
  - `C-x C-x` → exchange_point_and_mark()
  - Shift+arrows → extend selection
- **Selection Methods**: `has_selection()`, `selection()`, `start_selection()`, `clear_selection()`, `move_to(pos, extend)`

### 7. **Zap to Char (M-z)**
- **State**: `zap_to_char_pending: bool`
- **Keybinding**: `M-z`
- **Behavior**: Prompts for a character, then deletes from cursor to (and including) that character
- **Implementation**: `zap_to_char_start()` sets flag, next char input calls `zap_to_char(char)`

### 8. **Text Transformation Commands**
- `upcase-word` (M-u) — uppercase next word
- `downcase-word` (M-l) — lowercase next word
- `transpose-chars` (C-t) — swap adjacent characters
- `transpose-words` (M-t) — swap adjacent words

### 9. **Navigation (Emacs-style)**
- `C-a` → line start
- `C-e` → line end
- `C-f` → forward char (with universal arg)
- `C-b` → backward char (with universal arg)
- `C-p` → previous line (with universal arg)
- `C-n` → next line (with universal arg)
- `M-f` → forward word (with universal arg)
- `M-b` → backward word (with universal arg)
- `M-v` → page up
- `C-v` → page down
- `M-<` → document start
- `M->` → document end
- `C-l` → recenter (center cursor line in viewport)

### 10. **C-g (Abort)**
- **Keybinding**: `C-g`
- **Behavior**: Exit isearch, command palette, clear universal argument, cancel zap-to-char pending
- **Implementation**: `abort()` method clears modal state

---

## Architecture Overview

### Top-Level State: MdAppState
**File**: `src/state.rs` (lines 103-143)

**Core Fields**:
```rust
pub document: DocumentBuffer,         // Ropey-backed text buffer
pub history: EditHistory,             // Undo/redo stack
pub cursor: EditorCursor,             // Cursor position + selection
pub kill_ring: KillRing,              // Emacs-style kill ring
pub keymap_preset: KeymapPreset,      // Default/Emacs/Vim
pub universal_arg: Option<usize>,     // C-u prefix arg
pub isearch: IsearchState,            // C-s / C-r search state
pub command_palette: CommandPaletteState,  // M-x state
pub c_x_pending: bool,                // C-x chord awaiting second key
pub c_x_r_pending: bool,              // C-x r sub-prefix
pub zap_to_char_pending: bool,        // M-z awaiting target char
```

**Document Fields**:
```rust
pub show_preview: bool,               // Toggle live preview
pub show_line_numbers: bool,          // Gutter toggle
pub font_size: f32,                   // Editor font size
pub split_ratio: f32,                 // Editor/preview split
pub find_query: String,               // Find/replace bar text
pub replace_text: String,
pub find_bar_visible: bool,
pub replace_bar_visible: bool,
pub recent_files: Vec<PathBuf>,       // MRU file list
```

**Layout Fields**:
```rust
pub source_map: SourceMap,            // Preview → editor line mapping
pub editor_scroll_line: usize,        // Scroll offset tracking
pub last_parsed_version: u64,         // For preview regeneration
```

---

### Document Buffer: DocumentBuffer
**File**: `src/document/buffer.rs` (lines 1-143)

**Structure**:
```rust
pub struct DocumentBuffer {
    rope: Rope,                       // Ropey text storage
    file_path: Option<PathBuf>,       // Current file path
    dirty: bool,                      // Unsaved changes flag
    version: u64,                     // Incremented on every change
}
```

**Key Methods**:
- `new()`, `from_text()`, `from_file()`
- `text()` → full text as String
- `rope()` → immutable reference to Rope
- `len_chars()`, `len_lines()`
- `line(idx)`, `char_to_line()`, `line_to_char()`
- `insert()`, `remove()`, `set_text()`, `replace_content()`
- `snapshot()`, `restore()` — for undo/redo
- `file_path()`, `set_file_path()`, `is_dirty()`, `mark_clean()`

**Notes**:
- Backed by `ropey::Rope` for efficient large-file editing
- Dirty flag set on insert/remove but NOT on `set_text()` (file load)
- Version increments track changes for preview regeneration

---

### Undo/Redo: EditHistory
**File**: `src/document/history.rs` (lines 1-66)

**Structure**:
```rust
pub struct EditHistory {
    undo_stack: VecDeque<(Rope, usize)>,   // (document_snapshot, cursor_pos)
    redo_stack: VecDeque<(Rope, usize)>,
    max_history: usize,                    // Default 100
}
```

**Key Methods**:
- `push_undo(snapshot, cursor_pos)` — save before edit, clear redo stack
- `undo(current_doc, cursor)` → `Option<(Rope, usize)>` — restore and push to redo
- `redo(current_doc, cursor)` → `Option<(Rope, usize)>` — restore and push to undo
- `can_undo()`, `can_redo()`

---

### Kill Ring: KillRing
**File**: `src/document/kill_ring.rs` (lines 1-139)

**Structure**:
```rust
pub struct KillRing {
    ring: VecDeque<String>,                // MAX_RING_SIZE = 60
    yank_index: usize,                     // For yank-pop cycling
    last_was_kill: bool,                   // Consecutive kill flag
    last_was_yank: bool,                   // Yank-pop availability flag
    last_yank_range: Option<(usize, usize)>,  // Range of last yank
    pub rectangle_buffer: Option<Vec<String>>,  // Column operations
}
```

**Key Methods**:
- `push(text)` — append if last op was kill, else new entry
- `push_prepend(text)` — prepend if last op was kill (backward kill)
- `yank()` → `Option<&str>` — most recent or current yank index
- `yank_pop()` → cycle to next ring entry
- `mark_yank(start, end)` — record yank range for pop replacement
- `clear_kill_flag()`, `clear_yank_flag()`, `reset_flags()`

---

### Cursor: EditorCursor
**File**: `src/document/cursor.rs` (lines 1-69)

**Structure**:
```rust
pub struct EditorCursor {
    pub position: usize,              // Current cursor char offset
    pub anchor: Option<usize>,        // Selection start (None = no selection)
    pub preferred_column: usize,      // Preserved on vertical moves
}
```

**Key Methods**:
- `at(position)` — constructor
- `selection()` → `Option<(start, end)>`
- `has_selection()`, `clear_selection()`, `start_selection()`
- `move_to(position, extend_selection)`

---

## View Architecture

### MainView (root)
**File**: `src/views/main_view.rs` (lines 11-197)

**Structure**:
```rust
pub struct MainView {
    state: Entity<MdAppState>,
    editor: Entity<EditorPane>,       // Text editing pane
    preview: Entity<PreviewPane>,     // Live markdown preview
    toolbar: Entity<ToolbarView>,     // Formatting toolbar
    find_bar: Entity<FindBar>,        // Find/replace bar
    last_editor_scroll_y: f32,
    last_preview_scroll_y: f32,
}
```

**Responsibilities**:
- Layout: toolbar + find_bar + split pane (editor | preview) + status bar
- Scroll sync: when editor scrolls, proportionally sync preview (and vice versa)
- Title bar: displays filename + dirty marker
- Status bar: shows line number, word count, keymap name, universal arg, isearch/palette state

---

### EditorPane
**File**: `src/views/editor_pane.rs` (lines 27-993)

**Structure**:
```rust
pub struct EditorPane {
    state: Entity<MdAppState>,
    pub scroll_handle: ScrollHandle,   // Scroll position tracking
    focus_handle: FocusHandle,         // Focus management
    viewport_height: Rc<Cell<f32>>,    // For page-jump calculation
}
```

**Rendering**:
- Virtualized rendering: renders only visible lines + overscan
- Line height: 20px (estimated for scroll calculations)
- Gutter: line numbers (if enabled)
- Syntax highlighting: GFM code blocks, inline code, emphasis
- Cursor: block cursor on cursor line
- Selection: background highlight on selected text
- Find highlights: overlay on matching text

**Key Handler**: Lines 366-776
- **Priority Order**:
  1. Isearch intercept (when active)
  2. Command palette intercept (when visible)
  3. Universal arg digit accumulation
  4. Special Emacs keys (C-v, C-l, C-z, C-u, C-x, etc.)
  5. Clipboard operations (C-c, C-x, C-v)
  6. Regular editing (text input, navigation, deletion)

---

### FindBar
**File**: `src/views/find_bar.rs` (lines 1-165)

**Structure**:
```rust
pub struct FindBar {
    state: Entity<MdAppState>,
}
```

**Rendering**:
- Find row: input + match count + "Next" + "Close" buttons
- Optional replace row (when `replace_bar_visible`): input + "Replace" + "All" buttons
- State-driven visibility via `find_bar_visible`, `replace_bar_visible`

---

### CommandPalette (embedded in EditorPane status)
- Rendered in editor pane as overlay (lines 808-835 in editor_pane.rs)
- Displays prompt + filtered command list (top 10)
- Selected item highlighted
- No separate view component (integrated into editor pane rendering)

---

## File Opening / Window Management

**Current Approach** (src/bin/gpui_md_editor.rs):
- **CLI**: One window per file (lines 14-22)
  - `gpui-md-editor file1.md file2.md` → two separate windows
- **Each window**:
  - Unique `MdAppState` instance (independent documents/cursors)
  - Global `MdGlobalState` per window for action dispatch
  - Open file dialog: `rfd::AsyncFileDialog` → new file loads into CURRENT window (replaces content)

**Limitations**:
- No multi-buffer support within a single window
- No buffer switching (C-x C-b equivalent)
- No buffer list
- File opening always replaces current document
- Each window is isolated (no IPC between them)

---

## Keybindings

**File**: `src/keybindings.rs` (lines 1-132)

**Provider**: `MdKeybindingProvider` implements `KeybindingProvider`

**Preset Bindings**:

**Common** (all presets):
- `Cmd+N` → NewFile
- `Cmd+O` → OpenFile
- `Cmd+S` → SaveFile
- `Cmd+Shift+S` → SaveFileAs
- `Cmd+Z` → Undo
- `Cmd+Shift+Z` → Redo
- `Cmd+C/X/V` → Copy/Cut/Paste
- `Cmd+B/I/E` → Bold/Italic/Code
- `Cmd+Shift+X` → Strikethrough
- `Cmd+K` → InsertLink
- `Cmd+F` → Find
- `Cmd+H` → FindReplace
- `Cmd+=/-` → Font size

**Emacs Preset** (lines 108-128):
- `C-g` → Abort
- `C-l` → Recenter
- `C-u` → UniversalArgument
- `C-s` → IsearchForward
- `C-r` → IsearchBackward
- `C-v` → PageDown
- `M-x` → CommandPalette
- `M-u` → UpcaseWord
- `M-l` → DowncaseWord
- `M-b` → WordLeft
- `M-f` → WordRight
- `M-v` → PageUp
- Chord sequences: `C-x C-s/f/x/c`

**Vim Preset**:
- `g 1/2/3` → Insert headings (leader-based)

---

## State Management Pattern

**Global State Access**:
```rust
// In action handlers (src/bin/gpui_md_editor.rs, lines 109+)
let state = cx.global::<MdGlobalState>().0.clone();
state.update(cx, |s, _cx| {
    // Mutate state here
});
```

**Reactive Updates**:
- `EditorPane` observes `MdAppState` (lines 40-43 in editor_pane.rs)
- Any mutation triggers re-render: `cx.notify()`

**Action System**:
- Actions defined in `src/actions.rs` (61 action types)
- Handlers registered in `register_actions()` (src/bin/gpui_md_editor.rs, lines 109-512)
- Keyboard events dispatch to these handlers

---

## Data Flow for Features Not Yet Present

### What DOESN'T Exist (Planning Points)

1. **Multi-Buffer Management**
   - Currently: one `DocumentBuffer` per window
   - Needed: `Vec<DocumentBuffer>` or separate buffer entities
   - Switching: no equivalent to `C-x C-b` buffer list

2. **Mini-Buffer Input**
   - Currently: command palette is popup, isearch is inline
   - Needed: dedicated mini-buffer area for user input prompts
   - Could integrate into status bar or separate component

3. **Dired (File Browser)**
   - Currently: stub command in palette (line 84 in state.rs)
   - Needed: file tree view, directory navigation, file operations

4. **Keyboard Macros**
   - Currently: no recording/playback infrastructure
   - Needed: event recording system, macro history, playback engine

5. **Emacs Lisp Support**
   - Currently: hard-coded Rust actions only
   - Needed: embedded scripting language, binding system, eval loop

---

## Naming Conventions

**State Methods**:
- `toggle_*()` — boolean toggle
- `*_start()` — enter mode (isearch, zap-to-char)
- `*_add_char()`, `*_backspace()` — text manipulation during mode
- `*_next()`, `*_prev()` — navigation in search results
- `*_exit()`, `*_abort()` — exit mode

**Commands** (in `actions.rs`):
- PascalCase: `NewFile`, `UpcaseWord`, `IsearchForward`
- Verb+Object: `ToggleBold`, `InsertLink`, `ExchangePointAndMark`

**Files**:
- `state.rs` — App state + mutation methods
- `document/*.rs` — Buffer, history, cursor, kill ring
- `views/*.rs` — GPUI components
- `keybindings.rs` — Key event mapping
- `actions.rs` — Action definitions (not handlers)
- `markdown/*.rs` — Parsing, rendering, syntax highlight
- `export/`, `import/` — Format conversions

---

## Key Implementation Details

### Virtualization
- Renders lines based on scroll offset (estimated 20px per line)
- Overscan: ±10 lines outside viewport for smooth scrolling
- Spacer divs above/below rendered range for correct scroll geometry

### Rope Text Storage
- Efficient for large files (100+ MB tested)
- Copy-on-write semantics for snapshots
- Supports efficient line-based access
- UTF-8 safe char/line indexing

### Click-to-Locate
- Preview pane renders markdown with source map
- Clicking in preview jumps to corresponding editor line
- Bidirectional scroll sync (proportional based on document height)

### Modal Stacking
- Isearch takes priority over command palette
- Either can exit via `C-g` or `Escape`
- Universal arg can accumulate while other modes are active (independent)

---

## Summary: What Exists, What's Needed

**Exists Now** (Emacs foundations):
- Kill ring with yank/yank-pop
- Universal argument (C-u)
- Isearch (C-s / C-r)
- Command palette (M-x) — with stub commands
- C-x chord prefix system
- Mark and selection
- Text transformation (upcase, downcase, transpose)
- Zap-to-char (M-z)
- Full cursor model (position, anchor, selection)
- Undo/redo with cursor restoration

**Needs Building** (in priority order):
1. **Multi-buffer infrastructure** — Vec<Buffer>, switching, buffer list (C-x C-b)
2. **Mini-buffer** — dedicated user input area (not just overlays)
3. **Dired** — file browser, directory operations
4. **Keyboard macro recording/playback** — event capture and replay
5. **Emacs Lisp substrate** — interpreter, function binding, eval loop
6. **Buffer-local settings** — per-buffer modes, flags, keymaps
7. **Window management** — split windows, window switching (C-x o)
8. **Completion framework** — for minibuffer inputs
9. **Help system** — describe commands, key bindings

---

## File Reference Map

| File | Lines | Purpose |
|------|-------|---------|
| `src/state.rs` | 1-1500+ | MdAppState, all commands & queries |
| `src/document/buffer.rs` | 1-143 | DocumentBuffer (Rope-backed) |
| `src/document/kill_ring.rs` | 1-139 | KillRing (Emacs-style) |
| `src/document/history.rs` | 1-66 | EditHistory (undo/redo) |
| `src/document/cursor.rs` | 1-69 | EditorCursor (position + selection) |
| `src/views/editor_pane.rs` | 1-993 | Main editor UI + key handler |
| `src/views/main_view.rs` | 1-197 | Root layout + scroll sync |
| `src/views/find_bar.rs` | 1-165 | Find/replace UI |
| `src/views/command_palette.rs` | 1-100+ | Command palette (embedded) |
| `src/keybindings.rs` | 1-132 | Key binding definitions |
| `src/actions.rs` | 1-64 | Action enum declarations |
| `src/bin/gpui_md_editor.rs` | 1-618 | Binary entry, action handlers, menus |
