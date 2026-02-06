impl PlayerView {
    fn handle_apo_file_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for APO file loading mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.input_state.apo_file_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // File autocomplete support
                self.state.update(cx, |state, _cx| {
                    if state.app.input_state.autocomplete_suggestions.is_empty() {
                        state.app.generate_autocomplete_suggestions_for_apo_file();
                        if !state.app.input_state.autocomplete_suggestions.is_empty() {
                            state.app.apply_autocomplete_to_apo_file();
                        }
                    } else {
                        state.app.next_autocomplete_for_apo_file();
                    }
                });
                cx.notify();
            }
            "escape" => {
                // Already handled by Cancel action
                self.state.update(cx, |state, _cx| {
                    state.app.clear_autocomplete();
                });
            }
            "enter" => {
                // Load the APO file
                self.state.update(cx, |state, _cx| {
                    state.app.clear_autocomplete();
                    match state.app.load_apo_file() {
                        Ok(()) => {
                            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                                "APO file loaded successfully",
                            ));
                            state.app.input_state.apo_file_input.clear();
                            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                        }
                        Err(e) => {
                            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Failed to load APO file: {}", e),
                            ));
                        }
                    }
                });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.input_state.apo_file_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_sofa_file_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for SOFA file loading mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.input_state.sofa_file_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // File autocomplete support
                self.state.update(cx, |state, _cx| {
                    if state.app.input_state.autocomplete_suggestions.is_empty() {
                        state.app.generate_autocomplete_suggestions_for_sofa_file();
                        if !state.app.input_state.autocomplete_suggestions.is_empty() {
                            state.app.apply_autocomplete_to_sofa_file();
                        }
                    } else {
                        state.app.next_autocomplete_for_sofa_file();
                    }
                });
                cx.notify();
            }
            "escape" => {
                // Already handled by Cancel action
                self.state.update(cx, |state, _cx| {
                    state.app.clear_autocomplete();
                });
            }
            "enter" => {
                // Load the SOFA file
                self.state.update(cx, |state, _cx| {
                    state.app.clear_autocomplete();
                    match state.app.load_sofa_file() {
                        Ok(()) => {
                            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::success(
                                "SOFA file loaded successfully",
                            ));
                            state.app.input_state.sofa_file_input.clear();
                            state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                        }
                        Err(e) => {
                            state.app.ui_state.toast_message = Some(crate::app::ToastMessage::error(
                                format!("Failed to load SOFA file: {}", e),
                            ));
                        }
                    }
                });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.input_state.sofa_file_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    pub(crate) fn handle_spinorama_speaker_search_input(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) {
        log::info!(
            "[SPINORAMA] handle_spinorama_speaker_search_input called, key={}",
            event.keystroke.key
        );
        // Handle text input for spinorama speaker search mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                log::info!("[SPINORAMA] Backspace pressed");
                self.state.update(cx, |state, _cx| {
                    state.app.measurement_state.spinorama_eq_state.speaker_search.pop();
                    state.app.measurement_state.spinorama_eq_state.update_suggestions();
                });
                cx.notify();
            }
            "escape" => {
                log::info!("[SPINORAMA] Escape pressed - exiting search mode");
                // Exit search mode
                self.state.update(cx, |state, _cx| {
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                });
                cx.notify();
            }
            "enter" => {
                log::info!("[SPINORAMA] Enter pressed - exiting search mode");
                // Exit search mode, keep current search results
                self.state.update(cx, |state, _cx| {
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                });
                cx.notify();
            }
            _ => {
                // Add character to search query using key_char (handles all printable chars including space)
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    log::info!("[SPINORAMA] Character typed: '{}'", text);
                    self.state.update(cx, |state, _cx| {
                        state.app.measurement_state.spinorama_eq_state.speaker_search.push_str(text);
                        state.app.measurement_state.spinorama_eq_state.update_suggestions();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_save_plugins_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for save plugins mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.input_state.plugin_file_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // Autocomplete from available presets
                self.state.update(cx, |state, _cx| {
                    if state.app.input_state.autocomplete_suggestions.is_empty() {
                        state
                            .app
                            .generate_autocomplete_suggestions_for_save_preset();
                        if !state.app.input_state.autocomplete_suggestions.is_empty() {
                            state.app.apply_autocomplete_to_plugin_file();
                        }
                    } else {
                        state.app.next_autocomplete_for_plugin_file();
                    }
                });
                cx.notify();
            }
            "escape" => {
                self.state.update(cx, |state, _cx| {
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    state.app.input_state.plugin_file_input.clear();
                    state.app.clear_autocomplete();
                    state.app.ui_state.pending_studio_close = false; // Cancel close if save cancelled
                });
                cx.notify();
            }
            "enter" => {
                self.state.update(cx, |state, _cx| {
                    // If there are presets shown and input is empty, use selected preset (overwrite)
                    if state.app.input_state.plugin_file_input.is_empty()
                        && !state.app.plugin_state.available_plugin_presets.is_empty()
                    {
                        state.app.save_selected_preset();
                    } else if !state.app.input_state.plugin_file_input.is_empty() {
                        state.app.save_plugin_chain();
                    }
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    state.app.clear_autocomplete();
                    state.app.plugin_state.plugin_chain_modified = false;

                    if state.app.ui_state.pending_studio_close {
                        state.app.ui_state.pending_studio_close = false;
                        state.app.ui_state.current_screen = state.app.ui_state.last_screen;
                    }
                });
                cx.notify();
            }
            "up" => {
                // Navigate preset list when input is empty
                self.state.update(cx, |state, _cx| {
                    if state.app.input_state.plugin_file_input.is_empty()
                        && !state.app.plugin_state.available_plugin_presets.is_empty()
                    {
                        state.app.select_previous_preset();
                    }
                });
                cx.notify();
            }
            "down" => {
                // Navigate preset list when input is empty
                self.state.update(cx, |state, _cx| {
                    if state.app.input_state.plugin_file_input.is_empty()
                        && !state.app.plugin_state.available_plugin_presets.is_empty()
                    {
                        state.app.select_next_preset();
                    }
                });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.input_state.plugin_file_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_load_plugins_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for load plugins mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.input_state.plugin_file_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // Autocomplete file path
                self.state.update(cx, |state, _cx| {
                    if !state.app.input_state.plugin_file_input.is_empty() {
                        if state.app.input_state.autocomplete_suggestions.is_empty() {
                            state
                                .app
                                .generate_autocomplete_suggestions_for_plugin_file();
                            if !state.app.input_state.autocomplete_suggestions.is_empty() {
                                state.app.apply_autocomplete_to_plugin_file();
                            }
                        } else {
                            state.app.next_autocomplete_for_plugin_file();
                        }
                    }
                });
                cx.notify();
            }
            "escape" => {
                self.state.update(cx, |state, _cx| {
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    state.app.input_state.plugin_file_input.clear();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "enter" => {
                self.state.update(cx, |state, _cx| {
                    // If there are presets shown and input is empty, load selected preset
                    if state.app.input_state.plugin_file_input.is_empty()
                        && !state.app.plugin_state.available_plugin_presets.is_empty()
                    {
                        state.app.load_selected_preset();
                    } else if !state.app.input_state.plugin_file_input.is_empty() {
                        state.app.load_plugin_chain();
                    }
                    state.app.ui_state.input_mode = crate::app::InputMode::Normal;
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "up" | "k" => {
                // Navigate through presets
                self.state.update(cx, |state, _cx| {
                    if state.app.input_state.plugin_file_input.is_empty() {
                        state.app.select_previous_preset();
                    }
                });
                cx.notify();
            }
            "down" | "j" => {
                // Navigate through presets
                self.state.update(cx, |state, _cx| {
                    if state.app.input_state.plugin_file_input.is_empty() {
                        state.app.select_next_preset();
                    }
                });
                cx.notify();
            }
            _ => {
                // Add character to input
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.input_state.plugin_file_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;

            // Block action if in text input modes (where typing should take priority)
            match state.app.ui_state.input_mode {
                InputMode::Search
                | InputMode::SavePlugins
                | InputMode::LoadPlugins
                | InputMode::LoadApoFile
                | InputMode::LoadSofaFile => {
                    // Don't execute action - these modes handle Enter themselves via keyboard handlers
                    return;
                }
                InputMode::AddDirectory => {
                    // Add the directory
                    if !state.app.input_state.directory_input.is_empty() {
                        let path = std::path::PathBuf::from(&state.app.input_state.directory_input);
                        state.app.add_directory(path);
                        state.app.input_state.directory_input.clear();
                        state.app.clear_autocomplete();
                    }
                    state.app.ui_state.input_mode = InputMode::Normal;
                    return;
                }
                _ => {
                    // Continue to handle screen-specific actions
                }
            }

            // Handle screen-specific actions in Normal mode
            match state.app.ui_state.current_screen {
                Screen::Library => {
                    // Add selected album to queue
                    if let Some(path) = state.app.add_album_to_queue() {
                        Self::play_track(state, path);
                    }
                }
                Screen::Queue => {
                    // Play selected album in queue
                    if let Some(path) = state.app.play_selected_queue_item() {
                        Self::play_track(state, path);
                    }
                }
                Screen::Settings => {
                    // Enter key in Settings screen - no action needed
                }
                _ => {}
            }
        });
        cx.notify();
    }

        fn handle_directory_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {

            // ... (existing code)

            match event.keystroke.key.as_str() {

                "backspace" => {

                    self.state.update(cx, |state, _cx| {

                        state.app.input_state.directory_input.pop();

                        state.app.clear_autocomplete();

                    });

                    cx.notify();

                }

                "tab" => {

                    // Tab autocomplete

                    self.state.update(cx, |state, _cx| {

                        if state.app.input_state.autocomplete_suggestions.is_empty() {

                            state.app.generate_autocomplete_suggestions();

                        } else {

                            state.app.next_autocomplete();

                        }

                    });

                    cx.notify();

                }

                "escape" => {

                    // Already handled by Cancel action

                }

                "enter" => {

                    // Already handled by Enter action (adds directory)

                }

                _ => {

                    // Add character to directory input

                    if let Some(text) = event.keystroke.key_char.as_ref() {

                        self.state.update(cx, |state, _cx| {

                            state.app.input_state.directory_input.push_str(text);

                            state.app.clear_autocomplete();

                        });

                        cx.notify();

                    }

                }

            }

        }

    

        pub(crate) fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {

            let screen = self.state.read(cx).app.ui_state.current_screen;

            match screen {

                Screen::Library => self.state.update(cx, |state, _cx| state.app.select_next_album()),

                Screen::Queue => self.state.update(cx, |state, _cx| state.app.select_next_queue_item()),

                Screen::Settings => {

                    self.state.update(cx, |state, _cx| state.app.select_next_directory())

                }

                _ => {}

            }

            cx.notify();

        }

    

        pub(crate) fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {

            let screen = self.state.read(cx).app.ui_state.current_screen;

            match screen {

                Screen::Library => self.state.update(cx, |state, _cx| state.app.select_previous_album()),

                Screen::Queue => {

                    self.state.update(cx, |state, _cx| state.app.select_previous_queue_item())

                }

                Screen::Settings => {

                    self.state.update(cx, |state, _cx| state.app.select_previous_directory())

                }

                _ => {}

            }

            cx.notify();

        }

    

        pub(crate) fn select_next_page(

            &mut self,

            _: &SelectNextPage,

            _: &mut Window,

            cx: &mut Context<Self>,

        ) {

            let screen = self.state.read(cx).app.ui_state.current_screen;

            match screen {

                Screen::Library => self.state.update(cx, |state, _cx| {

                    let page_size = state.app.library_state.items_per_page;

                    state.app.page_up_albums(page_size); // Corrected from page_down which was removed

                }),

                Screen::Queue => self.state.update(cx, |state, _cx| {

                    state.app.page_down_queue(10);

                }),

                Screen::Settings => self.state.update(cx, |state, _cx| {

                    state.app.page_down_directories(10);

                }),

                _ => {}

            }

            cx.notify();

        }

    

        pub(crate) fn select_prev_page(

            &mut self,

            _: &SelectPrevPage,

            _: &mut Window,

            cx: &mut Context<Self>,

        ) {

            let screen = self.state.read(cx).app.ui_state.current_screen;

            match screen {

                Screen::Library => self.state.update(cx, |state, _cx| {

                    let page_size = state.app.library_state.items_per_page;

                    state.app.page_up_albums(page_size);

                }),

                Screen::Queue => self.state.update(cx, |state, _cx| {

                    state.app.page_up_queue(10);

                }),

                Screen::Settings => self.state.update(cx, |state, _cx| {

                    state.app.page_up_directories(10);

                }),

                _ => {}

            }

            cx.notify();

        }

    

        pub(crate) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {

            let (screen, cols) = {

                let state = self.state.read(cx);

                let app = &state.app;

                (app.ui_state.current_screen, app.library_state.library_columns)

            };

            if screen == Screen::Library {

                self.state.update(cx, |state, _cx| {

                    state.app.library_state.select_grid_left(cols);

                });

            }

            cx.notify();

        }

    

        pub(crate) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
            let (screen, cols) = {
                let state = self.state.read(cx);
                let app = &state.app;
                (app.ui_state.current_screen, app.library_state.library_columns)
            };

            if screen == Screen::Library {
                let mut should_load_more = false;
                self.state.update(cx, |state, _cx| {
                    let total = state.app.filtered_albums().len();
                    if state.app.library_state.selected_index < total - 1 {
                        state.app.library_state.select_grid_right(cols);
                    } else {
                        should_load_more = true;
                    }
                });

                if should_load_more {
                    self.load_more_albums(cx);
                }
            }
            cx.notify();
        }

    

        pub(crate) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {

            let (screen, cols) = {

                let state = self.state.read(cx);

                let app = &state.app;

                (app.ui_state.current_screen, app.library_state.library_columns)

            };

            if screen == Screen::Library {

                self.state.update(cx, |state, _cx| {

                    state.app.library_state.select_grid_up(cols);

                });

            }

            cx.notify();

        }

    

        pub(crate) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
            let (screen, cols) = {
                let state = self.state.read(cx);
                let app = &state.app;
                (app.ui_state.current_screen, app.library_state.library_columns)
            };

            if screen == Screen::Library {
                let mut should_load_more = false;
                self.state.update(cx, |state, _cx| {
                    let current_count = state.app.get_paginated_albums().len();
                    let next_row_index = state.app.library_state.selected_index + cols;

                    if next_row_index < current_count {
                        state.app.library_state.select_grid_down(cols);
                    } else {
                        should_load_more = true;
                    }
                });

                if should_load_more {
                    self.load_more_albums(cx);
                    // After load more, we try to move down again
                    self.state.update(cx, |state, _cx| {
                        let total = state.app.filtered_albums().len();
                        if state.app.library_state.selected_index + cols < total {
                            state.app.library_state.select_grid_down(cols);
                        } else {
                            state.app.library_state.selected_index = total.saturating_sub(1);
                        }
                    });
                }
            }
            cx.notify();
        }

    }

    
