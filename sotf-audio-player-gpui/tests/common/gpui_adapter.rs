//! GPUI App adapter for equivalence testing.
//!
//! Adapts the GPUI App to the common AppAdapter interface.

use super::{
    AppAdapter, ChannelFilterId, ComparableState, InputModeId, Operation, PluginSnapshot,
    PluginTypeId, ScreenId, SortOrderId, TestAlbum, ViewModeId,
};
use sotf_audio_player_gpui::app::{
    App as GpuiApp, ChannelFilter, InputMode, LibrarySortOrder, LibraryViewMode, Screen,
};

/// Adapter wrapping the GPUI App for testing
pub struct GpuiAdapter {
    pub app: GpuiApp,
}

impl GpuiAdapter {
    pub fn new() -> Self {
        Self {
            app: GpuiApp::new(),
        }
    }
}

impl Default for GpuiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// Conversion helpers for GPUI types to normalized IDs

fn screen_to_id(screen: Screen) -> ScreenId {
    match screen {
        Screen::Library => ScreenId::Library,
        Screen::DirectoryManager => ScreenId::DirectoryManager,
        Screen::Queue => ScreenId::Queue,
        Screen::Plugins => ScreenId::Plugins,
        Screen::Devices => ScreenId::Devices,
        Screen::Spectrum => ScreenId::Spectrum,
    }
}

fn id_to_screen(id: ScreenId) -> Screen {
    match id {
        ScreenId::Library => Screen::Library,
        ScreenId::DirectoryManager => Screen::DirectoryManager,
        ScreenId::Queue => Screen::Queue,
        ScreenId::Plugins => Screen::Plugins,
        ScreenId::Devices => Screen::Devices,
        ScreenId::Spectrum => Screen::Spectrum,
    }
}

fn input_mode_to_id(mode: InputMode) -> InputModeId {
    match mode {
        InputMode::Normal => InputModeId::Normal,
        InputMode::Search => InputModeId::Search,
        InputMode::AddDirectory => InputModeId::AddDirectory,
        InputMode::EditPlugin => InputModeId::EditPlugin,
        InputMode::SavePlugins => InputModeId::SavePlugins,
        InputMode::LoadPlugins => InputModeId::LoadPlugins,
        InputMode::LoadApoFile => InputModeId::LoadApoFile,
        InputMode::LoadSofaFile => InputModeId::LoadSofaFile,
        InputMode::Help => InputModeId::Help,
    }
}

fn id_to_input_mode(id: InputModeId) -> InputMode {
    match id {
        InputModeId::Normal => InputMode::Normal,
        InputModeId::Search => InputMode::Search,
        InputModeId::AddDirectory => InputMode::AddDirectory,
        InputModeId::EditPlugin => InputMode::EditPlugin,
        InputModeId::SavePlugins => InputMode::SavePlugins,
        InputModeId::LoadPlugins => InputMode::LoadPlugins,
        InputModeId::LoadApoFile => InputMode::LoadApoFile,
        InputModeId::LoadSofaFile => InputMode::LoadSofaFile,
        InputModeId::Help => InputMode::Help,
    }
}

fn sort_order_to_id(order: LibrarySortOrder) -> SortOrderId {
    match order {
        LibrarySortOrder::Artist => SortOrderId::Artist,
        LibrarySortOrder::Album => SortOrderId::Album,
        LibrarySortOrder::Title => SortOrderId::Title,
        LibrarySortOrder::Year => SortOrderId::Year,
    }
}

fn id_to_sort_order(id: SortOrderId) -> LibrarySortOrder {
    match id {
        SortOrderId::Artist => LibrarySortOrder::Artist,
        SortOrderId::Album => LibrarySortOrder::Album,
        SortOrderId::Title => LibrarySortOrder::Title,
        SortOrderId::Year => LibrarySortOrder::Year,
        SortOrderId::Popularity => LibrarySortOrder::Artist, // GPUI doesn't have Popularity yet
    }
}

fn channel_filter_to_id(filter: ChannelFilter) -> ChannelFilterId {
    match filter {
        ChannelFilter::All => ChannelFilterId::All,
        ChannelFilter::Mono => ChannelFilterId::Mono,
        ChannelFilter::Stereo => ChannelFilterId::Stereo,
        ChannelFilter::Multichannel => ChannelFilterId::Multichannel,
        ChannelFilter::Mixed => ChannelFilterId::Mixed,
        ChannelFilter::Specific(n) => ChannelFilterId::Specific(n),
    }
}

fn id_to_channel_filter(id: ChannelFilterId) -> ChannelFilter {
    match id {
        ChannelFilterId::All => ChannelFilter::All,
        ChannelFilterId::Mono => ChannelFilter::Mono,
        ChannelFilterId::Stereo => ChannelFilter::Stereo,
        ChannelFilterId::Multichannel => ChannelFilter::Multichannel,
        ChannelFilterId::Mixed => ChannelFilter::Mixed,
        ChannelFilterId::Specific(n) => ChannelFilter::Specific(n),
    }
}

fn view_mode_to_id(mode: LibraryViewMode) -> ViewModeId {
    match mode {
        LibraryViewMode::Flat => ViewModeId::Flat,
        LibraryViewMode::TreeView => ViewModeId::TreeView,
    }
}

fn plugin_type_to_id(plugin_type: &sotf_audio_player::PluginType) -> PluginTypeId {
    use sotf_audio_player::PluginType;
    match plugin_type {
        PluginType::Gain => PluginTypeId::Gain,
        PluginType::EQ => PluginTypeId::EQ,
        PluginType::Upmixer => PluginTypeId::Upmixer,
        PluginType::BinauralDecoder => PluginTypeId::BinauralDecoder,
        PluginType::LoudnessMonitor => PluginTypeId::LoudnessMonitor,
        PluginType::SpectrumAnalyzer => PluginTypeId::SpectrumAnalyzer,
        PluginType::Compressor => PluginTypeId::Compressor,
        PluginType::Gate => PluginTypeId::Gate,
        PluginType::Limiter => PluginTypeId::Limiter,
        PluginType::LoudnessCompensation => PluginTypeId::LoudnessCompensation,
        PluginType::Convolution => PluginTypeId::Convolution,
        PluginType::ChannelMuteSolo => PluginTypeId::ChannelMuteSolo,
    }
}

fn id_to_plugin_type(id: &PluginTypeId) -> sotf_audio_player::PluginType {
    use sotf_audio_player::PluginType;
    match id {
        PluginTypeId::Gain => PluginType::Gain,
        PluginTypeId::EQ => PluginType::EQ,
        PluginTypeId::Upmixer => PluginType::Upmixer,
        PluginTypeId::BinauralDecoder => PluginType::BinauralDecoder,
        PluginTypeId::LoudnessMonitor => PluginType::LoudnessMonitor,
        PluginTypeId::SpectrumAnalyzer => PluginType::SpectrumAnalyzer,
        PluginTypeId::Compressor => PluginType::Compressor,
        PluginTypeId::Gate => PluginType::Gate,
        PluginTypeId::Limiter => PluginType::Limiter,
        PluginTypeId::LoudnessCompensation => PluginType::LoudnessCompensation,
        PluginTypeId::Convolution => PluginType::Convolution,
        PluginTypeId::ChannelMuteSolo => PluginType::ChannelMuteSolo,
        PluginTypeId::Unknown(_) => PluginType::Gain, // Fallback
    }
}

impl AppAdapter for GpuiAdapter {
    fn get_state(&self) -> ComparableState {
        let plugins: Vec<PluginSnapshot> = self
            .app
            .plugin_chain
            .plugins()
            .iter()
            .map(|p| PluginSnapshot {
                plugin_type: plugin_type_to_id(&p.plugin_type()),
                enabled: p.enabled,
                param_count: 0, // Plugin settings don't expose parameter count directly
            })
            .collect();

        ComparableState {
            current_screen: screen_to_id(self.app.current_screen),
            input_mode: input_mode_to_id(self.app.input_mode),
            selected_album_index: self.app.selected_album_index,
            library_album_count: self.app.library.albums.len(),
            search_query: self.app.search_query.clone(),
            library_sort_order: sort_order_to_id(self.app.library_sort_order),
            channel_filter: channel_filter_to_id(self.app.channel_filter),
            library_view_mode: view_mode_to_id(self.app.library_view_mode),
            queue_length: self.app.queue.len(),
            current_queue_index: self.app.current_queue_index,
            selected_queue_index: self.app.selected_queue_index,
            is_playing: self.app.is_playing,
            volume: self.app.volume,
            plugin_chain_length: self.app.plugin_chain.len(),
            selected_plugin_index: self.app.selected_plugin_index,
            editing_plugin_index: self.app.editing_plugin_index,
            plugins,
            directory_count: self.app.library.directories.len(),
            selected_directory_index: self.app.selected_directory_index,
            device_count: self.app.output_devices.len(),
            selected_device_index: self.app.selected_output_device_index,
        }
    }

    fn apply_operation(&mut self, op: Operation) {
        match op {
            // Navigation
            Operation::SwitchScreen(screen) => {
                self.app.current_screen = id_to_screen(screen);
            }
            Operation::SetInputMode(mode) => {
                self.app.input_mode = id_to_input_mode(mode);
            }
            Operation::ExitInputMode => {
                self.app.input_mode = InputMode::Normal;
            }

            // Library navigation
            Operation::SelectNextAlbum => {
                if !self.app.library.albums.is_empty() {
                    self.app.selected_album_index = (self.app.selected_album_index + 1)
                        .min(self.app.library.albums.len().saturating_sub(1));
                }
            }
            Operation::SelectPreviousAlbum => {
                self.app.selected_album_index = self.app.selected_album_index.saturating_sub(1);
            }
            Operation::SelectAlbumAtIndex(idx) => {
                if idx < self.app.library.albums.len() {
                    self.app.selected_album_index = idx;
                }
            }
            Operation::PageDown => {
                let page_size = self.app.library_items_per_page;
                self.app.selected_album_index = (self.app.selected_album_index + page_size)
                    .min(self.app.library.albums.len().saturating_sub(1));
            }
            Operation::PageUp => {
                let page_size = self.app.library_items_per_page;
                self.app.selected_album_index =
                    self.app.selected_album_index.saturating_sub(page_size);
            }

            // Library configuration
            Operation::SetSearchQuery(query) => {
                self.app.search_query = query;
            }
            Operation::ClearSearch => {
                self.app.search_query.clear();
            }
            Operation::CycleSortOrder => {
                self.app.library_sort_order = match self.app.library_sort_order {
                    LibrarySortOrder::Artist => LibrarySortOrder::Album,
                    LibrarySortOrder::Album => LibrarySortOrder::Title,
                    LibrarySortOrder::Title => LibrarySortOrder::Year,
                    LibrarySortOrder::Year => LibrarySortOrder::Artist,
                };
            }
            Operation::SetSortOrder(order) => {
                self.app.library_sort_order = id_to_sort_order(order);
            }
            Operation::CycleChannelFilter => {
                self.app.channel_filter = match self.app.channel_filter {
                    ChannelFilter::All => ChannelFilter::Stereo,
                    ChannelFilter::Stereo => ChannelFilter::Multichannel,
                    ChannelFilter::Multichannel => ChannelFilter::Mono,
                    ChannelFilter::Mono => ChannelFilter::Mixed,
                    ChannelFilter::Mixed => ChannelFilter::All,
                    ChannelFilter::Specific(_) => ChannelFilter::All,
                };
            }
            Operation::SetChannelFilter(filter) => {
                self.app.channel_filter = id_to_channel_filter(filter);
            }
            Operation::ToggleViewMode => {
                self.app.library_view_mode = match self.app.library_view_mode {
                    LibraryViewMode::Flat => LibraryViewMode::TreeView,
                    LibraryViewMode::TreeView => LibraryViewMode::Flat,
                };
            }
            Operation::SetViewMode(mode) => {
                self.app.library_view_mode = match mode {
                    ViewModeId::Flat => LibraryViewMode::Flat,
                    ViewModeId::TreeView => LibraryViewMode::TreeView,
                };
            }

            // Queue management
            Operation::AddSelectedAlbumToQueue => {
                if let Some(album) = self.app.library.albums.get(self.app.selected_album_index) {
                    use sotf_audio_player_gpui::app::QueueItem;
                    self.app.queue.push(QueueItem::new(album.clone()));
                    self.app.expanded_queue_items.push(false);
                }
            }
            Operation::AddAlbumToQueueAtIndex(idx) => {
                if let Some(album) = self.app.library.albums.get(idx) {
                    use sotf_audio_player_gpui::app::QueueItem;
                    self.app.queue.push(QueueItem::new(album.clone()));
                    self.app.expanded_queue_items.push(false);
                }
            }
            Operation::RemoveFromQueue(idx) => {
                if idx < self.app.queue.len() {
                    self.app.queue.remove(idx);
                    if idx < self.app.expanded_queue_items.len() {
                        self.app.expanded_queue_items.remove(idx);
                    }
                    // Adjust selected index if needed
                    if self.app.selected_queue_index >= self.app.queue.len()
                        && !self.app.queue.is_empty()
                    {
                        self.app.selected_queue_index = self.app.queue.len() - 1;
                    }
                }
            }
            Operation::ClearQueue => {
                self.app.queue.clear();
                self.app.expanded_queue_items.clear();
                self.app.selected_queue_index = 0;
                self.app.current_queue_index = None;
            }
            Operation::SelectNextQueueItem => {
                if !self.app.queue.is_empty() {
                    self.app.selected_queue_index = (self.app.selected_queue_index + 1)
                        .min(self.app.queue.len().saturating_sub(1));
                }
            }
            Operation::SelectPreviousQueueItem => {
                self.app.selected_queue_index = self.app.selected_queue_index.saturating_sub(1);
            }
            Operation::MoveQueueItemUp => {
                if self.app.selected_queue_index > 0 {
                    let idx = self.app.selected_queue_index;
                    self.app.queue.swap(idx, idx - 1);
                    if idx < self.app.expanded_queue_items.len()
                        && idx - 1 < self.app.expanded_queue_items.len()
                    {
                        self.app.expanded_queue_items.swap(idx, idx - 1);
                    }
                    self.app.selected_queue_index -= 1;
                }
            }
            Operation::MoveQueueItemDown => {
                if self.app.selected_queue_index + 1 < self.app.queue.len() {
                    let idx = self.app.selected_queue_index;
                    self.app.queue.swap(idx, idx + 1);
                    if idx < self.app.expanded_queue_items.len()
                        && idx + 1 < self.app.expanded_queue_items.len()
                    {
                        self.app.expanded_queue_items.swap(idx, idx + 1);
                    }
                    self.app.selected_queue_index += 1;
                }
            }

            // Playback (state-only)
            Operation::Play => {
                self.app.is_playing = true;
            }
            Operation::Pause => {
                self.app.is_playing = false;
            }
            Operation::TogglePlayback => {
                self.app.is_playing = !self.app.is_playing;
            }
            Operation::Stop => {
                self.app.is_playing = false;
                self.app.position_secs = 0.0;
            }
            Operation::NextTrack | Operation::PreviousTrack => {
                // Would need track navigation logic
            }
            Operation::SetVolume(vol) => {
                self.app.volume = vol.clamp(0.0, 1.0);
            }
            Operation::VolumeUp => {
                self.app.volume = (self.app.volume + 0.05).min(1.0);
            }
            Operation::VolumeDown => {
                self.app.volume = (self.app.volume - 0.05).max(0.0);
            }

            // Plugin management
            Operation::AddPlugin(plugin_type) => {
                let pt = id_to_plugin_type(&plugin_type);
                self.app.plugin_chain.add_plugin(&pt);
                self.app.needs_plugin_update = true;
            }
            Operation::RemovePlugin(idx) => {
                if idx < self.app.plugin_chain.len() {
                    self.app.plugin_chain.remove_plugin(idx);
                    self.app.needs_plugin_update = true;
                    // Adjust selection
                    if self.app.selected_plugin_index >= self.app.plugin_chain.len()
                        && self.app.plugin_chain.len() > 0
                    {
                        self.app.selected_plugin_index = self.app.plugin_chain.len() - 1;
                    }
                }
            }
            Operation::TogglePlugin(idx) => {
                if idx < self.app.plugin_chain.len() {
                    self.app.plugin_chain.toggle_plugin(idx);
                    self.app.needs_plugin_update = true;
                }
            }
            Operation::SelectNextPlugin => {
                if self.app.plugin_chain.len() > 0 {
                    self.app.selected_plugin_index = (self.app.selected_plugin_index + 1)
                        .min(self.app.plugin_chain.len().saturating_sub(1));
                }
            }
            Operation::SelectPreviousPlugin => {
                self.app.selected_plugin_index = self.app.selected_plugin_index.saturating_sub(1);
            }
            Operation::EnterPluginEdit => {
                if self.app.selected_plugin_index < self.app.plugin_chain.len() {
                    self.app.editing_plugin_index = Some(self.app.selected_plugin_index);
                    self.app.input_mode = InputMode::EditPlugin;
                }
            }
            Operation::ExitPluginEdit => {
                self.app.editing_plugin_index = None;
                self.app.input_mode = InputMode::Normal;
            }
            Operation::MovePluginUp => {
                if self.app.selected_plugin_index > 0 {
                    let from = self.app.selected_plugin_index;
                    let to = from - 1;
                    self.app.plugin_chain.move_plugin(from, to);
                    self.app.selected_plugin_index = to;
                    self.app.needs_plugin_update = true;
                }
            }
            Operation::MovePluginDown => {
                if self.app.selected_plugin_index + 1 < self.app.plugin_chain.len() {
                    let from = self.app.selected_plugin_index;
                    let to = from + 1;
                    self.app.plugin_chain.move_plugin(from, to);
                    self.app.selected_plugin_index = to;
                    self.app.needs_plugin_update = true;
                }
            }

            // Directory management
            Operation::SelectNextDirectory => {
                if !self.app.library.directories.is_empty() {
                    self.app.selected_directory_index = (self.app.selected_directory_index + 1)
                        .min(self.app.library.directories.len().saturating_sub(1));
                }
            }
            Operation::SelectPreviousDirectory => {
                self.app.selected_directory_index =
                    self.app.selected_directory_index.saturating_sub(1);
            }
            Operation::RemoveSelectedDirectory => {
                if self.app.selected_directory_index < self.app.library.directories.len() {
                    self.app
                        .library
                        .directories
                        .remove(self.app.selected_directory_index);
                    if self.app.selected_directory_index >= self.app.library.directories.len()
                        && !self.app.library.directories.is_empty()
                    {
                        self.app.selected_directory_index =
                            self.app.library.directories.len() - 1;
                    }
                }
            }

            // Device management
            Operation::SelectNextDevice => {
                if !self.app.output_devices.is_empty() {
                    self.app.selected_output_device_index =
                        (self.app.selected_output_device_index + 1)
                            .min(self.app.output_devices.len().saturating_sub(1));
                }
            }
            Operation::SelectPreviousDevice => {
                self.app.selected_output_device_index =
                    self.app.selected_output_device_index.saturating_sub(1);
            }
            Operation::SelectDevice(idx) => {
                if idx < self.app.output_devices.len() {
                    self.app.selected_output_device_index = idx;
                }
            }
        }
    }

    fn load_test_library(&mut self, albums: &[TestAlbum]) {
        self.app.library.albums.clear();
        for test_album in albums {
            self.app.library.albums.push(test_album.to_album());
        }
        self.app.selected_album_index = 0;
        self.app.rebuild_artist_tree();
    }

    fn reset(&mut self) {
        self.app = GpuiApp::new();
    }
}
