use std::path::PathBuf;

use clap::Parser;
use gpui::*;
use gpui_keybinding::{KeybindingProvider, KeymapPreset};
use gpui_md::actions;
use gpui_md::state::MdAppState;
use gpui_md::views::main_view::MainView;
use gpui_md::MdKeybindingProvider;
use gpui_ui_kit::{MiniApp, MiniAppConfig};
use log::error;

#[derive(Parser)]
#[command(name = "gpui-md-editor", about = "Markdown editor with live preview")]
struct Cli {
    /// Markdown files to open as buffers in a single window
    files: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    let files = cli.files;

    MiniApp::run(
        MiniAppConfig::new("Markdown Editor")
            .size(1400.0, 900.0)
            .scrollable(false)
            .with_theme(true),
        move |cx| {
            // Register keybindings
            let provider = MdKeybindingProvider;
            let bindings = provider.bindings(KeymapPreset::Default);
            cx.bind_keys(bindings);

            // Create state — load first file into the active buffer, rest as
            // stored buffers. If no files, open with the default scratch-like
            // welcome buffer.
            let state = if let Some(first_file) = files.first() {
                match std::fs::read_to_string(first_file) {
                    Ok(content) => {
                        let canonical = std::fs::canonicalize(first_file)
                            .unwrap_or_else(|_| first_file.clone());
                        cx.new(|_| MdAppState::from_file(canonical, &content))
                    }
                    Err(e) => {
                        error!("Failed to open {}: {}", first_file.display(), e);
                        cx.new(|_| MdAppState::new())
                    }
                }
            } else {
                cx.new(|_| MdAppState::new())
            };

            // Create additional buffers for remaining files.
            state.update(cx, |s, _cx| {
                for file in files.iter().skip(1) {
                    match std::fs::read_to_string(file) {
                        Ok(content) => {
                            let canonical = std::fs::canonicalize(file)
                                .unwrap_or_else(|_| file.clone());
                            s.create_file_buffer(canonical, &content);
                        }
                        Err(e) => {
                            error!("Failed to open {}: {}", file.display(), e);
                        }
                    }
                }
            });

            let global_state = state.clone();
            cx.set_global(MdGlobalState(global_state));

            register_actions(cx);
            cx.set_menus(build_menus());

            cx.new(|cx| MainView::new(state, cx))
        },
    );
}

/// Global wrapper so we can access state from App-level action handlers.
struct MdGlobalState(Entity<MdAppState>);
impl Global for MdGlobalState {}

fn register_actions(cx: &mut App) {
    // File operations
    cx.on_action::<actions::NewFile>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| {
            s.document.set_text("");
            s.cursor.position = 0;
            s.cursor.clear_selection();
        });
    });

    cx.on_action::<actions::OpenFile>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        cx.spawn(async move |cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Markdown", &["md", "markdown", "txt"])
                .add_filter("All files", &["*"])
                .pick_file()
                .await;
            if let Some(file) = file {
                let path = file.path().to_path_buf();
                // Guard against excessively large files (50 MB)
                match std::fs::metadata(&path) {
                    Ok(meta) if meta.len() > 50 * 1024 * 1024 => {
                        error!("File too large: {} ({} bytes)", path.display(), meta.len());
                        rfd::AsyncMessageDialog::new()
                            .set_level(rfd::MessageLevel::Error)
                            .set_title("File Too Large")
                            .set_description(format!(
                                "Cannot open '{}': file is {:.1} MB (limit is 50 MB).",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                meta.len() as f64 / (1024.0 * 1024.0),
                            ))
                            .show()
                            .await;
                        return;
                    }
                    Err(e) => {
                        error!("Cannot read file metadata {}: {}", path.display(), e);
                        rfd::AsyncMessageDialog::new()
                            .set_level(rfd::MessageLevel::Error)
                            .set_title("Open Failed")
                            .set_description(format!("Cannot open '{}': {}", path.display(), e))
                            .show()
                            .await;
                        return;
                    }
                    _ => {}
                }
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        cx.update(|cx| {
                            state.update(cx, |s, _cx| {
                                // Open as a new buffer (or switch to existing one)
                                s.open_file_as_buffer(path.clone(), &content);
                                s.add_recent_file(path);
                            });
                        });
                    }
                    Err(e) => {
                        error!("Failed to open file {}: {}", path.display(), e);
                        rfd::AsyncMessageDialog::new()
                            .set_level(rfd::MessageLevel::Error)
                            .set_title("Open Failed")
                            .set_description(format!(
                                "Cannot open '{}': {}",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                e,
                            ))
                            .show()
                            .await;
                    }
                }
            }
        })
        .detach();
    });

    cx.on_action::<actions::SaveFile>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        let text = state.read(cx).document.text();
        let path = state.read(cx).document.file_path().cloned();

        if let Some(path) = path {
            match std::fs::write(&path, &text) {
                Ok(()) => state.update(cx, |s, _cx| s.document.mark_clean()),
                Err(e) => error!("Failed to save file {}: {}", path.display(), e),
            }
        } else {
            // No file path — trigger Save As
            cx.spawn(async move |cx| {
                let file = rfd::AsyncFileDialog::new()
                    .add_filter("Markdown", &["md"])
                    .save_file()
                    .await;
                if let Some(file) = file {
                    let path = file.path().to_path_buf();
                    match std::fs::write(&path, &text) {
                        Ok(()) => {
                            cx.update(|cx| {
                                state.update(cx, |s, _cx| {
                                    s.document.set_file_path(path.clone());
                                    s.document.mark_clean();
                                    s.add_recent_file(path);
                                });
                            });
                        }
                        Err(e) => error!("Failed to save file {}: {}", path.display(), e),
                    }
                }
            })
            .detach();
        }
    });

    cx.on_action::<actions::SaveFileAs>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        let text = state.read(cx).document.text();
        cx.spawn(async move |cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Markdown", &["md"])
                .save_file()
                .await;
            if let Some(file) = file {
                let path = file.path().to_path_buf();
                match std::fs::write(&path, &text) {
                    Ok(()) => {
                        cx.update(|cx| {
                            state.update(cx, |s, _cx| {
                                s.document.set_file_path(path.clone());
                                s.document.mark_clean();
                                s.add_recent_file(path);
                            });
                        });
                    }
                    Err(e) => error!("Failed to save file as {}: {}", path.display(), e),
                }
            }
        })
        .detach();
    });

    // Edit operations
    cx.on_action::<actions::Undo>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.undo());
    });

    cx.on_action::<actions::Redo>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.redo());
    });

    // Formatting
    cx.on_action::<actions::ToggleBold>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.toggle_format("**", "**"));
    });

    cx.on_action::<actions::ToggleItalic>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.toggle_format("*", "*"));
    });

    cx.on_action::<actions::ToggleCode>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.toggle_format("`", "`"));
    });

    cx.on_action::<actions::ToggleStrikethrough>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.toggle_format("~~", "~~"));
    });

    cx.on_action::<actions::InsertHeading1>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.insert_text("# "));
    });

    cx.on_action::<actions::InsertHeading2>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.insert_text("## "));
    });

    cx.on_action::<actions::InsertHeading3>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.insert_text("### "));
    });

    cx.on_action::<actions::InsertLink>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.toggle_format("[", "](url)"));
    });

    cx.on_action::<actions::InsertImage>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.insert_text("![alt text](image.png)"));
    });

    cx.on_action::<actions::InsertCodeBlock>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.insert_text("```\n\n```\n"));
    });

    cx.on_action::<actions::InsertTable>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| {
            s.insert_text(
                "| Column 1 | Column 2 |\n| -------- | -------- |\n| Cell 1   | Cell 2   |\n",
            );
        });
    });

    cx.on_action::<actions::InsertHorizontalRule>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.insert_text("\n---\n"));
    });

    cx.on_action::<actions::InsertTaskList>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.insert_text("- [ ] "));
    });

    // Font size
    cx.on_action::<actions::IncreaseFontSize>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| {
            s.font_size = (s.font_size + 1.0).min(48.0);
        });
    });

    cx.on_action::<actions::DecreaseFontSize>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| {
            s.font_size = (s.font_size - 1.0).max(8.0);
        });
    });

    // View
    cx.on_action::<actions::TogglePreview>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.show_preview = !s.show_preview);
    });

    cx.on_action::<actions::ToggleLineNumbers>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.show_line_numbers = !s.show_line_numbers);
    });

    // Keybindings preset selection
    cx.on_action::<actions::SetKeymapDefault>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        let bindings = MdKeybindingProvider.bindings(KeymapPreset::Default);
        cx.bind_keys(bindings);
        state.update(cx, |s, _cx| {
            s.keymap_preset = KeymapPreset::Default;
        });
    });

    cx.on_action::<actions::SetKeymapEmacs>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        let bindings = MdKeybindingProvider.bindings(KeymapPreset::Emacs);
        cx.bind_keys(bindings);
        state.update(cx, |s, _cx| {
            s.keymap_preset = KeymapPreset::Emacs;
        });
    });

    cx.on_action::<actions::SetKeymapVim>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        let bindings = MdKeybindingProvider.bindings(KeymapPreset::Vim);
        cx.bind_keys(bindings);
        state.update(cx, |s, _cx| {
            s.keymap_preset = KeymapPreset::Vim;
        });
    });

    // Find
    cx.on_action::<actions::Find>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.toggle_find());
    });

    cx.on_action::<actions::FindReplace>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.toggle_find_replace());
    });

    // Navigation
    cx.on_action::<actions::PageUp>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.page_up());
    });

    cx.on_action::<actions::PageDown>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.page_down());
    });

    cx.on_action::<actions::WordLeft>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.move_word_left(false));
    });

    cx.on_action::<actions::WordRight>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.move_word_right(false));
    });

    // Emacs commands
    cx.on_action::<actions::Abort>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.abort());
    });

    cx.on_action::<actions::UpcaseWord>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.upcase_word());
    });

    cx.on_action::<actions::DowncaseWord>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.downcase_word());
    });

    cx.on_action::<actions::ExchangePointAndMark>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.exchange_point_and_mark());
    });

    cx.on_action::<actions::CommandPalette>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.minibuffer_start_command());
    });

    cx.on_action::<actions::IsearchForward>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| {
            s.isearch_start(gpui_md::state::IsearchDirection::Forward);
        });
    });

    cx.on_action::<actions::IsearchBackward>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| {
            s.isearch_start(gpui_md::state::IsearchDirection::Backward);
        });
    });

    cx.on_action::<actions::Recenter>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.recenter());
    });

    cx.on_action::<actions::UniversalArgument>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        state.update(cx, |s, _cx| s.universal_argument());
    });

    // Import/Export
    cx.on_action::<actions::ImportDocx>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        cx.spawn(async move |cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Word Document", &["docx"])
                .pick_file()
                .await;
            if let Some(file) = file {
                let path = file.path().to_path_buf();
                match gpui_md::import::docx::import_docx(&path) {
                    Ok(md) => {
                        cx.update(|cx| {
                            state.update(cx, |s, _cx| {
                                s.document.set_text(&md);
                                s.cursor.position = 0;
                                s.cursor.clear_selection();
                            });
                        });
                    }
                    Err(e) => {
                        error!("Failed to import docx {}: {}", path.display(), e);
                        rfd::AsyncMessageDialog::new()
                            .set_level(rfd::MessageLevel::Error)
                            .set_title("Import Failed")
                            .set_description(format!(
                                "Cannot import '{}': {}",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                e,
                            ))
                            .show()
                            .await;
                    }
                }
            }
        })
        .detach();
    });

    cx.on_action::<actions::ExportDocx>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        let text = state.read(cx).document.text();
        cx.spawn(async move |_cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("Word Document", &["docx"])
                .save_file()
                .await;
            if let Some(file) = file {
                let path = file.path().to_path_buf();
                let _ = gpui_md::export::docx::export_docx(&text, &path);
            }
        })
        .detach();
    });

    #[cfg(feature = "pdf-export")]
    cx.on_action::<actions::ExportPdf>(|_action, cx| {
        let state = cx.global::<MdGlobalState>().0.clone();
        let text = state.read(cx).document.text();
        cx.spawn(async move |_cx| {
            let file = rfd::AsyncFileDialog::new()
                .add_filter("PDF", &["pdf"])
                .save_file()
                .await;
            if let Some(file) = file {
                let path = file.path().to_path_buf();
                let _ = gpui_md::export::pdf::export_pdf(&text, &path);
            }
        })
        .detach();
    });
}

fn build_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Markdown Editor".into(),
            items: vec![
                MenuItem::separator(),
                MenuItem::action("Quit Markdown Editor", gpui_ui_kit::app::miniapp::Quit),
            ],
            disabled: false,
        },
        {
            #[allow(unused_mut)]
            let mut file_items = vec![
                MenuItem::action("New       Cmd+N", actions::NewFile),
                MenuItem::action("Open...   Cmd+O", actions::OpenFile),
                MenuItem::separator(),
                MenuItem::action("Save      Cmd+S", actions::SaveFile),
                MenuItem::action("Save As...  Cmd+Shift+S", actions::SaveFileAs),
                MenuItem::separator(),
                MenuItem::action("Import Word (.docx)...", actions::ImportDocx),
                MenuItem::separator(),
                MenuItem::action("Export as Word (.docx)...", actions::ExportDocx),
            ];
            #[cfg(feature = "pdf-export")]
            file_items.push(MenuItem::action("Export as PDF...", actions::ExportPdf));
            Menu {
                name: "File".into(),
                items: file_items,
                disabled: false,
            }
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo    Cmd+Z", actions::Undo),
                MenuItem::action("Redo    Cmd+Shift+Z", actions::Redo),
                MenuItem::separator(),
                MenuItem::action("Find       Cmd+F", actions::Find),
                MenuItem::action("Find & Replace  Cmd+H", actions::FindReplace),
            ],
            disabled: false,
        },
        Menu {
            name: "Format".into(),
            items: vec![
                MenuItem::action("Bold           Cmd+B", actions::ToggleBold),
                MenuItem::action("Italic         Cmd+I", actions::ToggleItalic),
                MenuItem::action("Code           Cmd+E", actions::ToggleCode),
                MenuItem::action("Strikethrough  Cmd+Shift+X", actions::ToggleStrikethrough),
                MenuItem::separator(),
                MenuItem::action("Heading 1", actions::InsertHeading1),
                MenuItem::action("Heading 2", actions::InsertHeading2),
                MenuItem::action("Heading 3", actions::InsertHeading3),
                MenuItem::separator(),
                MenuItem::action("Insert Link    Cmd+K", actions::InsertLink),
                MenuItem::action("Insert Image", actions::InsertImage),
                MenuItem::action("Insert Code Block", actions::InsertCodeBlock),
                MenuItem::action("Insert Table", actions::InsertTable),
                MenuItem::action("Insert Horizontal Rule", actions::InsertHorizontalRule),
                MenuItem::action("Insert Task List", actions::InsertTaskList),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Preview  Cmd+Shift+V", actions::TogglePreview),
                MenuItem::action("Toggle Line Numbers", actions::ToggleLineNumbers),
                MenuItem::separator(),
                MenuItem::action("Increase Font Size  Cmd+=", actions::IncreaseFontSize),
                MenuItem::action("Decrease Font Size  Cmd+-", actions::DecreaseFontSize),
                MenuItem::separator(),
                MenuItem::submenu(Menu {
                    name: "Theme".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("Dark", gpui_ui_kit::app::miniapp::SetThemeDark),
                        MenuItem::action("Light", gpui_ui_kit::app::miniapp::SetThemeLight),
                        MenuItem::action("Midnight", gpui_ui_kit::app::miniapp::SetThemeMidnight),
                        MenuItem::action("Forest", gpui_ui_kit::app::miniapp::SetThemeForest),
                        MenuItem::action(
                            "Black & White",
                            gpui_ui_kit::app::miniapp::SetThemeBlackAndWhite,
                        ),
                        MenuItem::separator(),
                        MenuItem::action("Toggle Theme  Cmd+T", gpui_ui_kit::app::miniapp::ToggleTheme),
                    ],
                }),
                MenuItem::submenu(Menu {
                    name: "Design System".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("Neutral", gpui_ui_kit::app::miniapp::SetDesignNeutral),
                        MenuItem::action("Apple HIG", gpui_ui_kit::app::miniapp::SetDesignAppleHig),
                        MenuItem::action("Material 3", gpui_ui_kit::app::miniapp::SetDesignMaterial3),
                        MenuItem::action("Fluent", gpui_ui_kit::app::miniapp::SetDesignFluent),
                    ],
                }),
                MenuItem::submenu(Menu {
                    name: "Keybindings".into(),
                    disabled: false,
                    items: vec![
                        MenuItem::action("Default", actions::SetKeymapDefault),
                        MenuItem::action("Emacs", actions::SetKeymapEmacs),
                        MenuItem::action("Vim", actions::SetKeymapVim),
                    ],
                }),
            ],
            disabled: false,
        },
    ]
}
