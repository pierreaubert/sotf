use crate::app::{AppState, Screen};
use gpui::prelude::*;
use gpui::*;

// Actions for keyboard shortcuts
actions!(
    player_ui,
    [
        PlayPause,
        Stop,
        NextTrack,
        PrevTrack,
        VolumeUp,
        VolumeDown,
        SwitchToLibrary,
        SwitchToQueue,
        SwitchToPlugins,
        SwitchToDevices,
        SwitchToSpectrum,
        SwitchToDirectoryManager,
        ToggleSearch,
        ToggleLibraryView,
        ToggleHelp,
        CycleSortOrder,
        SetSortArtist,
        SetSortAlbum,
        SetSortTitle,
        SetSortYear,
        CycleChannelFilter,
        SetFilterAll,
        SetFilterMono,
        SetFilterStereo,
        SetFilterMultichannel,
        SetFilterMixed,
        SelectNext,
        SelectPrev,
        SelectNextPage,
        SelectPrevPage,
        ToggleExpand,
        Enter,
        Cancel,
        RemoveItem,
        ClearQueue,
        MovePluginUp,
        MovePluginDown,
        TogglePlugin,
        AddDirectory,
        ScanLibrary,
    ]
);

pub struct PlayerView {
    state: Entity<AppState>,
    focus_handle: FocusHandle,
}

impl PlayerView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // TODO: Set up periodic update timer for playback position and loudness
        // This requires fixing the async spawn pattern with GPUI
        // cx.spawn(|(this, mut cx)| async move {
        //     loop {
        //         smol::Timer::after(std::time::Duration::from_millis(100)).await;
        //         let _ = cx.update(|cx| {
        //             if let Some(view) = this.upgrade() {
        //                 view.update(cx, |view, cx| {
        //                     view.update_playback_state(cx);
        //                     cx.notify();
        //                 });
        //             }
        //         }).ok();
        //     }
        // })
        // .detach();

        Self {
            state,
            focus_handle,
        }
    }

    fn toggle_playback(&mut self, _: &PlayPause, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.is_playing {
                let _ = state.player.lock().pause();
                state.app.is_playing = false;
            } else {
                let _ = state.player.lock().resume();
                state.app.is_playing = true;
            }
        });
        cx.notify();
    }

    fn stop_playback(&mut self, _: &Stop, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            let _ = state.player.lock().stop();
            state.app.is_playing = false;
            state.app.current_queue_index = None;
        });
        cx.notify();
    }

    fn next_track(&mut self, _: &NextTrack, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(path) = state.app.next_track() {
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                let output_channels = state.app.plugin_chain.output_channels();

                if let Err(e) = state.player.lock().load_and_play(
                    path,
                    plugins,
                    output_channels,
                    state.app.current_output_device_name.clone(),
                ) {
                    log::error!("Failed to play next track: {}", e);
                    state.app.is_playing = false;
                }
            } else {
                state.app.is_playing = false;
            }
        });
        cx.notify();
    }

    fn prev_track(&mut self, _: &PrevTrack, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            if let Some(path) = state.app.previous_track() {
                let sample_rate = 48000.0;
                let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                let output_channels = state.app.plugin_chain.output_channels();

                if let Err(e) = state.player.lock().load_and_play(
                    path,
                    plugins,
                    output_channels,
                    state.app.current_output_device_name.clone(),
                ) {
                    log::error!("Failed to play previous track: {}", e);
                    state.app.is_playing = false;
                }
            } else {
                state.app.is_playing = false;
            }
        });
        cx.notify();
    }

    fn adjust_volume(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.volume = (state.app.volume + delta).clamp(0.0, 1.0);
            let _ = state.player.lock().set_volume(state.app.volume);
        });
        cx.notify();
    }

    fn volume_up(&mut self, _: &VolumeUp, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(0.05, cx);
    }

    fn volume_down(&mut self, _: &VolumeDown, _: &mut Window, cx: &mut Context<Self>) {
        self.adjust_volume(-0.05, cx);
    }

    fn switch_screen(&mut self, screen: Screen, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.current_screen = screen;
        });
        cx.notify();
    }

    fn switch_to_library(&mut self, _: &SwitchToLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Library, cx);
    }

    fn switch_to_queue(&mut self, _: &SwitchToQueue, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Queue, cx);
    }

    fn switch_to_plugins(&mut self, _: &SwitchToPlugins, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Plugins, cx);
    }

    fn switch_to_devices(&mut self, _: &SwitchToDevices, _: &mut Window, cx: &mut Context<Self>) {
        self.switch_screen(Screen::Devices, cx);
    }

    fn switch_to_directory_manager(
        &mut self,
        _: &SwitchToDirectoryManager,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switch_screen(Screen::DirectoryManager, cx);
    }

    fn toggle_search(&mut self, _: &ToggleSearch, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.input_mode == crate::app::InputMode::Search {
                state.app.input_mode = crate::app::InputMode::Normal;
            } else {
                state.app.input_mode = crate::app::InputMode::Search;
                state.app.search_query.clear();
            }
        });
        cx.notify();
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.input_mode = crate::app::InputMode::Normal;
            state.app.search_query.clear();
            state.app.directory_input.clear();
            state.app.clear_autocomplete();
        });
        cx.notify();
    }

    fn toggle_library_view(
        &mut self,
        _: &ToggleLibraryView,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_library_view_mode();
        });
        cx.notify();
    }

    fn toggle_help(&mut self, _: &ToggleHelp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            if state.app.input_mode == InputMode::Help {
                state.app.input_mode = InputMode::Normal;
            } else {
                state.app.input_mode = InputMode::Help;
            }
        });
        cx.notify();
    }

    fn cycle_sort_order(&mut self, _: &CycleSortOrder, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::LibrarySortOrder;
            let next_order = match state.app.library_sort_order {
                LibrarySortOrder::Artist => LibrarySortOrder::Album,
                LibrarySortOrder::Album => LibrarySortOrder::Title,
                LibrarySortOrder::Title => LibrarySortOrder::Year,
                LibrarySortOrder::Year => LibrarySortOrder::Artist,
            };
            state.app.set_library_sort_order(next_order);
        });
        cx.notify();
    }

    fn set_sort_artist(&mut self, _: &SetSortArtist, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_library_sort_order(crate::app::LibrarySortOrder::Artist);
        });
        cx.notify();
    }

    fn set_sort_album(&mut self, _: &SetSortAlbum, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_library_sort_order(crate::app::LibrarySortOrder::Album);
        });
        cx.notify();
    }

    fn set_sort_title(&mut self, _: &SetSortTitle, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_library_sort_order(crate::app::LibrarySortOrder::Title);
        });
        cx.notify();
    }

    fn set_sort_year(&mut self, _: &SetSortYear, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_library_sort_order(crate::app::LibrarySortOrder::Year);
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
            state.app.set_channel_filter(crate::app::ChannelFilter::Mono);
        });
        cx.notify();
    }

    fn set_filter_stereo(&mut self, _: &SetFilterStereo, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_channel_filter(crate::app::ChannelFilter::Stereo);
        });
        cx.notify();
    }

    fn set_filter_multichannel(
        &mut self,
        _: &SetFilterMultichannel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, _cx| {
            state.app.set_channel_filter(crate::app::ChannelFilter::Multichannel);
        });
        cx.notify();
    }

    fn set_filter_mixed(&mut self, _: &SetFilterMixed, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.set_channel_filter(crate::app::ChannelFilter::Mixed);
        });
        cx.notify();
    }

    fn handle_search_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for search mode
        match &event.keystroke.key {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.search_query.pop();
                    state.app.selected_album_index = 0; // Reset selection when query changes
                });
                cx.notify();
            }
            "escape" => {
                // Already handled by Cancel action
            }
            "enter" => {
                // Already handled by Enter action (exits search mode)
            }
            _ => {
                // Add character to search query
                if let Some(text) = event.keystroke.ime_key.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.search_query.push_str(text);
                        state.app.selected_album_index = 0; // Reset selection when query changes
                    });
                    cx.notify();
                }
            }
        }
    }

    fn handle_directory_input(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        // Handle text input for add directory mode
        match &event.keystroke.key {
            "backspace" => {
                self.state.update(cx, |state, _cx| {
                    state.app.directory_input.pop();
                    state.app.clear_autocomplete();
                });
                cx.notify();
            }
            "tab" => {
                // Tab autocomplete
                self.state.update(cx, |state, _cx| {
                    if state.app.autocomplete_suggestions.is_empty() {
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
                if let Some(text) = event.keystroke.ime_key.as_ref() {
                    self.state.update(cx, |state, _cx| {
                        state.app.directory_input.push_str(text);
                        state.app.clear_autocomplete();
                    });
                    cx.notify();
                }
            }
        }
    }

    fn select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    if state.app.library_view_mode == crate::app::LibraryViewMode::TreeView {
                        state.app.select_next_tree_item();
                    } else {
                        state.app.select_next_album();
                    }
                }
                Screen::Queue => state.app.select_next_queue_item(),
                Screen::DirectoryManager => state.app.select_next_directory(),
                _ => {}
            });
        cx.notify();
    }

    fn select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    if state.app.library_view_mode == crate::app::LibraryViewMode::TreeView {
                        state.app.select_previous_tree_item();
                    } else {
                        state.app.select_previous_album();
                    }
                }
                Screen::Queue => state.app.select_previous_queue_item(),
                Screen::DirectoryManager => state.app.select_previous_directory(),
                _ => {}
            });
        cx.notify();
    }

    fn select_next_page(&mut self, _: &SelectNextPage, _: &mut Window, cx: &mut Context<Self>) {
        const PAGE_SIZE: usize = 20;

        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    if state.app.library_view_mode == crate::app::LibraryViewMode::TreeView {
                        state.app.page_down_tree(PAGE_SIZE);
                    } else {
                        state.app.page_down_albums(PAGE_SIZE);
                    }
                }
                Screen::Queue => state.app.page_down_queue(PAGE_SIZE),
                Screen::DirectoryManager => state.app.page_down_directories(PAGE_SIZE),
                _ => {}
            });
        cx.notify();
    }

    fn select_prev_page(&mut self, _: &SelectPrevPage, _: &mut Window, cx: &mut Context<Self>) {
        const PAGE_SIZE: usize = 20;

        self.state
            .update(cx, |state, _cx| match state.app.current_screen {
                Screen::Library => {
                    if state.app.library_view_mode == crate::app::LibraryViewMode::TreeView {
                        state.app.page_up_tree(PAGE_SIZE);
                    } else {
                        state.app.page_up_albums(PAGE_SIZE);
                    }
                }
                Screen::Queue => state.app.page_up_queue(PAGE_SIZE),
                Screen::DirectoryManager => state.app.page_up_directories(PAGE_SIZE),
                _ => {}
            });
        cx.notify();
    }

    fn toggle_expand(&mut self, _: &ToggleExpand, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            if state.app.current_screen == Screen::Library {
                state.app.toggle_artist_expansion();
            } else if state.app.current_screen == Screen::Queue {
                state.app.toggle_queue_item_expansion();
            } else if state.app.current_screen == Screen::DirectoryManager {
                state.app.toggle_directory_expansion();
            }
        });
        cx.notify();
    }

    fn update_playback_state(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            let playback_state = state
                .player
                .lock()
                .get_playback_state(state.app.spectrum_visible);

            state.app.position_secs = playback_state.position_secs;
            state.app.loudness_info = playback_state.loudness;

            if state.app.spectrum_visible {
                state.app.spectrum_info = playback_state.spectrum;
            }

            // Check if playback ended and auto-advance
            if state.app.is_playing
                && !playback_state.is_playing
                && state.app.current_queue_index.is_some()
            {
                if let Some(path) = state.app.next_track() {
                    let sample_rate = 48000.0;
                    let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                    let output_channels = state.app.plugin_chain.output_channels();

                    if let Err(e) = state.player.lock().load_and_play(
                        path,
                        plugins,
                        output_channels,
                        state.app.current_output_device_name.clone(),
                    ) {
                        log::error!("Failed to auto-advance: {}", e);
                        state.app.is_playing = false;
                    }
                } else {
                    state.app.is_playing = false;
                }
            }
        });
    }

    fn play_album_at_index(&mut self, index: usize, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            // Add album to queue and start playing
            let albums = state.app.filtered_albums();
            if let Some(album) = albums.get(index).cloned() {
                state
                    .app
                    .queue
                    .push(crate::app::QueueItem::new(album.clone()));
                state.app.expanded_queue_items.push(false);

                if let Some(path) = state.app.start_queue() {
                    let sample_rate = 48000.0;
                    let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
                    let output_channels = state.app.plugin_chain.output_channels();

                    if let Err(e) = state.player.lock().load_and_play(
                        path,
                        plugins,
                        output_channels,
                        state.app.current_output_device_name.clone(),
                    ) {
                        log::error!("Failed to play album: {}", e);
                        state.app.is_playing = false;
                    }
                }
            }
        });
        cx.notify();
    }
    fn remove_item(&mut self, _: &RemoveItem, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            match state.app.current_screen {
                Screen::Queue => {
                    state.app.remove_from_queue(state.app.selected_queue_index);
                }
                Screen::DirectoryManager => {
                    state.app.remove_selected_directory();
                }
                _ => {}
            }
        });
        cx.notify();
    }

    fn clear_queue(&mut self, _: &ClearQueue, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.clear_queue();
        });
        cx.notify();
    }

    fn move_plugin_up(&mut self, _: &MovePluginUp, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_up(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    fn move_plugin_down(&mut self, _: &MovePluginDown, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.move_plugin_down(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    fn toggle_plugin(&mut self, _: &TogglePlugin, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            state.app.toggle_plugin(state.app.selected_plugin_index);
        });
        cx.notify();
    }

    fn add_directory(&mut self, _: &AddDirectory, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            use crate::app::InputMode;
            // Enter add directory mode
            state.app.input_mode = InputMode::AddDirectory;
            state.app.directory_input.clear();
            state.app.clear_autocomplete();
        });
        cx.notify();
    }

    fn scan_library(&mut self, _: &ScanLibrary, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _cx| {
            // Start scan (this will be async in reality, but for now we do it synchronously)
            if let Err(e) = state.app.scan_library() {
                log::error!("Library scan failed: {}", e);
                state.app.status_message = Some(format!("Scan failed: {}", e));
            }
            // Save directories to config after successful scan
            if let Err(e) = state.app.save_config() {
                log::warn!("Failed to save config: {}", e);
            }
        });
        cx.notify();
    }

    fn handle_enter(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            use crate::app::InputMode;

            // Handle input modes first
            if state.app.input_mode == InputMode::AddDirectory {
                // Add the directory
                if !state.app.directory_input.is_empty() {
                    let path = std::path::PathBuf::from(&state.app.directory_input);
                    state.app.add_directory(path);
                    state.app.directory_input.clear();
                    state.app.clear_autocomplete();
                }
                state.app.input_mode = InputMode::Normal;
                return;
            }

            // Handle screen-specific actions in Normal mode
            match state.app.current_screen {
                Screen::Library => {
                    if state.app.library_view_mode == crate::app::LibraryViewMode::TreeView {
                        // Add tree selection to queue
                        if let Some(path) = state.app.add_album_to_queue() {
                            Self::play_track(state, path);
                        }
                    } else {
                        // Add album to queue
                        if let Some(path) = state.app.add_album_to_queue() {
                            Self::play_track(state, path);
                        }
                    }
                }
                Screen::Queue => {
                    // Play selected track in queue
                    // TODO: Implement playing specific track from queue
                }
                _ => {}
            }
        });
        cx.notify();
    }

    fn play_track(state: &mut AppState, path: std::path::PathBuf) {
        let sample_rate = 48000.0;
        let plugins = state.app.plugin_chain.to_plugin_configs(sample_rate);
        let output_channels = state.app.plugin_chain.output_channels();

        if let Err(e) = state.player.lock().load_and_play(
            path,
            plugins,
            output_channels,
            state.app.current_output_device_name.clone(),
        ) {
            log::error!("Failed to play track: {}", e);
            state.app.is_playing = false;
        }
    }
}

impl Render for PlayerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_screen = self.state.read(cx).app.current_screen;
        let input_mode = self.state.read(cx).app.input_mode;

        div()
            .key_context("PlayerView")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::toggle_playback))
            .on_action(cx.listener(Self::stop_playback))
            .on_action(cx.listener(Self::next_track))
            .on_action(cx.listener(Self::prev_track))
            .on_action(cx.listener(Self::volume_up))
            .on_action(cx.listener(Self::volume_down))
            .on_action(cx.listener(Self::switch_to_library))
            .on_action(cx.listener(Self::switch_to_queue))
            .on_action(cx.listener(Self::switch_to_plugins))
            .on_action(cx.listener(Self::switch_to_devices))
            .on_action(cx.listener(Self::switch_to_directory_manager))
            .on_action(cx.listener(Self::toggle_search))
            .on_action(cx.listener(Self::toggle_library_view))
            .on_action(cx.listener(Self::toggle_help))
            .on_action(cx.listener(Self::cycle_sort_order))
            .on_action(cx.listener(Self::set_sort_artist))
            .on_action(cx.listener(Self::set_sort_album))
            .on_action(cx.listener(Self::set_sort_title))
            .on_action(cx.listener(Self::set_sort_year))
            .on_action(cx.listener(Self::cycle_channel_filter))
            .on_action(cx.listener(Self::set_filter_all))
            .on_action(cx.listener(Self::set_filter_mono))
            .on_action(cx.listener(Self::set_filter_stereo))
            .on_action(cx.listener(Self::set_filter_multichannel))
            .on_action(cx.listener(Self::set_filter_mixed))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_prev))
            .on_action(cx.listener(Self::select_next_page))
            .on_action(cx.listener(Self::select_prev_page))
            .on_action(cx.listener(Self::toggle_expand))
            .on_action(cx.listener(Self::handle_enter))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::remove_item))
            .on_action(cx.listener(Self::clear_queue))
            .on_action(cx.listener(Self::move_plugin_up))
            .on_action(cx.listener(Self::move_plugin_down))
            .on_action(cx.listener(Self::toggle_plugin))
            .on_action(cx.listener(Self::add_directory))
            .on_action(cx.listener(Self::scan_library))
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _window, cx| {
                // Handle text input for search mode and add directory mode
                let input_mode = view.state.read(cx).app.input_mode;

                match input_mode {
                    crate::app::InputMode::Search => {
                        view.handle_search_input(event, cx);
                    }
                    crate::app::InputMode::AddDirectory => {
                        view.handle_directory_input(event, cx);
                    }
                    _ => {}
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xcccccc))
            .child(self.render_header(cx))
            .child(div().flex().flex_1().child(match current_screen {
                Screen::Library => self.render_library_screen(cx).into_any_element(),
                Screen::Queue => self.render_queue_screen(cx).into_any_element(),
                Screen::Plugins => self.render_plugins_screen(cx).into_any_element(),
                Screen::Devices => self.render_devices_screen(cx).into_any_element(),
                Screen::Spectrum => self.render_spectrum_screen(cx).into_any_element(),
                Screen::DirectoryManager => self.render_directory_screen(cx).into_any_element(),
            }))
            .child(self.render_footer(cx))
            .when(input_mode == crate::app::InputMode::Help, |div| {
                div.child(self.render_help_modal(cx))
            })
    }
}

impl PlayerView {
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .bg(rgb(0x2d2d2d))
            .border_b_1()
            .border_color(rgb(0x3e3e3e))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .child("SOTF Audio Player"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(self.render_tab_button("Library", Screen::Library, cx))
                    .child(self.render_tab_button("Queue", Screen::Queue, cx))
                    .child(self.render_tab_button("Plugins", Screen::Plugins, cx))
                    .child(self.render_tab_button("Devices", Screen::Devices, cx)),
            )
    }

    fn render_tab_button(
        &self,
        label: &str,
        screen: Screen,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let state = self.state.read(cx);
        let is_active = state.app.current_screen == screen;

        let button = div()
            .px_4()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .child(label.to_string());

        if is_active {
            button.bg(rgb(0x007acc)).text_color(rgb(0xffffff))
        } else {
            button
                .bg(rgb(0x3e3e3e))
                .hover(|style| style.bg(rgb(0x505050)))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                        view.switch_screen(screen, cx);
                    }),
                )
        }
    }

    fn render_library_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (library_view_mode, albums_count, search_query, scan_in_progress, input_mode, sort_order, channel_filter, filtered_count) = {
            let state = self.state.read(cx);
            let filtered_count = state.app.filtered_albums().len();
            (
                state.app.library_view_mode,
                state.app.library.albums.len(),
                state.app.search_query.clone(),
                state.app.scan_in_progress,
                state.app.input_mode,
                state.app.library_sort_order,
                state.app.channel_filter,
                filtered_count,
            )
        };

        let is_search_mode = input_mode == crate::app::InputMode::Search;

        let content = if library_view_mode == crate::app::LibraryViewMode::TreeView {
            self.render_library_tree(cx).into_any_element()
        } else {
            self.render_library_flat(cx).into_any_element()
        };

        let sort_label = match sort_order {
            crate::app::LibrarySortOrder::Artist => "Artist",
            crate::app::LibrarySortOrder::Album => "Album",
            crate::app::LibrarySortOrder::Title => "Title",
            crate::app::LibrarySortOrder::Year => "Year",
        };

        let filter_label = match channel_filter {
            crate::app::ChannelFilter::All => "All".to_string(),
            crate::app::ChannelFilter::Mono => "Mono".to_string(),
            crate::app::ChannelFilter::Stereo => "Stereo".to_string(),
            crate::app::ChannelFilter::Multichannel => "Multi".to_string(),
            crate::app::ChannelFilter::Mixed => "Mixed".to_string(),
            crate::app::ChannelFilter::Specific(n) => format!("{}ch", n),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .mb_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if filtered_count == albums_count {
                                format!("Library ({} albums)", albums_count)
                            } else {
                                format!("Library ({}/{} albums)", filtered_count, albums_count)
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .bg(rgb(0x2d2d2d))
                                    .rounded_md()
                                    .border_1()
                                    .when(is_search_mode, |div| div.border_color(rgb(0x007acc)))
                                    .when(!is_search_mode, |div| div.border_color(rgb(0x3e3e3e)))
                                    .px_2()
                                    .py_1()
                                    .w_64()
                                    .child(
                                        div()
                                            .mr_2()
                                            .text_color(if is_search_mode { rgb(0x007acc) } else { rgb(0x999999) })
                                            .child("🔍")
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(if search_query.is_empty() {
                                                if is_search_mode { rgb(0x999999) } else { rgb(0x666666) }
                                            } else {
                                                rgb(0xcccccc)
                                            })
                                            .child(if search_query.is_empty() {
                                                if is_search_mode {
                                                    "Type to search...".to_string()
                                                } else {
                                                    "Press / to search".to_string()
                                                }
                                            } else {
                                                format!("{}{}",search_query, if is_search_mode { "|" } else { "" })
                                            })
                                    )
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child(format!("Sort: {}", sort_label))
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x999999))
                                    .child(format!("Filter: {}", filter_label))
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(if library_view_mode == crate::app::LibraryViewMode::Flat { rgb(0x4e4e4e) } else { rgb(0x2d2d2d) })
                                            .cursor_pointer()
                                            .on_mouse_up(MouseButton::Left, cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, cx| state.app.library_view_mode = crate::app::LibraryViewMode::Flat);
                                                cx.notify();
                                            }))
                                            .child("Flat"),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_1()
                                            .rounded_md()
                                            .bg(if library_view_mode == crate::app::LibraryViewMode::TreeView { rgb(0x4e4e4e) } else { rgb(0x2d2d2d) })
                                            .cursor_pointer()
                                            .on_mouse_up(MouseButton::Left, cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                                view.state.update(cx, |state, cx| {
                                                    state.app.library_view_mode = crate::app::LibraryViewMode::TreeView;
                                                    state.app.rebuild_artist_tree();
                                                });
                                                cx.notify();
                                            }))
                                            .child("Tree"),
                                    ),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_1()
                                    .ml_2()
                                    .rounded_md()
                                    .bg(rgb(0x2d2d2d))
                                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                                    .cursor_pointer()
                                    .id("scan_btn")
                                    .on_mouse_up(MouseButton::Left, cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, cx| {
                                            if let Err(e) = state.app.scan_library() {
                                                log::error!("Scan failed: {}", e);
                                            }
                                        });
                                        cx.notify();
                                    }))
                                    .child(if scan_in_progress { "Scanning..." } else { "Scan" }),
                            ),
                    ),
            )
            .child(content)
    }

    fn render_library_flat(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let albums = state.app.filtered_albums();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .children(albums.iter().enumerate().map(|(idx, album)| {
                let is_selected = state.app.selected_album_index == idx;
                div()
                    .p_3()
                    .rounded_md()
                    .when(is_selected, |div| div.bg(rgb(0x007acc)))
                    .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state
                                .update(cx, |state, cx| state.app.selected_album_index = idx);
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(album.title.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x999999))
                                    .child(album.artist.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x666666))
                                    .child(format!("{} tracks", album.tracks.len())),
                            ),
                    )
            }))
    }

    fn render_library_tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let tree_items = state.app.get_tree_items();

        div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .children(tree_items.iter().enumerate().map(|(idx, item)| {
                let is_selected = state.app.selected_tree_index == idx;

                match item {
                    crate::app::TreeItem::Artist { name, expanded } => div()
                        .p_2()
                        .rounded_md()
                        .when(is_selected, |div| div.bg(rgb(0x007acc)))
                        .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                        .cursor_pointer()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, cx| {
                                    state.app.selected_tree_index = idx;
                                    state.app.toggle_artist_expansion();
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(if *expanded { "▼" } else { "▶" })
                                .child(name.clone()),
                        ),
                    crate::app::TreeItem::Album { index } => {
                        let album = &state.app.library.albums[*index];
                        div()
                            .pl_8()
                            .p_2()
                            .rounded_md()
                            .when(is_selected, |div| div.bg(rgb(0x007acc)))
                            .when(!is_selected, |div| div.bg(rgb(0x252525))) // Slightly darker for albums
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, cx| {
                                        state.app.selected_tree_index = idx
                                    });
                                    cx.notify();
                                }),
                            )
                            .child(album.title.clone())
                    }
                }
            }))
    }

    fn render_queue_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(format!("Queue ({} albums)", state.app.queue.len()))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x2d2d2d))
                            .hover(|style| style.bg(rgb(0x8e2e2e)))
                            .cursor_pointer()
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                                    view.state.update(cx, |state, cx| {
                                        state.app.clear_queue();
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("Clear"),
                    ),
            )
            .child(div().flex().flex_col().gap_2().flex_1().children(
                state.app.queue.iter().enumerate().map(|(idx, item)| {
                    let is_current = state.app.current_queue_index == Some(idx);
                    div()
                        .p_3()
                        .rounded_md()
                        .when(is_current, |div| div.bg(rgb(0x007acc)))
                        .when(!is_current, |div| div.bg(rgb(0x2d2d2d)))
                        .hover(|style| style.bg(rgb(0x3e3e3e)))
                        .cursor_pointer()
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                view.state.update(cx, |state, cx| {
                                    state.app.current_queue_index = Some(idx);
                                    if let Some(path) =
                                        state.app.queue[idx].current_track().map(|t| t.path.clone())
                                    {
                                        Self::play_track(state, path);
                                    }
                                });
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(item.album.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(0x999999))
                                        .child(item.album.artist.clone()),
                                )
                                .child(div().text_xs().text_color(rgb(0x666666)).child(format!(
                                    "Track {}/{}",
                                    item.current_track_index + 1,
                                    item.album.tracks.len()
                                ))),
                        )
                }),
            ))
    }

    fn render_plugins_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .child(
                // Left panel: Plugin List
                div()
                    .w_1_3()
                    .border_r_1()
                    .border_color(rgb(0x3e3e3e))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(rgb(0x3e3e3e))
                            .font_weight(FontWeight::BOLD)
                            .child("Plugin Chain"),
                    )
                    .child(self.render_plugin_list(cx))
                    .child(self.render_plugin_actions(cx)),
            )
            .child(
                // Right panel: Plugin Settings
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .p_4()
                            .border_b_1()
                            .border_color(rgb(0x3e3e3e))
                            .font_weight(FontWeight::BOLD)
                            .child("Settings"),
                    )
                    .child(self.render_plugin_settings(cx)),
            )
    }

    fn render_plugin_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let plugins = state.app.plugin_chain.plugins();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .children(plugins.iter().enumerate().map(|(idx, plugin)| {
                let is_selected = state.app.selected_plugin_index == idx;
                let name = plugin.plugin_type().name().to_string();
                let enabled = plugin.enabled;

                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .when(is_selected, |div| div.bg(rgb(0x007acc)))
                    .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                    .hover(|style| style.bg(rgb(0x3e3e3e)))
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                            view.state
                                .update(cx, |state, cx| state.app.selected_plugin_index = idx);
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(rgb(0x999999))
                                    .bg(if enabled {
                                        rgb(0x00ff00)
                                    } else {
                                        rgb(0x000000)
                                    })
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                            view.state.update(cx, |state, cx| {
                                                state.app.plugin_chain.toggle_plugin(idx);
                                                state.app.needs_plugin_update = true;
                                            });
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(name),
                    )
            }))
    }

    fn render_plugin_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .border_t_1()
            .border_color(rgb(0x3e3e3e))
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(rgb(0x4e4e4e))
                    .rounded_md()
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, cx| {
                                // Add Upmixer by default for now, or show menu
                                // For simplicity, let's cycle or add a specific one
                                state
                                    .app
                                    .plugin_chain
                                    .add_plugin(&sotf_audio_player::PluginType::Upmixer);
                                state.app.needs_plugin_update = true;
                            });
                            cx.notify();
                        }),
                    )
                    .child("+ Upmixer"),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(rgb(0x4e4e4e))
                    .rounded_md()
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, cx| {
                                state
                                    .app
                                    .plugin_chain
                                    .add_plugin(&sotf_audio_player::PluginType::EQ);
                                state.app.needs_plugin_update = true;
                            });
                            cx.notify();
                        }),
                    )
                    .child("+ EQ"),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .bg(rgb(0x8e2e2e))
                    .rounded_md()
                    .cursor_pointer()
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _: &MouseUpEvent, _window, cx| {
                            view.state.update(cx, |state, cx| {
                                let idx = state.app.selected_plugin_index;
                                state.app.plugin_chain.remove_plugin(idx);
                                if state.app.selected_plugin_index >= state.app.plugin_chain.len()
                                    && state.app.plugin_chain.len() > 0
                                {
                                    state.app.selected_plugin_index =
                                        state.app.plugin_chain.len() - 1;
                                }
                                state.app.needs_plugin_update = true;
                            });
                            cx.notify();
                        }),
                    )
                    .child("Remove"),
            )
    }

    fn render_plugin_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        if let Some(plugin) = state
            .app
            .plugin_chain
            .get_plugin(state.app.selected_plugin_index)
        {
            match &plugin.settings {
                sotf_audio_player::PluginSettings::Upmixer {
                    speaker_config,
                    lfe_gain,
                    gain_front_direct: _,
                    gain_front_ambient: _,
                    gain_rear_ambient: _,
                    lfe_cutoff_hz: _,
                    stereo_width: _,
                    bandpass_hz: _,
                    height_gain: _,
                    enable_subharmonic_synth: _,
                    subharmonic_gain: _,
                    enable_hr_direct: _,
                    hr_sharpen: _,
                    safety_cap_db: _,
                } => div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(format!("Speaker Config: {}", speaker_config))
                    .child(format!("LFE Gain: {:.2}", lfe_gain))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x999999))
                            .child("Editing not yet implemented"),
                    ),
                sotf_audio_player::PluginSettings::EQ { filters } => div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(format!("EQ ({} bands)", filters.len()))
                    .children(filters.iter().enumerate().map(|(i, f)| {
                        div().child(format!(
                            "Band {}: {:.0} Hz, {:.1} dB",
                            i + 1,
                            f.frequency,
                            f.gain_db
                        ))
                    })),
                _ => div()
                    .p_4()
                    .child("Settings not available for this plugin type"),
            }
        } else {
            div().p_4().child("No plugin selected")
        }
    }

    fn render_devices_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Audio Devices"),
            )
            .child(
                div().flex().flex_col().gap_2().children(
                    state
                        .app
                        .output_devices
                        .iter()
                        .enumerate()
                        .map(|(idx, device)| {
                            let is_selected = state.app.selected_output_device_index == idx;
                            div()
                                .p_3()
                                .rounded_md()
                                .when(is_selected, |div| div.bg(rgb(0x007acc)))
                                .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                                .hover(|style| style.bg(rgb(0x3e3e3e)))
                                .cursor_pointer()
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |view, _: &MouseUpEvent, _window, cx| {
                                        view.state.update(cx, |state, cx| {
                                            state.app.selected_output_device_index = idx;
                                            if let Some(device) = state.app.output_devices.get(idx)
                                            {
                                                state.app.current_output_device_name =
                                                    Some(device.name.clone());

                                                // If playing, restart track with new device
                                                if state.app.is_playing {
                                                    if let Some(queue_idx) =
                                                        state.app.current_queue_index
                                                    {
                                                        if let Some(item) =
                                                            state.app.queue.get(queue_idx)
                                                        {
                                                            if let Some(track) =
                                                                item.current_track()
                                                            {
                                                                let path = track.path.clone();
                                                                Self::play_track(state, path);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                        cx.notify();
                                    }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(device.name.clone()),
                                        )
                                        .child(div().text_sm().text_color(rgb(0x999999)).child(
                                            format!(
                                                    "{} channels",
                                                    device
                                                        .default_config
                                                        .as_ref()
                                                        .map(|c| c.channels)
                                                        .unwrap_or(0)
                                                ),
                                        )),
                                )
                        }),
                ),
            )
    }

    fn render_spectrum_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        let content = if let Some(info) = &state.app.spectrum_info {
            div()
                .flex()
                .flex_col()
                .size_full()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_1()
                        .h_64()
                        .w_full()
                        .bg(rgb(0x000000))
                        .p_2()
                        .children(info.magnitudes.iter().enumerate().map(|(i, &mag)| {
                            let normalized = ((mag + 100.0) / 100.0).clamp(0.0, 1.0);
                            let color = if normalized > 0.9 {
                                rgb(0xff0000)
                            } else if normalized > 0.7 {
                                rgb(0xffff00)
                            } else {
                                rgb(0x00ff00)
                            };

                            div()
                                .w_full()
                                .h(gpui::Length::Definite(gpui::DefiniteLength::Fraction(
                                    normalized,
                                )))
                                .bg(color)
                                .rounded_t_sm()
                        })),
                )
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(rgb(0x999999))
                        .child("20 Hz")
                        .child("1 kHz")
                        .child("20 kHz"),
                )
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(rgb(0x666666))
                .child("No spectrum data available. Play audio to see visualization.")
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Spectrum Analyzer"),
            )
            .child(content)
    }

    fn render_directory_screen(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let is_add_mode = state.app.input_mode == crate::app::InputMode::AddDirectory;
        let tree_items = state.app.get_directory_tree_items();

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mb_4()
                    .child("Directory Manager"),
            )
            .when(is_add_mode, |div| {
                div.child(
                    div()
                        .p_3()
                        .mb_4()
                        .rounded_md()
                        .bg(rgb(0x2d2d2d))
                        .border_1()
                        .border_color(rgb(0x007acc))
                        .child(div().text_sm().child("Add Directory"))
                        .child(
                            div()
                                .text_sm()
                                .mt_2()
                                .child(format!(
                                    "Path: {}{}",
                                    state.app.directory_input,
                                    if is_add_mode { "█" } else { "" }
                                ))
                        )
                        .child(
                            div()
                                .text_xs()
                                .mt_2()
                                .text_color(rgb(0x999999))
                                .child("Tab: autocomplete, Enter: add, Esc: cancel")
                        )
                )
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .flex_1()
                    .overflow_y_scroll()
                    .children(tree_items.iter().enumerate().map(|(i, (path, level, expanded))| {
                        let is_selected = i == state.app.selected_directory_index;
                        let indent = "  ".repeat(*level);
                        let prefix = if *level == 0 {
                            if *expanded { "▼ " } else { "▶ " }
                        } else {
                            "  "
                        };

                        div()
                            .p_2()
                            .rounded_md()
                            .when(is_selected, |div| div.bg(rgb(0x264f78)))
                            .when(!is_selected, |div| div.bg(rgb(0x2d2d2d)))
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{}{}{}", indent, prefix, path.display()))
                            )
                    }))
            )
            .when(state.app.scan_in_progress, |div| {
                div.child(
                    div()
                        .p_3()
                        .mt_4()
                        .rounded_md()
                        .bg(rgb(0x2d2d2d))
                        .border_1()
                        .border_color(rgb(0x4ec9b0))
                        .child(div().text_sm().child("Scanning library..."))
                        .child(
                            div()
                                .text_xs()
                                .mt_2()
                                .child(format!(
                                    "{} tracks, {} albums found",
                                    state.app.scan_progress_tracks,
                                    state.app.scan_progress_albums
                                ))
                        )
                )
            })
            .when(state.app.status_message.is_some(), |div| {
                div.child(
                    div()
                        .p_3()
                        .mt_4()
                        .rounded_md()
                        .bg(rgb(0x2d2d2d))
                        .border_1()
                        .border_color(rgb(0x4ec9b0))
                        .child(
                            div()
                                .text_sm()
                                .child(state.app.status_message.as_ref().unwrap().clone())
                        )
                )
            })
            .child(
                div()
                    .p_3()
                    .mt_4()
                    .rounded_md()
                    .bg(rgb(0x1e1e1e))
                    .text_xs()
                    .text_color(rgb(0x999999))
                    .child("Shift-A: Add Directory | Shift-S: Scan Library | D: Remove | Enter/L: Expand | Esc: Cancel")
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        div()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .bg(rgb(0x2d2d2d))
            .border_t_1()
            .border_color(rgb(0x3e3e3e))
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(div().text_sm().child(if state.app.is_playing {
                        "Playing"
                    } else {
                        "Stopped"
                    }))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Volume: {:.0}%", state.app.volume * 100.0)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("Space: Play/Pause"),
                    )
                    .child(div().text_xs().text_color(rgb(0x999999)).child("N: Next"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x999999))
                            .child("+/-: Volume"),
                    ),
            )
    }

    fn render_help_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let screen_name = match state.app.current_screen {
            Screen::Library => "Library",
            Screen::DirectoryManager => "Directories",
            Screen::Queue => "Queue",
            Screen::Plugins => "Plugins",
            Screen::Devices => "Devices",
            Screen::Spectrum => "Spectrum",
        };

        // Get keybindings for current screen
        let keybindings = get_keybindings_for_screen(state.app.current_screen);

        // Create modal overlay (centered, 80% width, 90% height)
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x000000aa)) // Semi-transparent background
            .child(
                div()
                    .w(Rems(60.0)) // 80% approx
                    .h(Rems(40.0)) // 90% approx
                    .bg(rgb(0x1e1e1e))
                    .border_2()
                    .border_color(rgb(0x007acc))
                    .rounded_md()
                    .p_4()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .mb_4()
                            .child(format!(
                                "Help - {} Screen (Press ESC or ? to close)",
                                screen_name
                            ))
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            // Global keybindings section
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0x4ec9b0))
                                    .mb_2()
                                    .child("GLOBAL KEYBINDINGS")
                            )
                            .child(self.render_keybinding("Shift-L/Q/P/O/D", "Jump to Library/Queue/Plugins/Devices/Directories"))
                            .child(self.render_keybinding("+/=", "Increase volume"))
                            .child(self.render_keybinding("-/_", "Decrease volume"))
                            .child(self.render_keybinding("?", "Show this help"))
                            .child(div().h_4()) // Spacer
                            // Screen-specific keybindings section
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0x4ec9b0))
                                    .mb_2()
                                    .child(format!("{} KEYBINDINGS", screen_name.to_uppercase()))
                            )
                            .children(
                                keybindings.iter().map(|(key, desc)| {
                                    self.render_keybinding(key, desc)
                                })
                            )
                    )
            )
    }

    fn render_keybinding(&self, key: &str, description: &str) -> impl IntoElement {
        div()
            .flex()
            .gap_4()
            .mb_1()
            .child(
                div()
                    .w(Rems(12.0))
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0x569cd6))
                    .child(format!("  {}", key))
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0xcccccc))
                    .child(description)
            )
    }
}

fn get_keybindings_for_screen(screen: Screen) -> Vec<(&'static str, &'static str)> {
    match screen {
        Screen::Library => vec![
            ("↑/↓ or K/J", "Navigate albums/artists"),
            ("PageUp/PageDown", "Jump by page"),
            ("/", "Search albums"),
            ("T", "Toggle tree view / flat view"),
            ("H/L or ←/→", "Collapse/expand artists in tree view"),
            ("S or 1/2/3/4", "Sort by Artist/Album/Title/Year"),
            ("C or 5/6/7/8/9", "Filter: All/Mono/Stereo/Multi/Mixed"),
            ("A or Enter", "Add album to queue"),
            ("Shift-Q", "Go to queue screen"),
        ],
        Screen::DirectoryManager => vec![
            ("↑/↓ or K/J", "Navigate directories"),
            ("PageUp/PageDown", "Jump by page"),
            ("Enter/→/L", "Expand/collapse directory"),
            ("Shift-A", "Add directory"),
            ("D/Delete", "Remove selected directory"),
            ("Shift-S", "Scan library"),
        ],
        Screen::Queue => vec![
            ("↑/↓ or K/J", "Navigate queue items"),
            ("Enter", "Play selected album from start"),
            ("H/L or ←/→", "Expand/collapse album tracks"),
            ("Space", "Play/Pause"),
            ("N or >", "Next track"),
            ("B or <", "Previous track"),
            ("D/Delete", "Remove from queue"),
            ("C", "Clear entire queue"),
        ],
        Screen::Plugins => vec![
            ("↑/↓ or K/J", "Navigate plugin chain"),
            ("A", "Add plugin (default)"),
            ("1/2/3/4/5/6/7", "Quick add: EQ/Upmixer/Compressor/Gate/Limiter/Loudness/Binaural"),
            ("E or Enter", "Edit selected plugin"),
            ("Shift-T", "Toggle plugin enabled/disabled"),
            ("D/Delete", "Remove plugin"),
            ("U", "Move plugin up in chain"),
            ("Shift-N", "Move plugin down in chain"),
        ],
        Screen::Devices => vec![
            ("↑/↓ or K/J", "Navigate output devices"),
            ("Enter/Space", "Select output device"),
        ],
        Screen::Spectrum => vec![
            ("Space", "Play/Pause"),
            ("N", "Next track"),
        ],
    }
}
