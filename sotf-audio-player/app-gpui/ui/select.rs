impl PlayerView {
    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            match state.app.current_screen {
                Screen::Library => {
                    state.app.select_next_album();
                }
                Screen::Queue => {
                    state.app.select_next_queue_item();
                }
                _ => {}
            }
        });
        cx.notify();
    }

    fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            match state.app.current_screen {
                Screen::Library => {
                    state.app.select_previous_album();
                }
                Screen::Queue => {
                    state.app.select_previous_queue_item();
                }
                _ => {}
            }
        });
        cx.notify();
    }

    fn select_next_page(&mut self, _: &SelectNextPage, _: &mut Window, cx: &mut Context<Self>) {
        // Grid uses rows × columns for page size
        const GRID_COLUMNS: usize = 7;
        const GRID_PAGE_ROWS: usize = 3;
        const LIST_PAGE_SIZE: usize = 20;

        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    // Grid view: move by full rows
                    state.app.page_down_albums(GRID_COLUMNS * GRID_PAGE_ROWS);
                }
                Screen::Queue => {
                    state.app.page_down_queue(LIST_PAGE_SIZE);
                }
                _ => {}
            });
        cx.notify();
    }

    fn select_prev_page(&mut self, _: &SelectPrevPage, _: &mut Window, cx: &mut Context<Self>) {
        // Grid uses rows × columns for page size
        const GRID_COLUMNS: usize = 7;
        const GRID_PAGE_ROWS: usize = 3;
        const LIST_PAGE_SIZE: usize = 20;

        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    // Grid view: move by full rows
                    state.app.page_up_albums(GRID_COLUMNS * GRID_PAGE_ROWS);
                }
                Screen::Queue => {
                    state.app.page_up_queue(LIST_PAGE_SIZE);
                }
                _ => {}
            });
        cx.notify();
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_left();
            }
        });
        cx.notify();
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_right();
            }
        });
        cx.notify();
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_up();
            }
        });
        cx.notify();
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.select_grid_down();
            }
        });
        cx.notify();
    }
}
