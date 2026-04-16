use std::collections::HashMap;
use std::path::{Path, PathBuf};
use gpui_keybinding::KeymapPreset;
use gpui_elisp::{Interpreter, add_primitives, EditorCallbacks};

use crate::commands::{CommandArgs, CommandRegistry, InteractiveSpec, register_builtin_commands};
use crate::dired::DiredState;
use crate::document::buffer_list::{BufferId, BufferKind, StoredBuffer, name_from_path, unique_name as unique_buffer_name};
use crate::document::kill_ring::KillRing;
use crate::document::{DocumentBuffer, EditHistory, EditorCursor};
use crate::macros::{MacroState, RecordedAction};
use crate::markdown::SourceMap;
use crate::minibuffer::{MiniBufferPrompt, MiniBufferResult, MiniBufferState};

struct ElispEditorCallbacks {
    state: *mut MdAppState,
}

unsafe impl Send for ElispEditorCallbacks {}
unsafe impl Sync for ElispEditorCallbacks {}

impl EditorCallbacks for ElispEditorCallbacks {
    fn buffer_string(&self) -> String {
        unsafe { (*self.state).document.text() }
    }

    fn buffer_size(&self) -> usize {
        unsafe { (*self.state).document.len_chars() }
    }

    fn point(&self) -> usize {
        unsafe { (*self.state).cursor.position }
    }

    fn insert(&mut self, text: &str) {
        unsafe { (*self.state).insert_text(text) }
    }

    fn delete_char(&mut self, n: i64) {
        unsafe {
            if n > 0 {
                for _ in 0..n as usize {
                    (*self.state).delete_forward();
                }
            } else if n < 0 {
                for _ in 0..(-n) as usize {
                    (*self.state).backspace();
                }
            }
        }
    }

    fn goto_char(&mut self, pos: usize) {
        unsafe {
            (*self.state).cursor.position = pos;
            (*self.state).cursor.clear_selection();
        }
    }

    fn forward_char(&mut self, n: i64) {
        unsafe {
            if n > 0 {
                for _ in 0..n as usize {
                    (*self.state).move_right(false);
                }
            } else if n < 0 {
                for _ in 0..(-n) as usize {
                    (*self.state).move_left(false);
                }
            }
        }
    }

    fn find_file(&mut self, path: &str) -> bool {
        unsafe {
            let path_buf = std::path::PathBuf::from(path);
            if let Ok(content) = std::fs::read_to_string(&path_buf) {
                (*self.state).open_file_as_buffer(path_buf, &content);
                true
            } else {
                false
            }
        }
    }

    fn save_buffer(&mut self) -> bool {
        unsafe {
            if let Some(path) = (*self.state).document.file_path() {
                let content = (*self.state).document.text();
                std::fs::write(path, content).is_ok()
            } else {
                false
            }
        }
    }
}

// ---- Emacs subsystem types ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsearchDirection {
    Forward,
    Backward,
}

/// State for incremental search (C-s / C-r).
pub struct IsearchState {
    pub active: bool,
    pub direction: IsearchDirection,
    pub query: String,
    pub start_position: usize,
    pub current_match_start: Option<usize>,
}

impl Default for IsearchState {
    fn default() -> Self {
        Self {
            active: false,
            direction: IsearchDirection::Forward,
            query: String::new(),
            start_position: 0,
            current_match_start: None,
        }
    }
}

/// Deferred action bound to the currently-open mini-buffer prompt.
///
/// When a command like "open-file" needs an argument, it stores a variant
/// here and opens a mini-buffer prompt. On submit, the state machine looks
/// at this field to decide what to do with the submitted value.
#[derive(Clone, Debug)]
pub enum PendingMinibufferAction {
    /// Switch to the buffer whose name is typed.
    SwitchBuffer,
    /// Kill the buffer whose name is typed.
    KillBuffer,
    /// Open a file at the typed path.
    OpenFile,
    /// Goto the typed line number.
    GotoLine,
    /// M-x — run the command whose name is typed via the registry.
    RunCommand,
    /// Run a specific command with the typed string as its argument.
    RunCommandWithInput { name: String },
    /// YesNo confirmation for dired mark execution.
    DiredConfirmDelete,
}

/// Mode the command palette is operating in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    /// Normal command selection.
    Command,
    /// Waiting for a line number (after selecting goto-line).
    GotoLine,
    /// Switching to a buffer by name (C-x b).
    SwitchBuffer,
    /// Killing a buffer by name (C-x k).
    KillBuffer,
}

/// A command available in the M-x palette.
#[derive(Clone)]
pub struct PaletteCommand {
    pub name: String,
    pub description: String,
    pub id: &'static str,
}

/// State for the M-x command palette.
pub struct CommandPaletteState {
    pub visible: bool,
    pub query: String,
    pub commands: Vec<PaletteCommand>,
    pub filtered_indices: Vec<usize>,
    pub selected: usize,
    pub mode: PaletteMode,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        let commands = default_palette_commands();
        let filtered_indices: Vec<usize> = (0..commands.len()).collect();
        Self {
            visible: false,
            query: String::new(),
            commands,
            filtered_indices,
            selected: 0,
            mode: PaletteMode::Command,
        }
    }
}

fn default_palette_commands() -> Vec<PaletteCommand> {
    vec![
        PaletteCommand { name: "goto-line".into(), description: "Jump to line number".into(), id: "goto-line" },
        PaletteCommand { name: "load-theme".into(), description: "Switch editor theme".into(), id: "load-theme" },
        PaletteCommand { name: "dired".into(), description: "Open file picker".into(), id: "dired" },
        PaletteCommand { name: "find".into(), description: "Find text".into(), id: "find" },
        PaletteCommand { name: "replace".into(), description: "Find and replace".into(), id: "replace" },
        PaletteCommand { name: "toggle-preview".into(), description: "Toggle preview pane".into(), id: "toggle-preview" },
        PaletteCommand { name: "toggle-line-numbers".into(), description: "Toggle editor line numbers".into(), id: "toggle-line-numbers" },
        PaletteCommand { name: "toggle-preview-line-numbers".into(), description: "Toggle preview line numbers".into(), id: "toggle-preview-line-numbers" },
        PaletteCommand { name: "select-all".into(), description: "Select all text".into(), id: "select-all" },
        PaletteCommand { name: "undo".into(), description: "Undo last edit".into(), id: "undo" },
        PaletteCommand { name: "redo".into(), description: "Redo last edit".into(), id: "redo" },
        PaletteCommand { name: "upcase-word".into(), description: "Uppercase next word".into(), id: "upcase-word" },
        PaletteCommand { name: "downcase-word".into(), description: "Lowercase next word".into(), id: "downcase-word" },
        PaletteCommand { name: "exchange-point-and-mark".into(), description: "Swap cursor and mark".into(), id: "exchange-point-and-mark" },
        PaletteCommand { name: "transpose-chars".into(), description: "Transpose characters at cursor".into(), id: "transpose-chars" },
        PaletteCommand { name: "transpose-words".into(), description: "Transpose words at cursor".into(), id: "transpose-words" },
        PaletteCommand { name: "zap-to-char".into(), description: "Kill to next occurrence of a character".into(), id: "zap-to-char" },
    ]
}

/// Top-level application state for the markdown editor.
pub struct MdAppState {
    // ---- Active buffer data (lives directly on MdAppState to minimise
    // churn for the 346+ method bodies that access these fields). ----
    pub document: DocumentBuffer,
    pub history: EditHistory,
    pub cursor: EditorCursor,

    // ---- Buffer list ----
    /// Buffer id of the currently-active buffer (whose data is in
    /// `document`/`cursor`/`history` above).
    pub current_buffer_id: BufferId,
    /// Display name of the currently-active buffer.
    pub current_buffer_name: String,
    /// Kind of the currently-active buffer.
    pub current_buffer_kind: BufferKind,
    /// Whether the current buffer is read-only.
    pub current_buffer_read_only: bool,
    /// Inactive buffers.
    pub stored_buffers: Vec<StoredBuffer>,
    /// Monotonic counter for generating new BufferIds.
    next_buffer_id: u64,

    pub keymap_preset: KeymapPreset,
    pub split_ratio: f32,
    pub font_size: f32,
    pub show_line_numbers: bool,
    pub show_preview_line_numbers: bool,
    pub show_preview: bool,
    pub last_parsed_version: u64,
    pub source_map: SourceMap,
    pub editor_scroll_line: usize,
    pub find_query: String,
    pub replace_text: String,
    pub find_bar_visible: bool,
    pub replace_bar_visible: bool,
    pub recent_files: Vec<PathBuf>,
    pub kill_ring: KillRing,
    pub editing_block: Option<String>,
    pub editing_block_text: String,
    /// Tracks consecutive character insertion for undo coalescing.
    last_edit_was_char_insert: bool,
    /// Tracks whether the last cursor movement was vertical (for preferred_column).
    last_move_was_vertical: bool,
    /// Universal argument prefix (C-u).
    pub universal_arg: Option<usize>,
    /// True while accumulating digits after C-u.
    pub universal_arg_accumulating: bool,
    /// Incremental search state (C-s / C-r).
    pub isearch: IsearchState,
    /// Command palette state (M-x).
    pub command_palette: CommandPaletteState,
    /// Unified mini-buffer for modal prompts.
    pub minibuffer: MiniBufferState,
    /// Pending command name whose arguments are being collected via the mini-buffer.
    pub pending_minibuffer_action: Option<PendingMinibufferAction>,
    /// Dired state per buffer id (only for dired-kind buffers).
    pub dired_states: HashMap<BufferId, DiredState>,
    /// Keyboard macro state (recording + replay).
    pub macros: MacroState,
    /// Command registry — all user-visible commands.
    pub commands: CommandRegistry,
    /// Elisp interpreter for extension and configuration.
    pub elisp: Interpreter,
    /// When true, the next character input is consumed as the target for zap-to-char (M-z).
    pub zap_to_char_pending: bool,
    /// When true, the next keystroke is treated as if Alt/Meta were held.
    /// Set by pressing Escape as a standalone key (classic Emacs terminal
    /// convention: `Esc b` is equivalent to `M-b`).
    pub meta_pending: bool,
    /// Emacs C-x prefix: the next key is interpreted as the second key of a C-x chord.
    /// Some chords have a sub-prefix (e.g. C-x r → rectangle ops).
    pub c_x_pending: bool,
    /// Emacs C-x r prefix: the next key is a rectangle operation.
    pub c_x_r_pending: bool,
}

impl MdAppState {
    /// Create state initialized with file contents as the first (and only) buffer.
    pub fn from_file(path: std::path::PathBuf, content: &str) -> Self {
        let mut state = Self::new();
        state.current_buffer_name = name_from_path(&path);
        state.current_buffer_kind = BufferKind::File;
        state.document = DocumentBuffer::from_file(path, content);
        state
    }

    pub fn new() -> Self {
        let mut commands = CommandRegistry::new();
        register_builtin_commands(&mut commands);

        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        let mut state = Self {
            document: DocumentBuffer::from_text(
                "# Welcome to gpui-md\n\n\
                 Start typing your markdown here.\n\n\
                 ## Features\n\n\
                 - **Bold** and *italic* text\n\
                 - `Code` blocks\n\
                 - [Links](https://example.com)\n\
                 - Task lists\n\
                 \x20 - [x] Done\n\
                 \x20 - [ ] Todo\n\n\
                 ```rust\n\
                 fn main() {\n\
                 \x20   println!(\"Hello, markdown!\");\n\
                 }\n\
                 ```\n",
            ),
            history: EditHistory::default(),
            cursor: EditorCursor::new(),
            current_buffer_id: BufferId(0),
            current_buffer_name: "*scratch*".to_string(),
            current_buffer_kind: BufferKind::Scratch,
            current_buffer_read_only: false,
            stored_buffers: Vec::new(),
            next_buffer_id: 1,
            keymap_preset: KeymapPreset::Default,
            split_ratio: 0.5,
            font_size: 14.0,
            show_line_numbers: true,
            show_preview_line_numbers: false,
            show_preview: true,
            last_parsed_version: 0,
            source_map: SourceMap::new(),
            editor_scroll_line: 0,
            find_query: String::new(),
            replace_text: String::new(),
            find_bar_visible: false,
            replace_bar_visible: false,
            recent_files: Vec::new(),
            kill_ring: KillRing::new(),
            editing_block: None,
            editing_block_text: String::new(),
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
            universal_arg: None,
            universal_arg_accumulating: false,
            isearch: IsearchState::default(),
            command_palette: CommandPaletteState::default(),
            minibuffer: MiniBufferState::default(),
            pending_minibuffer_action: None,
            dired_states: HashMap::new(),
            macros: MacroState::default(),
            commands,
            elisp: interp,
            zap_to_char_pending: false,
            meta_pending: false,
            c_x_pending: false,
            c_x_r_pending: false,
        };

        let callbacks = Box::new(ElispEditorCallbacks {
            state: &mut state as *mut MdAppState,
        });
        state.elisp.set_editor(callbacks);

        Self::init_elisp(&mut state.elisp);

        state
    }

    fn init_elisp(interp: &mut Interpreter) {
        if let Some(home) = dirs::home_dir() {
            let init_path = home.join(".gpui-md.el");
            if init_path.exists() {
                match std::fs::read_to_string(&init_path) {
                    Ok(content) => {
                        match gpui_elisp::read_all(&content) {
                            Ok(exprs) => {
                                for expr in exprs {
                                    if let Err(e) = interp.eval(expr) {
                                        log::error!("Error in {}: {:?}", init_path.display(), e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse {}: {:?}", init_path.display(), e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read {}: {:?}", init_path.display(), e);
                    }
                }
            }
        }
    }

    // ---- Buffer list management ----

    fn allocate_buffer_id(&mut self) -> BufferId {
        let id = BufferId(self.next_buffer_id);
        self.next_buffer_id += 1;
        id
    }

    /// Collect all currently-existing buffer names (active + stored).
    fn all_buffer_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.stored_buffers.len() + 1);
        names.push(self.current_buffer_name.clone());
        for b in &self.stored_buffers {
            names.push(b.name.clone());
        }
        names
    }

    /// Pick a unique display name for a new buffer with the given base.
    fn pick_unique_name(&self, base: &str) -> String {
        let existing = self.all_buffer_names();
        let refs: Vec<&str> = existing.iter().map(|s| s.as_str()).collect();
        unique_buffer_name(base, &refs)
    }

    /// Create a new scratch buffer and return its id. Does not switch to it.
    pub fn create_scratch(&mut self) -> BufferId {
        let id = self.allocate_buffer_id();
        let mut buf = StoredBuffer::new_scratch(id);
        buf.name = self.pick_unique_name("*scratch*");
        self.stored_buffers.push(buf);
        id
    }

    /// Create a file-backed buffer. Returns the id. Does not switch to it.
    /// If a buffer already exists for the same canonical path, returns its id
    /// without creating a duplicate.
    pub fn create_file_buffer(&mut self, path: PathBuf, content: &str) -> BufferId {
        // Canonicalize for reliable deduping (handles "./foo" vs "foo" etc.)
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(id) = self.find_buffer_by_path(&canonical) {
            return id;
        }
        let id = self.allocate_buffer_id();
        let base = name_from_path(&canonical);
        let name = self.pick_unique_name(&base);
        let buf = StoredBuffer::new_file(id, name, canonical, content);
        self.stored_buffers.push(buf);
        id
    }

    /// Return the id of an existing buffer backed by `path`, or None.
    pub fn find_buffer_by_path(&self, path: &Path) -> Option<BufferId> {
        if let Some(p) = self.document.file_path()
            && p == path
        {
            return Some(self.current_buffer_id);
        }
        for b in &self.stored_buffers {
            if let Some(p) = b.document.file_path()
                && p == path
            {
                return Some(b.id);
            }
        }
        None
    }

    /// Return the id of the buffer whose display name matches `name`, or None.
    pub fn find_buffer_by_name(&self, name: &str) -> Option<BufferId> {
        if self.current_buffer_name == name {
            return Some(self.current_buffer_id);
        }
        for b in &self.stored_buffers {
            if b.name == name {
                return Some(b.id);
            }
        }
        None
    }

    /// List all buffer names, current buffer first.
    pub fn buffer_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.stored_buffers.len() + 1);
        names.push(self.current_buffer_name.clone());
        for b in &self.stored_buffers {
            names.push(b.name.clone());
        }
        names
    }

    /// Total number of buffers (active + stored).
    pub fn buffer_count(&self) -> usize {
        self.stored_buffers.len() + 1
    }

    /// Swap the active buffer with the given stored buffer index, making the
    /// stored buffer active. The previously-active buffer becomes stored.
    fn swap_active_with_stored(&mut self, stored_idx: usize) {
        // Take the incoming stored buffer out of the vec
        let incoming = self.stored_buffers.remove(stored_idx);

        // Build an outgoing StoredBuffer from the current active fields,
        // replacing each with the corresponding field from `incoming`.
        let outgoing = StoredBuffer {
            id: self.current_buffer_id,
            name: std::mem::replace(&mut self.current_buffer_name, incoming.name),
            document: std::mem::replace(&mut self.document, incoming.document),
            history: std::mem::replace(&mut self.history, incoming.history),
            cursor: std::mem::replace(&mut self.cursor, incoming.cursor),
            kind: std::mem::replace(&mut self.current_buffer_kind, incoming.kind),
            read_only: std::mem::replace(&mut self.current_buffer_read_only, incoming.read_only),
            scroll_line: std::mem::replace(&mut self.editor_scroll_line, incoming.scroll_line),
            last_edit_was_char_insert: std::mem::replace(
                &mut self.last_edit_was_char_insert,
                incoming.last_edit_was_char_insert,
            ),
            last_move_was_vertical: std::mem::replace(
                &mut self.last_move_was_vertical,
                incoming.last_move_was_vertical,
            ),
        };

        // Active buffer id becomes the incoming buffer's id.
        self.current_buffer_id = incoming.id;

        // Push the now-inactive previous buffer onto the stored list.
        self.stored_buffers.push(outgoing);

        // Invalidate parsed version so preview re-parses
        self.last_parsed_version = 0;
    }

    /// Switch to a buffer by its id. Returns true on success.
    pub fn switch_to_buffer_id(&mut self, id: BufferId) -> bool {
        if id == self.current_buffer_id {
            return true;
        }
        if let Some(idx) = self.stored_buffers.iter().position(|b| b.id == id) {
            self.swap_active_with_stored(idx);
            true
        } else {
            false
        }
    }

    /// Switch to a buffer by its display name. Returns true on success.
    pub fn switch_to_buffer_by_name(&mut self, name: &str) -> bool {
        if name == self.current_buffer_name {
            return true;
        }
        if let Some(idx) = self.stored_buffers.iter().position(|b| b.name == name) {
            self.swap_active_with_stored(idx);
            true
        } else {
            false
        }
    }

    /// Cycle to the next buffer in the list.
    pub fn switch_to_next_buffer(&mut self) {
        if self.stored_buffers.is_empty() {
            return;
        }
        self.swap_active_with_stored(0);
    }

    /// Cycle to the previous buffer in the list.
    pub fn switch_to_prev_buffer(&mut self) {
        if self.stored_buffers.is_empty() {
            return;
        }
        let last = self.stored_buffers.len() - 1;
        self.swap_active_with_stored(last);
    }

    /// Kill a buffer by id. Refuses to kill if it would leave zero buffers.
    /// If the killed buffer is the active one, switches to the first stored buffer.
    /// Returns true on success.
    pub fn kill_buffer_id(&mut self, id: BufferId) -> bool {
        if self.buffer_count() <= 1 {
            return false;
        }
        let removed = if id == self.current_buffer_id {
            // Swap in a stored buffer, then drop the outgoing one.
            self.swap_active_with_stored(0);
            // The outgoing is now at the end of stored_buffers.
            if let Some(pos) = self.stored_buffers.iter().position(|b| b.id == id) {
                self.stored_buffers.remove(pos);
                true
            } else {
                false
            }
        } else if let Some(pos) = self.stored_buffers.iter().position(|b| b.id == id) {
            self.stored_buffers.remove(pos);
            true
        } else {
            false
        };
        if removed {
            // Clean up any per-buffer auxiliary state (dired, etc.) to avoid leaks.
            self.dired_states.remove(&id);
        }
        removed
    }

    /// Kill the current buffer. Refuses if it's the only one.
    pub fn kill_current_buffer(&mut self) -> bool {
        self.kill_buffer_id(self.current_buffer_id)
    }

    /// Kill a buffer by name. Returns true on success.
    pub fn kill_buffer_by_name(&mut self, name: &str) -> bool {
        if let Some(id) = self.find_buffer_by_name(name) {
            self.kill_buffer_id(id)
        } else {
            false
        }
    }

    /// Open a file as a buffer. If a buffer for this path already exists,
    /// switches to it instead of creating a duplicate.
    pub fn open_file_as_buffer(&mut self, path: PathBuf, content: &str) -> BufferId {
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        if let Some(id) = self.find_buffer_by_path(&canonical) {
            self.switch_to_buffer_id(id);
            return id;
        }
        let id = self.create_file_buffer(canonical, content);
        self.switch_to_buffer_id(id);
        id
    }

    // ---- Mini-buffer (unified modal prompt) ----

    /// Open the mini-buffer with the given prompt. `candidates` are used for
    /// completion — pass an empty Vec for free-text prompts.
    pub fn minibuffer_open(
        &mut self,
        prompt: MiniBufferPrompt,
        candidates: Vec<String>,
        action: PendingMinibufferAction,
    ) {
        self.minibuffer.open(prompt, candidates);
        self.pending_minibuffer_action = Some(action);
    }

    /// Close the mini-buffer without dispatching.
    pub fn minibuffer_cancel(&mut self) {
        self.minibuffer.close();
        self.pending_minibuffer_action = None;
    }

    /// Dispatch the pending action with the given result, then close the mini-buffer.
    pub fn minibuffer_dispatch(&mut self, result: MiniBufferResult) {
        let action = self.pending_minibuffer_action.take();
        self.minibuffer.close();
        let Some(action) = action else { return };

        match (action, result) {
            (PendingMinibufferAction::SwitchBuffer, MiniBufferResult::Submitted(name)) => {
                self.switch_to_buffer_by_name(&name);
            }
            (PendingMinibufferAction::KillBuffer, MiniBufferResult::Submitted(name)) => {
                if !name.is_empty() {
                    self.kill_buffer_by_name(&name);
                } else {
                    self.kill_current_buffer();
                }
            }
            (PendingMinibufferAction::OpenFile, MiniBufferResult::Submitted(path_str)) => {
                let raw = PathBuf::from(&path_str);
                let path = if raw.is_absolute() {
                    raw
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(raw)
                };
                if let Ok(content) = std::fs::read_to_string(&path) {
                    self.open_file_as_buffer(path, &content);
                }
            }
            (PendingMinibufferAction::GotoLine, MiniBufferResult::Submitted(s)) => {
                if let Ok(n) = s.parse::<usize>() {
                    self.jump_to_line(n);
                }
            }
            (PendingMinibufferAction::RunCommand, MiniBufferResult::Submitted(name)) => {
                // M-x — look up the command and run it.
                self.run_command_by_name(&name);
            }
            (
                PendingMinibufferAction::RunCommandWithInput { name },
                MiniBufferResult::Submitted(input),
            ) => {
                self.run_command_with_string(&name, input);
            }
            (
                PendingMinibufferAction::RunCommandWithInput { name },
                MiniBufferResult::YesNoAnswer(answer),
            ) => {
                let input = if answer { "yes" } else { "no" };
                self.run_command_with_string(&name, input.to_string());
            }
            (PendingMinibufferAction::DiredConfirmDelete, MiniBufferResult::YesNoAnswer(true)) => {
                self.dired_perform_marked_deletes();
            }
            _ => {} // Cancelled or mismatched — nothing to do
        }
    }

    /// Submit the current mini-buffer value. Dispatches the pending action.
    pub fn minibuffer_submit(&mut self) {
        if !self.minibuffer.active {
            return;
        }
        let value = self.minibuffer.current_value();
        self.minibuffer.push_history(value.clone());
        self.minibuffer_dispatch(MiniBufferResult::Submitted(value));
    }

    /// Submit a YesNo answer.
    pub fn minibuffer_submit_yes_no(&mut self, answer: bool) {
        if !self.minibuffer.active {
            return;
        }
        self.minibuffer_dispatch(MiniBufferResult::YesNoAnswer(answer));
    }

    /// Open a switch-to-buffer prompt.
    pub fn minibuffer_start_switch_buffer(&mut self) {
        let mut candidates = self.buffer_names();
        // Move the current buffer to the end so Tab/selection defaults to the
        // next buffer (Emacs convention).
        if !candidates.is_empty() {
            let current = candidates.remove(0);
            candidates.push(current);
        }
        self.minibuffer_open(
            MiniBufferPrompt::SwitchBuffer,
            candidates,
            PendingMinibufferAction::SwitchBuffer,
        );
    }

    /// Open a kill-buffer prompt.
    pub fn minibuffer_start_kill_buffer(&mut self) {
        let candidates = self.buffer_names();
        self.minibuffer_open(
            MiniBufferPrompt::KillBuffer,
            candidates,
            PendingMinibufferAction::KillBuffer,
        );
    }

    /// Open a find-file prompt at the current working directory.
    pub fn minibuffer_start_find_file(&mut self) {
        let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let candidates = list_directory_entries(&base_dir);
        self.minibuffer_open(
            MiniBufferPrompt::FindFile { base_dir },
            candidates,
            PendingMinibufferAction::OpenFile,
        );
    }

    /// Open an M-x command prompt with all registry commands as candidates.
    pub fn minibuffer_start_command(&mut self) {
        let candidates: Vec<String> = self.commands.names().map(|s| s.to_string()).collect();
        self.minibuffer_open(
            MiniBufferPrompt::Command,
            candidates,
            PendingMinibufferAction::RunCommand,
        );
    }

    // ---- Dired (directory browser) ----

    /// Open a dired buffer for the given directory. Creates a new buffer
    /// or switches to an existing dired buffer for the same path.
    pub fn dired_open(&mut self, path: PathBuf) {
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);

        // If the current buffer is already dired on this path, just refresh.
        if self.current_buffer_kind == BufferKind::Dired
            && let Some(state) = self.dired_states.get(&self.current_buffer_id)
            && state.path == canonical
        {
            let _ = self.dired_refresh();
            return;
        }
        // Check stored dired buffers
        let existing_id = self
            .stored_buffers
            .iter()
            .find(|b| {
                if b.kind == BufferKind::Dired {
                    self.dired_states
                        .get(&b.id)
                        .map(|s| s.path == canonical)
                        .unwrap_or(false)
                } else {
                    false
                }
            })
            .map(|b| b.id);
        if let Some(id) = existing_id {
            self.switch_to_buffer_id(id);
            return;
        }

        // Create a new dired buffer.
        let state = match DiredState::read_dir(&canonical) {
            Ok(s) => s,
            Err(_) => return,
        };
        let text = state.render_to_text();
        let id = self.allocate_buffer_id();
        let display_name = format!("*dired:{}*", canonical.display());
        let unique = self.pick_unique_name(&display_name);

        // Create the stored buffer
        let buf = StoredBuffer {
            id,
            name: unique,
            document: DocumentBuffer::from_text(&text),
            history: EditHistory::default(),
            cursor: EditorCursor::new(),
            kind: BufferKind::Dired,
            read_only: true,
            scroll_line: 0,
            last_edit_was_char_insert: false,
            last_move_was_vertical: false,
        };
        self.stored_buffers.push(buf);
        self.dired_states.insert(id, state);
        self.switch_to_buffer_id(id);
        self.dired_move_cursor_to_selection();
    }

    /// Position the cursor on the line corresponding to the current dired
    /// selection.
    fn dired_move_cursor_to_selection(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        if let Some(state) = self.dired_states.get(&self.current_buffer_id) {
            let line = state.cursor_line();
            if line < self.document.len_lines() {
                self.cursor.position = self.document.line_to_char(line);
                self.cursor.clear_selection();
            }
        }
    }

    /// Move to the next entry in a dired buffer.
    pub fn dired_next(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        if let Some(state) = self.dired_states.get_mut(&self.current_buffer_id) {
            state.move_down();
        }
        self.dired_move_cursor_to_selection();
    }

    /// Move to the previous entry in a dired buffer.
    pub fn dired_prev(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        if let Some(state) = self.dired_states.get_mut(&self.current_buffer_id) {
            state.move_up();
        }
        self.dired_move_cursor_to_selection();
    }

    /// Open the selected entry. If a directory, recurse into dired.
    /// If a file, open it as a buffer.
    pub fn dired_open_selected(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        let entry = match self
            .dired_states
            .get(&self.current_buffer_id)
            .and_then(|s| s.current_entry().cloned())
        {
            Some(e) => e,
            None => return,
        };
        match entry.kind {
            crate::dired::DiredEntryKind::Directory => {
                self.dired_open(entry.path);
            }
            _ => {
                if let Ok(content) = std::fs::read_to_string(&entry.path) {
                    self.open_file_as_buffer(entry.path, &content);
                }
            }
        }
    }

    /// Mark the current entry for deletion.
    pub fn dired_mark_delete(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        if let Some(state) = self.dired_states.get_mut(&self.current_buffer_id) {
            state.mark_delete();
        }
        self.dired_regenerate_text();
    }

    /// Unmark the current entry.
    pub fn dired_unmark(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        if let Some(state) = self.dired_states.get_mut(&self.current_buffer_id) {
            state.unmark();
        }
        self.dired_regenerate_text();
    }

    /// Ask for confirmation, then execute marked deletions.
    pub fn dired_execute_marks(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        let count = self
            .dired_states
            .get(&self.current_buffer_id)
            .map(|s| s.marked_paths(crate::dired::DiredMark::Delete).len())
            .unwrap_or(0);
        if count == 0 {
            return;
        }
        // Open a YesNo prompt. `dired-do-delete` is a pseudo-command that
        // actually performs the deletions when the user answers yes.
        self.minibuffer_open(
            MiniBufferPrompt::YesNo {
                label: format!("Delete {} file(s)?", count),
            },
            Vec::new(),
            PendingMinibufferAction::DiredConfirmDelete,
        );
    }

    /// Actually delete the marked files. Called after YesNo confirmation.
    fn dired_perform_marked_deletes(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        let marked: Vec<PathBuf> = self
            .dired_states
            .get(&self.current_buffer_id)
            .map(|s| s.marked_paths(crate::dired::DiredMark::Delete))
            .unwrap_or_default();
        for path in marked {
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
        let _ = self.dired_refresh();
    }

    /// Refresh the current dired buffer.
    pub fn dired_refresh(&mut self) -> std::io::Result<()> {
        if self.current_buffer_kind != BufferKind::Dired {
            return Ok(());
        }
        if let Some(state) = self.dired_states.get_mut(&self.current_buffer_id) {
            state.refresh()?;
        }
        self.dired_regenerate_text();
        self.dired_move_cursor_to_selection();
        Ok(())
    }

    /// Go up one directory in the current dired buffer.
    pub fn dired_up_directory(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        let parent = self
            .dired_states
            .get(&self.current_buffer_id)
            .and_then(|s| s.parent());
        if let Some(p) = parent {
            self.dired_open(p);
        }
    }

    /// Regenerate the buffer text from the current dired state.
    fn dired_regenerate_text(&mut self) {
        if self.current_buffer_kind != BufferKind::Dired {
            return;
        }
        let text = self
            .dired_states
            .get(&self.current_buffer_id)
            .map(|s| s.render_to_text())
            .unwrap_or_default();
        self.document.set_text(&text);
        self.last_parsed_version = 0;
    }

    // ---- Command registry dispatch ----

    /// Run a command by name. If the command is interactive (needs input),
    /// opens the mini-buffer and defers execution until submit.
    pub fn run_command_by_name(&mut self, name: &str) {
        let interactive = match self.commands.get(name) {
            Some(c) => c.interactive.clone(),
            None => return,
        };
        match interactive {
            InteractiveSpec::None => {
                self.run_command_direct(name, CommandArgs::default());
            }
            InteractiveSpec::File { prompt: _ } => {
                let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                let candidates = list_directory_entries(&base_dir);
                self.minibuffer_open(
                    MiniBufferPrompt::FindFile { base_dir },
                    candidates,
                    PendingMinibufferAction::RunCommandWithInput {
                        name: name.to_string(),
                    },
                );
            }
            InteractiveSpec::Buffer { prompt: _ } => {
                let candidates = self.buffer_names();
                self.minibuffer_open(
                    MiniBufferPrompt::SwitchBuffer,
                    candidates,
                    PendingMinibufferAction::RunCommandWithInput {
                        name: name.to_string(),
                    },
                );
            }
            InteractiveSpec::String { prompt } => {
                self.minibuffer_open(
                    MiniBufferPrompt::FreeText { label: prompt },
                    Vec::new(),
                    PendingMinibufferAction::RunCommandWithInput {
                        name: name.to_string(),
                    },
                );
            }
            InteractiveSpec::Line => {
                self.minibuffer_open(
                    MiniBufferPrompt::GotoLine,
                    Vec::new(),
                    PendingMinibufferAction::RunCommandWithInput {
                        name: name.to_string(),
                    },
                );
            }
            InteractiveSpec::Confirm { prompt } => {
                self.minibuffer_open(
                    MiniBufferPrompt::YesNo { label: prompt },
                    Vec::new(),
                    PendingMinibufferAction::RunCommandWithInput {
                        name: name.to_string(),
                    },
                );
            }
        }
    }

    /// Run a command directly by cloning the handler Rc out of the registry
    /// and invoking it. This avoids holding a borrow of the registry during
    /// dispatch so the handler can mutate `self.commands` freely.
    pub fn run_command_direct(&mut self, name: &str, args: CommandArgs) {
        let handler = self.commands.get(name).map(|c| c.handler.clone());
        if let Some(h) = handler {
            h(self, args);
        }
    }

    /// Run a command with a string argument obtained from the mini-buffer.
    pub fn run_command_with_string(&mut self, name: &str, s: String) {
        let args = CommandArgs::default().with_string(s);
        self.run_command_direct(name, args);
    }

    // ---- Keyboard macros (C-x ( / C-x ) / C-x e) ----

    /// Start recording a keyboard macro.
    pub fn macro_start_recording(&mut self) {
        self.macros.start_recording();
    }

    /// Stop recording the current macro and save it as `last`.
    pub fn macro_stop_recording(&mut self) {
        self.macros.stop_recording();
    }

    /// Execute the last recorded macro `count` times.
    pub fn macro_execute_last(&mut self, count: usize) {
        let mac = match self.macros.last.clone() {
            Some(m) => m,
            None => return,
        };
        self.macros.applying = true;
        for _ in 0..count {
            for action in &mac.actions {
                self.apply_recorded_action(action);
            }
        }
        self.macros.applying = false;
    }

    /// Execute a named macro by name.
    pub fn macro_execute_named(&mut self, name: &str, count: usize) {
        let mac = match self.macros.get_named(name).cloned() {
            Some(m) => m,
            None => return,
        };
        self.macros.applying = true;
        for _ in 0..count {
            for action in &mac.actions {
                self.apply_recorded_action(action);
            }
        }
        self.macros.applying = false;
    }

    /// Record an action if a macro is currently being recorded.
    /// Called by the editor helpers that wrap edit verbs.
    pub fn record_action(&mut self, action: RecordedAction) {
        self.macros.record(action);
    }

    /// Apply a single recorded action to the current buffer.
    /// Does NOT record itself (we're already inside a replay).
    fn apply_recorded_action(&mut self, action: &RecordedAction) {
        match action {
            RecordedAction::InsertChar(c) => self.insert_text(&c.to_string()),
            RecordedAction::InsertString(s) => self.insert_text(s),
            RecordedAction::Newline => self.insert_text("\n"),
            RecordedAction::Backspace => self.backspace(),
            RecordedAction::DeleteForward => self.delete_forward(),
            RecordedAction::MoveLeft => self.move_left(false),
            RecordedAction::MoveRight => self.move_right(false),
            RecordedAction::MoveUp => self.move_up(false),
            RecordedAction::MoveDown => self.move_down(false),
            RecordedAction::MoveWordLeft => self.move_word_left(false),
            RecordedAction::MoveWordRight => self.move_word_right(false),
            RecordedAction::MoveLineStart => self.move_to_line_start(false),
            RecordedAction::MoveLineEnd => self.move_to_line_end(false),
            RecordedAction::MoveDocStart => self.move_to_doc_start(false),
            RecordedAction::MoveDocEnd => self.move_to_doc_end(false),
            RecordedAction::KillLine => self.kill_to_end_of_line(),
            RecordedAction::KillLineBackward => self.kill_to_start_of_line(),
            RecordedAction::KillWordForward => self.kill_word_forward(),
            RecordedAction::KillWordBackward => self.kill_word_backward(),
            RecordedAction::KillRegion => self.kill_region(),
            RecordedAction::CopyRegion => self.copy_region(),
            RecordedAction::Yank => self.yank(),
            RecordedAction::YankPop => self.yank_pop(),
            RecordedAction::Undo => self.undo(),
            RecordedAction::Redo => self.redo(),
            RecordedAction::SetMark => self.set_mark(),
            RecordedAction::ClearSelection => self.cursor.clear_selection(),
            RecordedAction::ExchangePointAndMark => self.exchange_point_and_mark(),
            RecordedAction::UpcaseWord => self.upcase_word(),
            RecordedAction::DowncaseWord => self.downcase_word(),
            RecordedAction::TransposeChars => self.transpose_chars(),
            RecordedAction::TransposeWords => self.transpose_words(),
            RecordedAction::Command { name, prefix_arg: _ } => {
                // Phase 5 will wire this to the command registry.
                // For now, fall back to dispatch_palette_command so the
                // user-visible palette commands can be replayed.
                self.dispatch_palette_command(name);
            }
        }
    }

    fn update_preferred_column(&mut self) {
        if self.document.len_chars() == 0 {
            self.cursor.preferred_column = 0;
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        self.cursor.preferred_column = pos - self.document.line_to_char(line);
    }

    fn clamp_cursor(&mut self) {
        self.cursor.position = self.cursor.position.min(self.document.len_chars());
    }

    // ---- Text editing ----

    pub fn insert_text(&mut self, text: &str) {
        self.clamp_cursor();
        self.last_move_was_vertical = false;

        let is_single_char = text.chars().count() == 1;

        let has_selection = self.cursor.has_selection();
        let should_coalesce =
            is_single_char && self.last_edit_was_char_insert && !has_selection;
        if !should_coalesce {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
        }

        if let Some((start, end)) = self.cursor.selection() {
            self.document.remove(start, end);
            self.cursor.position = start;
            self.cursor.clear_selection();
        }

        self.document.insert(self.cursor.position, text);
        self.cursor.position += text.chars().count();
        self.last_edit_was_char_insert = is_single_char;
        self.update_preferred_column();
    }

    pub fn backspace(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if let Some((start, end)) = self.cursor.selection() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.remove(start, end);
            self.cursor.position = start;
            self.cursor.clear_selection();
        } else if self.cursor.position > 0 {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.cursor.position -= 1;
            self.document
                .remove(self.cursor.position, self.cursor.position + 1);
        }
        self.update_preferred_column();
    }

    pub fn delete_forward(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if let Some((start, end)) = self.cursor.selection() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.remove(start, end);
            self.cursor.position = start;
            self.cursor.clear_selection();
        } else if self.cursor.position < self.document.len_chars() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document
                .remove(self.cursor.position, self.cursor.position + 1);
        }
        self.update_preferred_column();
    }

    pub fn undo(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((snapshot, cursor)) =
            self.history
                .undo(self.document.snapshot(), self.cursor.position)
        {
            self.document.restore(snapshot);
            self.cursor.position = cursor.min(self.document.len_chars());
            self.cursor.clear_selection();
        }
    }

    pub fn redo(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((snapshot, cursor)) =
            self.history
                .redo(self.document.snapshot(), self.cursor.position)
        {
            self.document.restore(snapshot);
            self.cursor.position = cursor.min(self.document.len_chars());
            self.cursor.clear_selection();
        }
    }

    // ---- Cursor movement ----

    pub fn move_left(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if !extend
            && let Some((start, _)) = self.cursor.selection()
        {
            self.cursor.position = start;
            self.cursor.clear_selection();
            self.update_preferred_column();
            return;
        }
        if self.cursor.position > 0 {
            self.cursor.move_to(self.cursor.position - 1, extend);
        }
        self.update_preferred_column();
    }

    pub fn move_right(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if !extend
            && let Some((_, end)) = self.cursor.selection()
        {
            self.cursor.position = end;
            self.cursor.clear_selection();
            self.update_preferred_column();
            return;
        }
        if self.cursor.position < self.document.len_chars() {
            self.cursor.move_to(self.cursor.position + 1, extend);
        }
        self.update_preferred_column();
    }

    pub fn move_up(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        if line == 0 {
            self.cursor.move_to(0, extend);
            if !self.last_move_was_vertical {
                self.cursor.preferred_column = 0;
            }
            self.last_move_was_vertical = true;
            return;
        }

        let current_col = pos - self.document.line_to_char(line);
        let target_col = if self.last_move_was_vertical {
            self.cursor.preferred_column
        } else {
            current_col
        };

        let prev_line = line - 1;
        let prev_line_start = self.document.line_to_char(prev_line);
        let prev_line_chars = self.document.line(prev_line).len_chars();
        let max_col = if prev_line < self.document.len_lines() - 1 {
            prev_line_chars.saturating_sub(1)
        } else {
            prev_line_chars
        };
        let actual_col = target_col.min(max_col);
        self.cursor.move_to(prev_line_start + actual_col, extend);
        self.cursor.preferred_column = target_col;
        self.last_move_was_vertical = true;
    }

    pub fn move_down(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        if line >= self.document.len_lines() - 1 {
            self.cursor.move_to(self.document.len_chars(), extend);
            if !self.last_move_was_vertical {
                self.cursor.preferred_column = pos - self.document.line_to_char(line);
            }
            self.last_move_was_vertical = true;
            return;
        }

        let current_col = pos - self.document.line_to_char(line);
        let target_col = if self.last_move_was_vertical {
            self.cursor.preferred_column
        } else {
            current_col
        };

        let next_line = line + 1;
        let next_line_start = self.document.line_to_char(next_line);
        let next_line_chars = self.document.line(next_line).len_chars();
        let max_col = if next_line < self.document.len_lines() - 1 {
            next_line_chars.saturating_sub(1)
        } else {
            next_line_chars
        };
        let actual_col = target_col.min(max_col);
        self.cursor.move_to(next_line_start + actual_col, extend);
        self.cursor.preferred_column = target_col;
        self.last_move_was_vertical = true;
    }

    pub fn move_to_line_start(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        self.cursor.move_to(line_start, extend);
        self.update_preferred_column();
    }

    pub fn move_to_line_end(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        let line_len = self.document.line(line).len_chars();
        let line_end = if line < self.document.len_lines() - 1 {
            line_start + line_len.saturating_sub(1)
        } else {
            line_start + line_len
        };
        self.cursor.move_to(line_end, extend);
        self.update_preferred_column();
    }

    pub fn move_word_left(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let rope = self.document.rope();
        let mut pos = self.cursor.position;
        // Emacs M-b: skip non-word chars backward, then skip word chars backward
        while pos > 0 {
            let ch = rope.char(pos - 1);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            pos -= 1;
        }
        while pos > 0 {
            let ch = rope.char(pos - 1);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            pos -= 1;
        }
        self.cursor.move_to(pos, extend);
        self.update_preferred_column();
    }

    pub fn move_word_right(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let rope = self.document.rope();
        let len = rope.len_chars();
        let mut pos = self.cursor.position;
        // Emacs M-f: skip non-word chars, then skip word chars (land at end of next word)
        while pos < len {
            let ch = rope.char(pos);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            pos += 1;
        }
        while pos < len {
            let ch = rope.char(pos);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            pos += 1;
        }
        self.cursor.move_to(pos, extend);
        self.update_preferred_column();
    }

    pub fn select_all(&mut self) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.anchor = Some(0);
        self.cursor.position = self.document.len_chars();
    }

    pub fn move_to_doc_start(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.move_to(0, extend);
        self.update_preferred_column();
    }

    pub fn move_to_doc_end(&mut self, extend: bool) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        self.cursor.move_to(self.document.len_chars(), extend);
        self.update_preferred_column();
    }

    pub fn selected_text(&self) -> Option<String> {
        self.cursor.selection().map(|(start, end)| {
            self.document.rope().slice(start..end).to_string()
        })
    }

    pub fn delete_selection(&mut self) -> Option<String> {
        if let Some((start, end)) = self.cursor.selection() {
            let deleted = self.document.rope().slice(start..end).to_string();
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.remove(start, end);
            self.cursor.position = start;
            self.cursor.clear_selection();
            Some(deleted)
        } else {
            None
        }
    }

    pub fn jump_to_line(&mut self, line: usize) {
        if line == 0 {
            return;
        }
        let line_idx = (line - 1).min(self.document.len_lines().saturating_sub(1));
        let char_offset = self.document.line_to_char(line_idx);
        self.cursor.position = char_offset;
        self.cursor.clear_selection();
        self.update_preferred_column();
    }

    // ---- Word/Line selection ----

    pub fn select_word_at(&mut self, char_pos: usize) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        let text = self.document.text();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        if len == 0 {
            return;
        }
        let pos = char_pos.min(len.saturating_sub(1));

        let category = |c: char| -> u8 {
            if c.is_alphanumeric() || c == '_' {
                0
            } else if c.is_whitespace() {
                1
            } else {
                2
            }
        };

        let target_cat = category(chars[pos]);

        let mut start = pos;
        while start > 0 && category(chars[start - 1]) == target_cat {
            start -= 1;
        }
        let mut end = pos;
        while end < len && category(chars[end]) == target_cat {
            end += 1;
        }

        self.cursor.anchor = Some(start);
        self.cursor.position = end;
        self.kill_ring.reset_flags();
    }

    pub fn select_line_at(&mut self, char_pos: usize) {
        self.last_edit_was_char_insert = false;
        self.last_move_was_vertical = false;
        if self.document.len_chars() == 0 {
            return;
        }
        let pos = char_pos.min(self.document.len_chars().saturating_sub(1));
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        let line_end = if line + 1 < self.document.len_lines() {
            self.document.line_to_char(line + 1)
        } else {
            self.document.len_chars()
        };
        self.cursor.anchor = Some(line_start);
        self.cursor.position = line_end;
        self.kill_ring.reset_flags();
    }

    // ---- Kill ring operations ----

    pub fn kill_to_end_of_line(&mut self) {
        self.last_edit_was_char_insert = false;
        if self.document.len_chars() == 0 {
            return;
        }
        self.clamp_cursor();
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        let line_len = self.document.line(line).len_chars();
        let line_end = line_start + line_len;
        let is_last_line = line + 1 >= self.document.len_lines();
        let kill_end = if is_last_line
            || (self.cursor.position >= line_end.saturating_sub(1))
        {
            line_end // last line or cursor at EOL: kill to end (including newline on non-last)
        } else {
            line_end.saturating_sub(1) // non-last line, not at EOL: kill up to newline
        };

        if self.cursor.position >= kill_end {
            return;
        }

        let killed = self
            .document
            .rope()
            .slice(self.cursor.position..kill_end)
            .to_string();
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        self.document.remove(self.cursor.position, kill_end);
        self.kill_ring.push(killed);
    }

    pub fn kill_to_start_of_line(&mut self) {
        self.last_edit_was_char_insert = false;
        if self.document.len_chars() == 0 {
            return;
        }
        self.clamp_cursor();
        if self.cursor.position == 0 {
            return;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let line = self.document.char_to_line(pos);
        let line_start = self.document.line_to_char(line);
        if self.cursor.position <= line_start {
            return;
        }
        let killed = self
            .document
            .rope()
            .slice(line_start..self.cursor.position)
            .to_string();
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        self.document.remove(line_start, self.cursor.position);
        self.cursor.position = line_start;
        self.kill_ring.push_prepend(killed);
    }

    pub fn kill_word_forward(&mut self) {
        self.last_edit_was_char_insert = false;
        let start = self.cursor.position;
        let rope = self.document.rope();
        let len = rope.len_chars();
        let mut end = start;
        // Emacs M-d: skip non-word chars, then skip word chars
        while end < len {
            let ch = rope.char(end);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            end += 1;
        }
        while end < len {
            let ch = rope.char(end);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            end += 1;
        }
        if end > start {
            let killed = self.document.rope().slice(start..end).to_string();
            self.history
                .push_undo(self.document.snapshot(), start);
            self.document.remove(start, end);
            self.kill_ring.push(killed);
        }
    }

    pub fn kill_word_backward(&mut self) {
        self.last_edit_was_char_insert = false;
        let end = self.cursor.position;
        let rope = self.document.rope();
        let mut start = end;
        // Emacs M-Backspace: skip non-word chars backward, then skip word chars backward
        while start > 0 {
            let ch = rope.char(start - 1);
            if ch.is_alphanumeric() || ch == '_' {
                break;
            }
            start -= 1;
        }
        while start > 0 {
            let ch = rope.char(start - 1);
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            start -= 1;
        }
        if end > start {
            let killed = self.document.rope().slice(start..end).to_string();
            self.history
                .push_undo(self.document.snapshot(), end);
            self.cursor.position = start;
            self.document.remove(start, end);
            self.kill_ring.push_prepend(killed);
        }
    }

    pub fn kill_region(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some(text) = self.delete_selection() {
            self.kill_ring.push(text);
        }
    }

    pub fn copy_region(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((start, end)) = self.cursor.selection() {
            let text = self.document.rope().slice(start..end).to_string();
            self.kill_ring.push(text);
            self.kill_ring.clear_kill_flag(); // M-w is a copy, not a kill — don't chain
            self.cursor.clear_selection();
        }
    }

    pub fn yank(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some(text) = self.kill_ring.yank().map(|s| s.to_string()) {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);

            if let Some((start, end)) = self.cursor.selection() {
                self.document.remove(start, end);
                self.cursor.position = start;
                self.cursor.clear_selection();
            }

            let start = self.cursor.position;
            self.document.insert(self.cursor.position, &text);
            self.cursor.position += text.chars().count();
            self.kill_ring.mark_yank(start, self.cursor.position);
        }
    }

    pub fn yank_pop(&mut self) {
        self.last_edit_was_char_insert = false;
        if let Some((start, end)) = self.kill_ring.last_yank_range()
            && let Some(text) = self.kill_ring.yank_pop().map(|s| s.to_string())
        {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.remove(start, end);
            self.document.insert(start, &text);
            let new_end = start + text.chars().count();
            self.cursor.position = new_end;
            self.kill_ring.mark_yank(start, new_end);
        }
    }

    pub fn set_mark(&mut self) {
        self.cursor.anchor = Some(self.cursor.position);
    }

    pub fn exchange_point_and_mark(&mut self) {
        if let Some(anchor) = self.cursor.anchor {
            let old_pos = self.cursor.position;
            self.cursor.position = anchor;
            self.cursor.anchor = Some(old_pos);
        }
    }

    // ---- Rectangle operations ----

    pub fn rectangle_selection(&self) -> Option<(usize, usize, usize, usize)> {
        let anchor = self.cursor.anchor?;
        if self.document.len_chars() == 0 {
            return None;
        }
        let pos = self.cursor.position.min(self.document.len_chars());
        let anchor = anchor.min(self.document.len_chars());

        let pos_line = self.document.char_to_line(pos);
        let pos_col = pos - self.document.line_to_char(pos_line);
        let anc_line = self.document.char_to_line(anchor);
        let anc_col = anchor - self.document.line_to_char(anc_line);

        Some((
            pos_line.min(anc_line),
            pos_col.min(anc_col),
            pos_line.max(anc_line),
            pos_col.max(anc_col),
        ))
    }

    pub fn delete_rectangle(&mut self) {
        if let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            for line_idx in (start_line..=end_line).rev() {
                let line_start = self.document.line_to_char(line_idx);
                let line_len = self.document.line(line_idx).len_chars();
                let col_start = start_col.min(line_len);
                let col_end = end_col.min(line_len);
                if col_start < col_end {
                    self.document
                        .remove(line_start + col_start, line_start + col_end);
                }
            }
            self.cursor.clear_selection();
        }
    }

    pub fn kill_rectangle(&mut self) {
        if let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() {
            let mut lines = Vec::new();
            for line_idx in start_line..=end_line {
                let line_text: String = self.document.line(line_idx).to_string();
                let chars: Vec<char> = line_text.chars().collect();
                let col_start = start_col.min(chars.len());
                let col_end = end_col.min(chars.len());
                lines.push(chars[col_start..col_end].iter().collect());
            }
            self.kill_ring.rectangle_buffer = Some(lines);
            self.delete_rectangle();
        }
    }

    pub fn yank_rectangle(&mut self) {
        let rect = match &self.kill_ring.rectangle_buffer {
            Some(r) => r.clone(),
            None => return,
        };
        if rect.is_empty() {
            return;
        }
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        let pos = self.cursor.position.min(self.document.len_chars());
        let (start_line, start_col) = if self.document.len_chars() > 0 {
            let l = self.document.char_to_line(pos);
            (l, pos - self.document.line_to_char(l))
        } else {
            (0, 0)
        };

        for (i, rect_line) in rect.iter().enumerate() {
            let target_line = start_line + i;
            if target_line >= self.document.len_lines() {
                let doc_end = self.document.len_chars();
                self.document
                    .insert(doc_end, &format!("\n{}", " ".repeat(start_col)));
                let insert_pos = self.document.len_chars();
                self.document.insert(insert_pos, rect_line);
            } else {
                let line_start = self.document.line_to_char(target_line);
                let line_len = self.document.line(target_line).len_chars();
                let effective_col = start_col.min(line_len);
                if effective_col < start_col {
                    let padding = " ".repeat(start_col - effective_col);
                    self.document.insert(line_start + effective_col, &padding);
                }
                let insert_pos = self.document.line_to_char(target_line) + start_col;
                self.document.insert(insert_pos, rect_line);
            }
        }
        self.cursor.clear_selection();
    }

    pub fn open_rectangle(&mut self) {
        if let Some((start_line, start_col, end_line, end_col)) = self.rectangle_selection() {
            let width = end_col - start_col;
            if width == 0 {
                return;
            }
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            let spaces = " ".repeat(width);
            for line_idx in (start_line..=end_line).rev() {
                let line_start = self.document.line_to_char(line_idx);
                let line_len = self.document.line(line_idx).len_chars();
                let col = start_col.min(line_len);
                self.document.insert(line_start + col, &spaces);
            }
            self.cursor.clear_selection();
        }
    }

    pub fn toggle_format(&mut self, prefix: &str, suffix: &str) {
        self.last_edit_was_char_insert = false;
        if let Some((start, end)) = self.cursor.selection() {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.insert(end, suffix);
            self.document.insert(start, prefix);
            self.cursor.position = end + prefix.chars().count() + suffix.chars().count();
            self.cursor.clear_selection();
        } else {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            let marker = format!("{}{}", prefix, suffix);
            self.document.insert(self.cursor.position, &marker);
            self.cursor.position += prefix.chars().count();
        }
    }

    pub fn toggle_find(&mut self) {
        self.find_bar_visible = !self.find_bar_visible;
        if !self.find_bar_visible {
            self.find_query.clear();
            self.replace_bar_visible = false;
        }
    }

    pub fn toggle_find_replace(&mut self) {
        self.find_bar_visible = true;
        self.replace_bar_visible = !self.replace_bar_visible;
    }

    pub fn find_next(&mut self) {
        if self.find_query.is_empty() {
            return;
        }
        let text = self.document.text();
        let byte_start = text
            .char_indices()
            .nth(self.cursor.position + 1)
            .map(|(i, _)| i)
            .unwrap_or(text.len());

        if let Some(byte_pos) = text[byte_start..].find(&self.find_query) {
            let char_pos = text[..byte_start + byte_pos].chars().count();
            self.cursor.position = char_pos;
            self.cursor.clear_selection();
        } else if let Some(byte_pos) = text.find(&self.find_query) {
            let char_pos = text[..byte_pos].chars().count();
            self.cursor.position = char_pos;
            self.cursor.clear_selection();
        }
    }

    pub fn replace_next(&mut self) {
        if self.find_query.is_empty() {
            return;
        }
        let text = self.document.text();
        let cursor_byte = text
            .char_indices()
            .nth(self.cursor.position)
            .map(|(i, _)| i)
            .unwrap_or(text.len());

        if text[cursor_byte..].starts_with(&self.find_query) {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            let end = self.cursor.position + self.find_query.chars().count();
            self.document.remove(self.cursor.position, end);
            self.document
                .insert(self.cursor.position, &self.replace_text);
            self.cursor.position += self.replace_text.chars().count();
        }
        self.find_next();
    }

    pub fn replace_all(&mut self) {
        if self.find_query.is_empty() {
            return;
        }
        let text = self.document.text();
        let new_text = text.replace(&self.find_query, &self.replace_text);
        if new_text != text {
            self.history
                .push_undo(self.document.snapshot(), self.cursor.position);
            self.document.replace_content(&new_text);
            self.cursor.position = self.cursor.position.min(self.document.len_chars());
        }
    }

    pub fn add_recent_file(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        if self.recent_files.len() > 10 {
            self.recent_files.truncate(10);
        }
    }

    pub fn start_inline_edit(&mut self, block_id: &str) {
        if let Some(span) = self.source_map.get(block_id) {
            let start_line = (span.start_line.saturating_sub(1))
                .min(self.document.len_lines().saturating_sub(1));
            let end_line = span.end_line.min(self.document.len_lines());
            let start_char = self.document.line_to_char(start_line);
            let end_char = if end_line < self.document.len_lines() {
                self.document.line_to_char(end_line)
            } else {
                self.document.len_chars()
            };
            let block_text: String = self.document.rope().slice(start_char..end_char).into();
            self.editing_block = Some(block_id.to_string());
            self.editing_block_text = block_text;
        }
    }

    pub fn commit_inline_edit(&mut self) {
        if let Some(block_id) = self.editing_block.take() {
            if let Some(span) = self.source_map.get(&block_id) {
                let start_line = (span.start_line.saturating_sub(1))
                    .min(self.document.len_lines().saturating_sub(1));
                let end_line = span.end_line.min(self.document.len_lines());
                let start_char = self.document.line_to_char(start_line);
                let end_char = if end_line < self.document.len_lines() {
                    self.document.line_to_char(end_line)
                } else {
                    self.document.len_chars()
                };
                self.history
                    .push_undo(self.document.snapshot(), self.cursor.position);
                self.document.remove(start_char, end_char);
                self.document.insert(start_char, &self.editing_block_text);
            }
            self.editing_block_text.clear();
        }
    }

    pub fn cancel_inline_edit(&mut self) {
        self.editing_block = None;
        self.editing_block_text.clear();
    }

    // ---- Universal argument (C-u) ----

    /// Start or multiply the universal argument prefix.
    pub fn universal_argument(&mut self) {
        match self.universal_arg {
            None => {
                self.universal_arg = Some(4);
                self.universal_arg_accumulating = true;
            }
            Some(n) => {
                self.universal_arg = Some(n.saturating_mul(4));
            }
        }
    }

    /// Accumulate a digit into the universal argument. Returns true if consumed.
    pub fn universal_arg_digit(&mut self, digit: u8) -> bool {
        if !self.universal_arg_accumulating {
            return false;
        }
        let current = self.universal_arg.unwrap_or(0);
        // If at a power-of-4 default, replace entirely with the typed digit
        if matches!(current, 4 | 16 | 64 | 256) {
            self.universal_arg = Some(digit as usize);
        } else {
            self.universal_arg = Some(current.saturating_mul(10).saturating_add(digit as usize));
        }
        true
    }

    /// Take and reset the universal argument. Returns 1 if no prefix was set.
    pub fn take_universal_arg(&mut self) -> usize {
        let n = self.universal_arg.unwrap_or(1);
        self.universal_arg = None;
        self.universal_arg_accumulating = false;
        n
    }

    pub fn cancel_universal_arg(&mut self) {
        self.universal_arg = None;
        self.universal_arg_accumulating = false;
    }

    // ---- Abort (C-g) ----

    /// Cancel the current modal operation (selection, isearch, palette, universal arg, find bar).
    pub fn abort(&mut self) {
        if self.isearch.active {
            self.isearch_abort();
            return;
        }
        if self.command_palette.visible {
            self.command_palette_dismiss();
            return;
        }
        self.cancel_universal_arg();
        self.zap_to_char_pending = false;
        self.c_x_pending = false;
        self.c_x_r_pending = false;
        self.cursor.clear_selection();
        if self.find_bar_visible {
            self.find_bar_visible = false;
            self.find_query.clear();
            self.replace_bar_visible = false;
        }
    }

    // ---- Upcase / downcase word (M-u / M-l) ----

    pub fn upcase_word(&mut self) {
        self.last_edit_was_char_insert = false;
        let start = self.cursor.position;
        self.move_word_right(false);
        let end = self.cursor.position;
        if end > start && end <= self.document.len_chars() {
            let word: String = self.document.rope().slice(start..end).into();
            let upper = word.to_uppercase();
            self.history
                .push_undo(self.document.snapshot(), start);
            self.document.remove(start, end);
            self.document.insert(start, &upper);
            self.cursor.position = start + upper.chars().count();
        }
    }

    pub fn downcase_word(&mut self) {
        self.last_edit_was_char_insert = false;
        let start = self.cursor.position;
        self.move_word_right(false);
        let end = self.cursor.position;
        if end > start && end <= self.document.len_chars() {
            let word: String = self.document.rope().slice(start..end).into();
            let lower = word.to_lowercase();
            self.history
                .push_undo(self.document.snapshot(), start);
            self.document.remove(start, end);
            self.document.insert(start, &lower);
            self.cursor.position = start + lower.chars().count();
        }
    }

    // ---- Transpose (C-t / M-t) ----

    /// Transpose the two characters before the cursor (Emacs C-t).
    pub fn transpose_chars(&mut self) {
        self.last_edit_was_char_insert = false;
        let pos = self.cursor.position;
        let len = self.document.len_chars();
        if len < 2 {
            return;
        }
        // At end of doc or mid-line: swap chars at pos-2 and pos-1, advance cursor.
        // At position 0: no-op. At position 1: swap chars 0 and 1.
        let (a, b) = if pos == 0 {
            return;
        } else if pos >= len {
            (len - 2, len - 1)
        } else {
            (pos - 1, pos)
        };
        let rope = self.document.rope();
        let ch_a = rope.char(a);
        let ch_b = rope.char(b);
        if ch_a == ch_b {
            self.cursor.position = (b + 1).min(len);
            return;
        }
        self.history
            .push_undo(self.document.snapshot(), self.cursor.position);
        let a_str = ch_a.to_string();
        let b_str = ch_b.to_string();
        self.document.remove(a, b + 1);
        self.document.insert(a, &format!("{}{}", b_str, a_str));
        self.cursor.position = (b + 1).min(self.document.len_chars());
    }

    /// Transpose the two words around the cursor (Emacs M-t).
    pub fn transpose_words(&mut self) {
        self.last_edit_was_char_insert = false;
        let pos = self.cursor.position;

        // Find end of current/next word (move forward past word boundary)
        self.move_word_right(false);
        let word2_end = self.cursor.position;
        self.move_word_left(false);
        let word2_start = self.cursor.position;

        if word2_start == 0 {
            self.cursor.position = pos;
            return;
        }

        // Find the previous word
        self.cursor.position = word2_start;
        self.move_word_left(false);
        let word1_start = self.cursor.position;
        self.move_word_right(false);
        let word1_end = self.cursor.position;

        if word1_start == word2_start || word1_end > word2_start {
            self.cursor.position = pos;
            return;
        }

        let rope = self.document.rope();
        let word1: String = rope.slice(word1_start..word1_end).into();
        let word2: String = rope.slice(word2_start..word2_end).into();

        self.history
            .push_undo(self.document.snapshot(), pos);

        // Replace word2 first (higher offset) to avoid invalidating word1 offsets
        self.document.remove(word2_start, word2_end);
        self.document.insert(word2_start, &word1);
        self.document.remove(word1_start, word1_end);
        self.document.insert(word1_start, &word2);

        // Cursor goes to end of what was word2 (now in word1's position)
        // + the length difference
        self.cursor.position = word2_start + word1.chars().count();
    }

    // ---- Zap to char (M-z) ----

    /// Enter zap-to-char mode: the next character typed will be the target.
    pub fn zap_to_char_start(&mut self) {
        self.zap_to_char_pending = true;
    }

    /// Execute zap-to-char: kill from cursor to (and including) the next
    /// occurrence of `target`.
    pub fn zap_to_char(&mut self, target: char) {
        self.zap_to_char_pending = false;
        self.last_edit_was_char_insert = false;
        let pos = self.cursor.position;
        let text = self.document.text();
        let byte_start = text
            .char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        // Search for target char after current position
        if let Some(rel_byte) = text[byte_start..].find(target) {
            let target_byte = byte_start + rel_byte;
            // Include the target char
            let target_end_byte = target_byte + target.len_utf8();
            let end_char = text[..target_end_byte].chars().count();
            if end_char > pos {
                let killed: String = self.document.rope().slice(pos..end_char).into();
                self.kill_ring.push(killed);
                self.history
                    .push_undo(self.document.snapshot(), self.cursor.position);
                self.document.remove(pos, end_char);
                self.cursor.position = pos.min(self.document.len_chars());
            }
        }
    }

    // ---- Recenter (C-l) ----

    /// Hint that the view should recenter around the cursor.
    /// The actual scrolling is performed by the editor pane when it reads this flag.
    pub fn recenter(&mut self) {
        // Set editor_scroll_line to the cursor's current line so the view
        // can use it as a centering target on next render.
        if self.document.len_chars() > 0 {
            let pos = self.cursor.position.min(self.document.len_chars());
            self.editor_scroll_line = self.document.char_to_line(pos);
        }
    }

    // ---- Page navigation ----

    pub fn page_up(&mut self) {
        for _ in 0..40 {
            self.move_up(false);
        }
    }

    pub fn page_down(&mut self) {
        for _ in 0..40 {
            self.move_down(false);
        }
    }

    // ---- Incremental search (C-s / C-r) ----

    pub fn isearch_start(&mut self, direction: IsearchDirection) {
        self.isearch = IsearchState {
            active: true,
            direction,
            query: String::new(),
            start_position: self.cursor.position,
            current_match_start: None,
        };
    }

    pub fn isearch_add_char(&mut self, ch: char) {
        self.isearch.query.push(ch);
        self.isearch_find_current();
    }

    pub fn isearch_backspace(&mut self) {
        self.isearch.query.pop();
        if self.isearch.query.is_empty() {
            self.cursor.position = self.isearch.start_position;
            self.isearch.current_match_start = None;
        } else {
            // Re-search from the start position
            let saved = self.cursor.position;
            self.cursor.position = self.isearch.start_position;
            if self.isearch_find_current().is_none() {
                self.cursor.position = saved;
            }
        }
    }

    pub fn isearch_next(&mut self) {
        self.isearch.direction = IsearchDirection::Forward;
        if self.isearch.query.is_empty() {
            return;
        }
        let from = self.cursor.position + 1;
        self.isearch_find_forward_from(from);
    }

    pub fn isearch_prev(&mut self) {
        self.isearch.direction = IsearchDirection::Backward;
        if self.isearch.query.is_empty() {
            return;
        }
        self.isearch_find_backward();
    }

    pub fn isearch_exit(&mut self) {
        self.isearch.active = false;
        self.cursor.clear_selection();
    }

    pub fn isearch_abort(&mut self) {
        self.cursor.position = self.isearch.start_position;
        self.cursor.clear_selection();
        self.isearch.active = false;
    }

    fn isearch_find_current(&mut self) -> Option<usize> {
        if self.isearch.query.is_empty() {
            return None;
        }
        match self.isearch.direction {
            IsearchDirection::Forward => {
                self.isearch_find_forward_from(self.isearch.start_position)
            }
            IsearchDirection::Backward => self.isearch_find_backward(),
        }
    }

    fn isearch_find_forward_from(&mut self, from: usize) -> Option<usize> {
        let text = self.document.text();
        let query = &self.isearch.query;
        let byte_start = text
            .char_indices()
            .nth(from)
            .map(|(i, _)| i)
            .unwrap_or(text.len());

        if let Some(byte_pos) = text[byte_start..].find(query.as_str()) {
            let char_pos = text[..byte_start + byte_pos].chars().count();
            self.cursor.position = char_pos;
            self.isearch.current_match_start = Some(char_pos);
            return Some(char_pos);
        }
        // Wrap around
        if let Some(byte_pos) = text[..byte_start].find(query.as_str()) {
            let char_pos = text[..byte_pos].chars().count();
            self.cursor.position = char_pos;
            self.isearch.current_match_start = Some(char_pos);
            return Some(char_pos);
        }
        None
    }

    fn isearch_find_backward(&mut self) -> Option<usize> {
        let text = self.document.text();
        let query = &self.isearch.query;
        let byte_end = text
            .char_indices()
            .nth(self.cursor.position)
            .map(|(i, _)| i)
            .unwrap_or(text.len());

        if let Some(byte_pos) = text[..byte_end].rfind(query.as_str()) {
            let char_pos = text[..byte_pos].chars().count();
            self.cursor.position = char_pos;
            self.isearch.current_match_start = Some(char_pos);
            return Some(char_pos);
        }
        // Wrap around from end
        if let Some(byte_pos) = text[byte_end..].rfind(query.as_str()) {
            let char_pos = text[..byte_end + byte_pos].chars().count();
            self.cursor.position = char_pos;
            self.isearch.current_match_start = Some(char_pos);
            return Some(char_pos);
        }
        None
    }

    // ---- Command palette (M-x) ----

    pub fn toggle_command_palette(&mut self) {
        self.command_palette.visible = !self.command_palette.visible;
        if self.command_palette.visible {
            self.command_palette.query.clear();
            self.command_palette.selected = 0;
            self.command_palette.mode = PaletteMode::Command;
            self.command_palette_filter();
        }
    }

    pub fn command_palette_add_char(&mut self, ch: char) {
        self.command_palette.query.push(ch);
        self.command_palette.selected = 0;
        match self.command_palette.mode {
            PaletteMode::Command => self.command_palette_filter(),
            PaletteMode::GotoLine => {} // just accumulate digits
            PaletteMode::SwitchBuffer | PaletteMode::KillBuffer => {
                self.command_palette_filter_buffers();
            }
        }
    }

    pub fn command_palette_backspace(&mut self) {
        self.command_palette.query.pop();
        self.command_palette.selected = 0;
        if matches!(self.command_palette.mode, PaletteMode::Command) {
            self.command_palette_filter();
        }
    }

    pub fn command_palette_select_next(&mut self) {
        if !self.command_palette.filtered_indices.is_empty() {
            self.command_palette.selected =
                (self.command_palette.selected + 1) % self.command_palette.filtered_indices.len();
        }
    }

    pub fn command_palette_select_prev(&mut self) {
        if !self.command_palette.filtered_indices.is_empty() {
            self.command_palette.selected = self
                .command_palette
                .selected
                .checked_sub(1)
                .unwrap_or(self.command_palette.filtered_indices.len().saturating_sub(1));
        }
    }

    /// Execute the selected command. Returns the command id for the view to dispatch.
    pub fn command_palette_execute(&mut self) -> Option<String> {
        if !self.command_palette.visible {
            return None;
        }

        match self.command_palette.mode {
            PaletteMode::GotoLine => {
                if let Ok(line) = self.command_palette.query.parse::<usize>() {
                    self.jump_to_line(line);
                }
                self.command_palette.visible = false;
                self.command_palette.query.clear();
                None
            }
            PaletteMode::Command => {
                let idx = self
                    .command_palette
                    .filtered_indices
                    .get(self.command_palette.selected)?;
                let cmd = self.command_palette.commands.get(*idx)?;
                let id = cmd.id.to_string();

                // Handle goto-line specially: switch to prompt mode
                if id == "goto-line" {
                    self.command_palette.mode = PaletteMode::GotoLine;
                    self.command_palette.query.clear();
                    return None;
                }

                self.command_palette.visible = false;
                self.command_palette.query.clear();
                Some(id)
            }
            PaletteMode::SwitchBuffer => {
                let name = self.command_palette_selected_buffer_name();
                self.command_palette.visible = false;
                self.command_palette.query.clear();
                self.command_palette.mode = PaletteMode::Command;
                if let Some(name) = name {
                    self.switch_to_buffer_by_name(&name);
                }
                None
            }
            PaletteMode::KillBuffer => {
                let name = self.command_palette_selected_buffer_name();
                self.command_palette.visible = false;
                self.command_palette.query.clear();
                self.command_palette.mode = PaletteMode::Command;
                if let Some(name) = name {
                    self.kill_buffer_by_name(&name);
                }
                None
            }
        }
    }

    /// Returns the currently highlighted buffer name in a SwitchBuffer/KillBuffer palette.
    fn command_palette_selected_buffer_name(&self) -> Option<String> {
        // In buffer mode, `commands` contains the buffer list as PaletteCommands
        // with name = buffer name. If typed query doesn't match any filtered entry,
        // fall back to the typed query itself.
        if let Some(idx) = self
            .command_palette
            .filtered_indices
            .get(self.command_palette.selected)
            && let Some(cmd) = self.command_palette.commands.get(*idx)
        {
            return Some(cmd.name.clone());
        }
        if !self.command_palette.query.is_empty() {
            Some(self.command_palette.query.clone())
        } else {
            None
        }
    }

    /// Filter the palette candidates by the current query (buffer name mode).
    fn command_palette_filter_buffers(&mut self) {
        let query = self.command_palette.query.to_lowercase();
        self.command_palette.filtered_indices = self
            .command_palette
            .commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| {
                query.is_empty() || cmd.name.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        self.command_palette.selected = 0;
    }

    /// Open the palette in SwitchBuffer mode, populated with current buffer names.
    pub fn command_palette_start_switch_buffer(&mut self) {
        self.command_palette.visible = true;
        self.command_palette.mode = PaletteMode::SwitchBuffer;
        self.command_palette.query.clear();
        self.command_palette.selected = 0;
        self.command_palette.commands = self
            .buffer_names()
            .into_iter()
            .map(|name| PaletteCommand {
                description: name.clone(),
                name,
                id: "switch-buffer",
            })
            .collect();
        self.command_palette.filtered_indices =
            (0..self.command_palette.commands.len()).collect();
    }

    /// Open the palette in KillBuffer mode, populated with current buffer names.
    pub fn command_palette_start_kill_buffer(&mut self) {
        self.command_palette.visible = true;
        self.command_palette.mode = PaletteMode::KillBuffer;
        self.command_palette.query.clear();
        self.command_palette.selected = 0;
        self.command_palette.commands = self
            .buffer_names()
            .into_iter()
            .map(|name| PaletteCommand {
                description: format!("Kill buffer: {}", name),
                name,
                id: "kill-buffer",
            })
            .collect();
        self.command_palette.filtered_indices =
            (0..self.command_palette.commands.len()).collect();
    }

    pub fn command_palette_dismiss(&mut self) {
        self.command_palette.visible = false;
        self.command_palette.query.clear();
        self.command_palette.mode = PaletteMode::Command;
    }

    /// Dispatch a palette command by id. Returns true if handled internally.
    pub fn dispatch_palette_command(&mut self, id: &str) -> bool {
        match id {
            "toggle-preview" => self.show_preview = !self.show_preview,
            "toggle-line-numbers" => self.show_line_numbers = !self.show_line_numbers,
            "toggle-preview-line-numbers" => self.show_preview_line_numbers = !self.show_preview_line_numbers,
            "find" => self.toggle_find(),
            "replace" => self.toggle_find_replace(),
            "select-all" => self.select_all(),
            "undo" => self.undo(),
            "redo" => self.redo(),
            "upcase-word" => self.upcase_word(),
            "downcase-word" => self.downcase_word(),
            "exchange-point-and-mark" => self.exchange_point_and_mark(),
            "transpose-chars" => self.transpose_chars(),
            "transpose-words" => self.transpose_words(),
            "zap-to-char" => self.zap_to_char_start(),
            "dired" => {
                let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                self.dired_open(dir);
            }
            // "load-theme" needs view-level handling (theme picker)
            _ => return false,
        }
        true
    }

    fn command_palette_filter(&mut self) {
        let query = self.command_palette.query.to_lowercase();
        self.command_palette.filtered_indices = self
            .command_palette
            .commands
            .iter()
            .enumerate()
            .filter(|(_, cmd)| query.is_empty() || cmd.name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
    }
}

impl Default for MdAppState {
    fn default() -> Self {
        Self::new()
    }
}

/// List entries in a directory for find-file completion.
/// Directories are suffixed with "/" to make them visually distinct.
fn list_directory_entries(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                let mut s = name.to_string();
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    s.push('/');
                }
                out.push(s);
            }
        }
    }
    out.sort();
    out
}
