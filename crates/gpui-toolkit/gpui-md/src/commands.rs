//! Command registry — the plugin architecture for the editor.
//!
//! All user-visible commands (Rust built-ins and, later, Lisp commands) live
//! in a single `CommandRegistry`. M-x, keybindings, menus, and keyboard macros
//! all consume this registry.
//!
//! A command has a name, description, category, interactive specification
//! (what kind of input it needs via the mini-buffer), and a handler closure.
//! Handlers take a mutable reference to `MdAppState` and a `CommandArgs`
//! struct containing the prefix argument and any mini-buffer string.

use std::collections::HashMap;
use std::rc::Rc;

use crate::state::MdAppState;

/// Arguments passed to a command handler.
#[derive(Clone, Debug, Default)]
pub struct CommandArgs {
    /// Universal prefix argument (C-u N).
    pub prefix_arg: Option<usize>,
    /// Mini-buffer string input, if the command was interactive.
    pub string: Option<String>,
}

impl CommandArgs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_prefix(mut self, n: usize) -> Self {
        self.prefix_arg = Some(n);
        self
    }

    pub fn with_string(mut self, s: String) -> Self {
        self.string = Some(s);
        self
    }

    /// Return the prefix argument, or 1 if unset.
    pub fn count(&self) -> usize {
        self.prefix_arg.unwrap_or(1)
    }
}

/// A command handler closure. Rc-wrapped to allow cheap cloning out of the
/// registry for dispatch without holding a long-lived borrow.
pub type CommandHandler = Rc<dyn Fn(&mut MdAppState, CommandArgs)>;

/// What kind of interactive input (if any) a command needs before running.
#[derive(Clone, Debug)]
pub enum InteractiveSpec {
    /// No arguments needed — just run.
    None,
    /// Prompts for a file path via the mini-buffer's FindFile prompt.
    File { prompt: String },
    /// Prompts for a buffer name via the mini-buffer's SwitchBuffer prompt.
    Buffer { prompt: String },
    /// Prompts for a free-text string via the mini-buffer's FreeText prompt.
    String { prompt: String },
    /// Prompts for a line number.
    Line,
    /// Prompts for yes/no confirmation.
    Confirm { prompt: String },
}

/// Command category for M-x filtering and organisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandCategory {
    Navigation,
    Editing,
    Buffer,
    File,
    View,
    Search,
    Macro,
    Custom,
}

/// A named command.
#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub category: CommandCategory,
    pub handler: CommandHandler,
    pub interactive: InteractiveSpec,
}

/// Registry of all available commands.
pub struct CommandRegistry {
    commands: HashMap<String, Command>,
    /// Insertion order for deterministic listing.
    order: Vec<String>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Register a command. Replaces any existing command with the same name.
    pub fn register(&mut self, cmd: Command) {
        if !self.commands.contains_key(&cmd.name) {
            self.order.push(cmd.name.clone());
        }
        self.commands.insert(cmd.name.clone(), cmd);
    }

    /// Convenience helper: register a Rust command with a closure.
    pub fn register_fn(
        &mut self,
        name: &str,
        description: &str,
        category: CommandCategory,
        interactive: InteractiveSpec,
        handler: impl Fn(&mut MdAppState, CommandArgs) + 'static,
    ) {
        self.register(Command {
            name: name.to_string(),
            description: description.to_string(),
            category,
            handler: Rc::new(handler),
            interactive,
        });
    }

    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands.get(name)
    }

    /// All command names in registration order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.order.iter().map(|s| s.as_str())
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Filter command names by substring match.
    pub fn filter(&self, query: &str) -> Vec<String> {
        let q = query.to_lowercase();
        self.order
            .iter()
            .filter(|name| q.is_empty() || name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }
}

/// Register all built-in Rust commands on a fresh registry.
/// Called once from `MdAppState::new()`.
pub fn register_builtin_commands(registry: &mut CommandRegistry) {
    use CommandCategory::*;

    // ---- Navigation ----
    registry.register_fn(
        "forward-char",
        "Move cursor one character right",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_right(false);
            }
        },
    );
    registry.register_fn(
        "backward-char",
        "Move cursor one character left",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_left(false);
            }
        },
    );
    registry.register_fn(
        "next-line",
        "Move cursor down one line",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_down(false);
            }
        },
    );
    registry.register_fn(
        "previous-line",
        "Move cursor up one line",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_up(false);
            }
        },
    );
    registry.register_fn(
        "forward-word",
        "Move forward by one word",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_word_right(false);
            }
        },
    );
    registry.register_fn(
        "backward-word",
        "Move backward by one word",
        Navigation,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.move_word_left(false);
            }
        },
    );
    registry.register_fn(
        "beginning-of-line",
        "Move to the beginning of the current line",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_line_start(false),
    );
    registry.register_fn(
        "end-of-line",
        "Move to the end of the current line",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_line_end(false),
    );
    registry.register_fn(
        "beginning-of-buffer",
        "Move to the beginning of the document",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_doc_start(false),
    );
    registry.register_fn(
        "end-of-buffer",
        "Move to the end of the document",
        Navigation,
        InteractiveSpec::None,
        |s, _| s.move_to_doc_end(false),
    );
    registry.register_fn(
        "goto-line",
        "Jump to a given line number",
        Navigation,
        InteractiveSpec::Line,
        |s, args| {
            if let Some(line) = args.string.as_ref().and_then(|x| x.parse::<usize>().ok()) {
                s.jump_to_line(line);
            }
        },
    );

    // ---- Editing ----
    registry.register_fn(
        "delete-char",
        "Delete the character at point",
        Editing,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.delete_forward();
            }
        },
    );
    registry.register_fn(
        "backward-delete-char",
        "Delete the character before point",
        Editing,
        InteractiveSpec::None,
        |s, args| {
            for _ in 0..args.count() {
                s.backspace();
            }
        },
    );
    registry.register_fn(
        "undo",
        "Undo the last edit",
        Editing,
        InteractiveSpec::None,
        |s, _| s.undo(),
    );
    registry.register_fn(
        "redo",
        "Redo the last undone edit",
        Editing,
        InteractiveSpec::None,
        |s, _| s.redo(),
    );
    registry.register_fn(
        "kill-line",
        "Kill text from point to end of line",
        Editing,
        InteractiveSpec::None,
        |s, _| s.kill_to_end_of_line(),
    );
    registry.register_fn(
        "kill-word",
        "Kill the next word",
        Editing,
        InteractiveSpec::None,
        |s, _| s.kill_word_forward(),
    );
    registry.register_fn(
        "backward-kill-word",
        "Kill the previous word",
        Editing,
        InteractiveSpec::None,
        |s, _| s.kill_word_backward(),
    );
    registry.register_fn(
        "yank",
        "Yank (paste) the last killed text",
        Editing,
        InteractiveSpec::None,
        |s, _| s.yank(),
    );
    registry.register_fn(
        "yank-pop",
        "Replace yanked text with the previous kill ring entry",
        Editing,
        InteractiveSpec::None,
        |s, _| s.yank_pop(),
    );
    registry.register_fn(
        "upcase-word",
        "Uppercase the next word",
        Editing,
        InteractiveSpec::None,
        |s, _| s.upcase_word(),
    );
    registry.register_fn(
        "downcase-word",
        "Lowercase the next word",
        Editing,
        InteractiveSpec::None,
        |s, _| s.downcase_word(),
    );
    registry.register_fn(
        "transpose-chars",
        "Transpose the two characters around point",
        Editing,
        InteractiveSpec::None,
        |s, _| s.transpose_chars(),
    );
    registry.register_fn(
        "transpose-words",
        "Transpose the two words around point",
        Editing,
        InteractiveSpec::None,
        |s, _| s.transpose_words(),
    );
    registry.register_fn(
        "set-mark",
        "Set the mark at point",
        Editing,
        InteractiveSpec::None,
        |s, _| s.set_mark(),
    );
    registry.register_fn(
        "exchange-point-and-mark",
        "Exchange the cursor and the mark",
        Editing,
        InteractiveSpec::None,
        |s, _| s.exchange_point_and_mark(),
    );

    // ---- Buffer ----
    registry.register_fn(
        "switch-to-buffer",
        "Switch to another buffer",
        Buffer,
        InteractiveSpec::Buffer {
            prompt: "Switch to buffer".to_string(),
        },
        |s, args| {
            if let Some(name) = args.string.as_ref() {
                s.switch_to_buffer_by_name(name);
            }
        },
    );
    registry.register_fn(
        "kill-buffer",
        "Kill a buffer",
        Buffer,
        InteractiveSpec::Buffer {
            prompt: "Kill buffer".to_string(),
        },
        |s, args| {
            if let Some(name) = args.string.as_ref() {
                if name.is_empty() {
                    s.kill_current_buffer();
                } else {
                    s.kill_buffer_by_name(name);
                }
            }
        },
    );
    registry.register_fn(
        "next-buffer",
        "Switch to the next buffer in the list",
        Buffer,
        InteractiveSpec::None,
        |s, _| s.switch_to_next_buffer(),
    );
    registry.register_fn(
        "previous-buffer",
        "Switch to the previous buffer in the list",
        Buffer,
        InteractiveSpec::None,
        |s, _| s.switch_to_prev_buffer(),
    );

    // ---- File ----
    registry.register_fn(
        "find-file",
        "Open a file in a new buffer",
        File,
        InteractiveSpec::File {
            prompt: "Find file".to_string(),
        },
        |s, args| {
            if let Some(path_str) = args.string.as_ref() {
                let path = std::path::PathBuf::from(path_str);
                if let Ok(content) = std::fs::read_to_string(&path) {
                    s.open_file_as_buffer(path, &content);
                }
            }
        },
    );
    registry.register_fn(
        "dired",
        "Open a directory browser",
        File,
        InteractiveSpec::None,
        |s, _| {
            let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/"));
            s.dired_open(dir);
        },
    );

    // ---- View ----
    registry.register_fn(
        "toggle-preview",
        "Toggle the preview pane",
        View,
        InteractiveSpec::None,
        |s, _| s.show_preview = !s.show_preview,
    );
    registry.register_fn(
        "toggle-line-numbers",
        "Toggle editor line numbers",
        View,
        InteractiveSpec::None,
        |s, _| s.show_line_numbers = !s.show_line_numbers,
    );
    registry.register_fn(
        "toggle-preview-line-numbers",
        "Toggle preview line numbers",
        View,
        InteractiveSpec::None,
        |s, _| s.show_preview_line_numbers = !s.show_preview_line_numbers,
    );

    // ---- Search ----
    registry.register_fn(
        "isearch-forward",
        "Incremental search forward",
        Search,
        InteractiveSpec::None,
        |s, _| s.isearch_start(crate::state::IsearchDirection::Forward),
    );
    registry.register_fn(
        "isearch-backward",
        "Incremental search backward",
        Search,
        InteractiveSpec::None,
        |s, _| s.isearch_start(crate::state::IsearchDirection::Backward),
    );
    registry.register_fn(
        "find",
        "Open the find bar",
        Search,
        InteractiveSpec::None,
        |s, _| s.toggle_find(),
    );
    registry.register_fn(
        "replace",
        "Open the find-and-replace bar",
        Search,
        InteractiveSpec::None,
        |s, _| s.toggle_find_replace(),
    );

    // ---- Macros ----
    registry.register_fn(
        "start-kbd-macro",
        "Start recording a keyboard macro",
        Macro,
        InteractiveSpec::None,
        |s, _| s.macro_start_recording(),
    );
    registry.register_fn(
        "end-kbd-macro",
        "Stop recording the keyboard macro",
        Macro,
        InteractiveSpec::None,
        |s, _| s.macro_stop_recording(),
    );
    registry.register_fn(
        "call-last-kbd-macro",
        "Execute the last recorded keyboard macro",
        Macro,
        InteractiveSpec::None,
        |s, args| s.macro_execute_last(args.count()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut r = CommandRegistry::new();
        r.register_fn(
            "test-cmd",
            "A test command",
            CommandCategory::Custom,
            InteractiveSpec::None,
            |_s, _| {},
        );
        assert!(r.get("test-cmd").is_some());
        assert_eq!(r.get("test-cmd").unwrap().description, "A test command");
    }

    #[test]
    fn names_in_insertion_order() {
        let mut r = CommandRegistry::new();
        r.register_fn(
            "b",
            "",
            CommandCategory::Custom,
            InteractiveSpec::None,
            |_, _| {},
        );
        r.register_fn(
            "a",
            "",
            CommandCategory::Custom,
            InteractiveSpec::None,
            |_, _| {},
        );
        let names: Vec<&str> = r.names().collect();
        assert_eq!(names, vec!["b", "a"]);
    }

    #[test]
    fn filter_by_substring() {
        let mut r = CommandRegistry::new();
        r.register_fn(
            "forward-char",
            "",
            CommandCategory::Navigation,
            InteractiveSpec::None,
            |_, _| {},
        );
        r.register_fn(
            "forward-word",
            "",
            CommandCategory::Navigation,
            InteractiveSpec::None,
            |_, _| {},
        );
        r.register_fn(
            "backward-char",
            "",
            CommandCategory::Navigation,
            InteractiveSpec::None,
            |_, _| {},
        );

        let fwd = r.filter("forward");
        assert_eq!(fwd.len(), 2);
        let ch = r.filter("char");
        assert_eq!(ch.len(), 2);
    }

    #[test]
    fn builtin_registration_populates() {
        let mut r = CommandRegistry::new();
        register_builtin_commands(&mut r);
        assert!(r.len() > 20, "expected many built-in commands");
        assert!(r.get("forward-char").is_some());
        assert!(r.get("find-file").is_some());
        assert!(r.get("dired").is_some());
    }

    #[test]
    fn command_args_count_default() {
        let args = CommandArgs::default();
        assert_eq!(args.count(), 1);
        let args = CommandArgs::default().with_prefix(5);
        assert_eq!(args.count(), 5);
    }
}
