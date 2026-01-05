impl PlayerView {
    fn cycle_sort_order(&mut self, _: &CycleSortOrder, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            use crate::app::LibrarySortOrder;
            let next_order = match state.app.library_sort_order {
                LibrarySortOrder::Year => LibrarySortOrder::Genre,
                LibrarySortOrder::Genre => LibrarySortOrder::Artist,
                LibrarySortOrder::Artist => LibrarySortOrder::Album,
                LibrarySortOrder::Album => LibrarySortOrder::Tracks,
                LibrarySortOrder::Tracks => LibrarySortOrder::Composer,
                LibrarySortOrder::Composer => LibrarySortOrder::Popularity,
                LibrarySortOrder::Popularity => LibrarySortOrder::Year,
            };
            state.app.set_library_sort_order(next_order);
        });
        cx.notify();
    }

    fn set_sort_artist(&mut self, _: &SetSortArtist, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Artist);
        });
        cx.notify();
    }

    fn set_sort_album(&mut self, _: &SetSortAlbum, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Album);
        });
        cx.notify();
    }

    fn set_sort_title(&mut self, _: &SetSortTitle, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Title sorting now uses Album sort (sorts by album title)
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Album);
        });
        cx.notify();
    }

    fn set_sort_year(&mut self, _: &SetSortYear, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_library_sort_order(crate::app::LibrarySortOrder::Year);
        });
        cx.notify();
    }

    fn cycle_channel_filter(
        &mut self,
        _: &CycleChannelFilter,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            // Block if in text input mode
            if Self::is_text_input_mode(state.app.input_mode) {
                return;
            }
            state.app.cycle_channel_filter();
        });
        cx.notify();
    }

    fn set_filter_all(&mut self, _: &SetFilterAll, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_channel_filter(crate::app::ChannelFilter::All);
        });
        cx.notify();
    }

    fn set_filter_mono(&mut self, _: &SetFilterMono, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Mono);
        });
        cx.notify();
    }

    fn set_filter_stereo(&mut self, _: &SetFilterStereo, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Stereo);
        });
        cx.notify();
    }

    fn set_filter_surround(
        &mut self,
        _: &SetFilterSurround,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Surround);
        });
        cx.notify();
    }

    fn set_filter_surround71(
        &mut self,
        _: &SetFilterSurround71,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Surround71);
        });
        cx.notify();
    }

    fn set_filter_surround_plus(
        &mut self,
        _: &SetFilterSurroundPlus,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::SurroundPlus);
        });
        cx.notify();
    }

    fn set_filter_mixed(&mut self, _: &SetFilterMixed, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state
                .app
                .set_channel_filter(crate::app::ChannelFilter::Mixed);
        });
        cx.notify();
    }

    /// Check if we're in a text input mode where actions should be blocked
    fn is_text_input_mode(input_mode: crate::app::InputMode) -> bool {
        use crate::app::InputMode;
        matches!(
            input_mode,
            InputMode::Search
                | InputMode::AddDirectory
                | InputMode::SavePlugins
                | InputMode::LoadPlugins
                | InputMode::LoadApoFile
                | InputMode::LoadSofaFile
                | InputMode::SpinoramaSpeakerSearch
        )
    }

    fn handle_search_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for search mode
        match event.keystroke.key.as_str() {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.search_query.pop();
                    state.app.selected_album_index = 0;
                    state.app.reset_page();
                });
                cx.notify();
            }
            "escape" => {
                // Exit search mode and clear search
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                    state.app.search_query.clear();
                    state.app.selected_album_index = 0;
                    state.app.reset_page();
                });
                cx.notify();
            }
            "enter" => {
                // Exit search mode but keep search results
                self.state.update(cx, |state, _cx| {
                    state.app.input_mode = crate::app::InputMode::Normal;
                });
                cx.notify();
            }
            _ => {
                // Add character to search query
                if let Some(text) = event.keystroke.key_char.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.search_query.push_str(text);
                        state.app.selected_album_index = 0;
                        state.app.reset_page();
                    });
                    cx.notify();
                }
            }
        }
    }

}
