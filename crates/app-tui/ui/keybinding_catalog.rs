use crate::app::Screen;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TuiKeyContext {
    Always,
    SharedRoot,
    NormalRoot,
    GlobalMeters,
    LevelMeters,
    Library,
    Queue,
    PluginList,
    AddPlugin,
    PluginEdit,
    PlaylistList,
    PlaylistTracks,
    Devices,
    EarTraining,
    ConfigureTabs,
    Directories,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SharedCommand {
    Quit,
    CycleScreens,
    SwitchScreen,
    ExitApplication,
    FocusLevelMeters,
    CycleLanguage,
    AdjustVolume,
    SelectOutputDevice,
    NavigateMeterGroup,
    NavigateMeterControl,
    ToggleMeterSolo,
    ToggleMute,
    ToggleReplayGain,
    CycleReplayGainMode,
    ShowHelp,
    FocusedMeterGroup,
    FocusedMeterControl,
    FocusedMeterMute,
    FocusedMeterSolo,
    FocusedMeterDim,
    FocusedMeterClear,
    ExitLevelMeters,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LibraryCommand {
    Navigate,
    Page,
    Search,
    ToggleTree,
    ToggleArtist,
    Sort,
    Filter,
    AddToQueue,
    OpenQueue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QueueCommand {
    Navigate,
    PlaySelected,
    ToggleExpanded,
    PlayPause,
    NextTrack,
    PreviousTrack,
    Remove,
    Clear,
    AddToPlaylist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PluginListCommand {
    Navigate,
    Add,
    Edit,
    Toggle,
    Remove,
    MoveUp,
    MoveDown,
    Save,
    Load,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AddPluginCommand {
    Navigate,
    Select,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PluginEditCommand {
    NavigateParameter,
    AdjustSmall,
    AdjustLarge,
    LoadApo,
    LoadSofa,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlaylistCommand {
    Navigate,
    Open,
    Back,
    Create,
    Rename,
    Delete,
    PlayAll,
    RemoveTrack,
    MoveTrack,
    Import,
    Export,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeviceCommand {
    Navigate,
    Select,
    Rescan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EarTrainingCommand {
    SwitchTab,
    StartSession,
    CycleExercise,
    ToggleAdaptive,
    CycleChangeMode,
    AdjustBandCount,
    AdjustGain,
    AdjustQ,
    AdjustTrialCount,
    SelectAnswer,
    Activate,
    NextTrial,
    Audition,
    AddSource,
    NavigateSource,
    SetLoopBoundary,
    ToggleLoop,
    NavigateCourse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ConfigureCommand {
    NavigateTabs,
    OpenTab,
    Exit,
    JumpToTab,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DirectoryCommand {
    Navigate,
    Add,
    Remove,
    Scan,
    ForceScan,
    Maintenance,
    ReplayGain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TuiCommand {
    Shared(SharedCommand),
    Library(LibraryCommand),
    Queue(QueueCommand),
    PluginList(PluginListCommand),
    AddPlugin(AddPluginCommand),
    PluginEdit(PluginEditCommand),
    Playlist(PlaylistCommand),
    Device(DeviceCommand),
    EarTraining(EarTrainingCommand),
    Configure(ConfigureCommand),
    Directory(DirectoryCommand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KeyChord {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyChord {
    const fn plain(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    const fn shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::SHIFT,
        }
    }

    const fn alt(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::ALT,
        }
    }

    const fn control(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::CONTROL,
        }
    }

    const fn super_key(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::SUPER,
        }
    }

    fn matches(self, key: KeyEvent) -> bool {
        if self.code != key.code {
            return false;
        }
        if self.modifiers == key.modifiers {
            return true;
        }

        // Terminals differ on whether Shift is retained after producing an
        // uppercase letter or shifted symbol such as `>` or `?`.
        self.modifiers == KeyModifiers::NONE
            && key.modifiers == KeyModifiers::SHIFT
            && matches!(self.code, KeyCode::Char(c) if !c.is_ascii_lowercase())
    }
}

/// One authoritative TUI help entry. Runtime dispatch resolves the typed
/// chords and command from this record; compact and detailed help consume its
/// descriptions and display label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TuiKeybindingHelp {
    pub key: &'static str,
    pub description: &'static str,
    pub compact_description: Option<&'static str>,
    pub contexts: &'static [TuiKeyContext],
    pub chords: &'static [KeyChord],
    pub command: Option<TuiCommand>,
}

const fn binding(
    key: &'static str,
    description: &'static str,
    compact_description: Option<&'static str>,
    contexts: &'static [TuiKeyContext],
    chords: &'static [KeyChord],
    command: TuiCommand,
) -> TuiKeybindingHelp {
    TuiKeybindingHelp {
        key,
        description,
        compact_description,
        contexts,
        chords,
        command: Some(command),
    }
}

const fn section(key: &'static str, description: &'static str) -> TuiKeybindingHelp {
    TuiKeybindingHelp {
        key,
        description,
        compact_description: None,
        contexts: &[],
        chords: &[],
        command: None,
    }
}

const ALWAYS_KEYBINDINGS: &[TuiKeybindingHelp] = &[binding(
    "Ctrl+C/Ctrl+Q/Cmd+Q",
    "  Quit (ESC quits from main pane)",
    None,
    &[TuiKeyContext::Always],
    &[
        KeyChord::control(KeyCode::Char('c')),
        KeyChord::control(KeyCode::Char('q')),
        KeyChord::super_key(KeyCode::Char('q')),
    ],
    TuiCommand::Shared(SharedCommand::Quit),
)];

const SHARED_ROOT_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "L/Q/P/O/C/Y/N/T",
        "  Jump to Library/Queue/Plugins/Devices/Configure/Playlists/Tools",
        None,
        &[TuiKeyContext::SharedRoot],
        &[
            KeyChord::plain(KeyCode::Char('L')),
            KeyChord::plain(KeyCode::Char('Q')),
            KeyChord::plain(KeyCode::Char('P')),
            KeyChord::plain(KeyCode::Char('O')),
            KeyChord::plain(KeyCode::Char('C')),
            KeyChord::plain(KeyCode::Char('Y')),
            KeyChord::plain(KeyCode::Char('N')),
            KeyChord::plain(KeyCode::Char('T')),
        ],
        TuiCommand::Shared(SharedCommand::SwitchScreen),
    ),
    binding(
        "Alt+L",
        "Cycle language",
        None,
        &[TuiKeyContext::SharedRoot],
        &[KeyChord::alt(KeyCode::Char('l'))],
        TuiCommand::Shared(SharedCommand::CycleLanguage),
    ),
    binding(
        "?",
        "  Show this help",
        None,
        &[TuiKeyContext::SharedRoot],
        &[KeyChord::plain(KeyCode::Char('?'))],
        TuiCommand::Shared(SharedCommand::ShowHelp),
    ),
];

const NORMAL_ROOT_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "Tab",
        "  Cycle through screens and level meters pane",
        None,
        &[TuiKeyContext::NormalRoot],
        &[KeyChord::plain(KeyCode::Tab)],
        TuiCommand::Shared(SharedCommand::CycleScreens),
    ),
    binding(
        "Esc",
        "  Quit (ESC quits from main pane)",
        None,
        &[TuiKeyContext::NormalRoot],
        &[KeyChord::plain(KeyCode::Esc)],
        TuiCommand::Shared(SharedCommand::ExitApplication),
    ),
    binding(
        "Shift+M",
        "  Focus level meters pane",
        None,
        &[TuiKeyContext::NormalRoot],
        &[KeyChord::shift(KeyCode::Char('M'))],
        TuiCommand::Shared(SharedCommand::FocusLevelMeters),
    ),
    binding(
        "+/=",
        "  Increase volume",
        None,
        &[TuiKeyContext::NormalRoot],
        &[
            KeyChord::plain(KeyCode::Char('+')),
            KeyChord::plain(KeyCode::Char('=')),
        ],
        TuiCommand::Shared(SharedCommand::AdjustVolume),
    ),
    binding(
        "-/_",
        "  Decrease volume",
        None,
        &[TuiKeyContext::NormalRoot],
        &[
            KeyChord::plain(KeyCode::Char('-')),
            KeyChord::plain(KeyCode::Char('_')),
        ],
        TuiCommand::Shared(SharedCommand::AdjustVolume),
    ),
    binding(
        "Ctrl+Left/Right",
        "  Select output device",
        None,
        &[TuiKeyContext::NormalRoot],
        &[
            KeyChord::control(KeyCode::Left),
            KeyChord::control(KeyCode::Right),
        ],
        TuiCommand::Shared(SharedCommand::SelectOutputDevice),
    ),
    binding(
        "Shift+Left/Right",
        "  Navigate level meter groups",
        None,
        &[TuiKeyContext::GlobalMeters],
        &[
            KeyChord::shift(KeyCode::Left),
            KeyChord::shift(KeyCode::Right),
        ],
        TuiCommand::Shared(SharedCommand::NavigateMeterGroup),
    ),
    binding(
        "Shift+Up/Down",
        "  Select mute/solo control",
        None,
        &[TuiKeyContext::GlobalMeters],
        &[KeyChord::shift(KeyCode::Up), KeyChord::shift(KeyCode::Down)],
        TuiCommand::Shared(SharedCommand::NavigateMeterControl),
    ),
    binding(
        "Shift+S",
        "  Toggle solo on selected group",
        None,
        &[TuiKeyContext::GlobalMeters],
        &[KeyChord::shift(KeyCode::Char('S'))],
        TuiCommand::Shared(SharedCommand::ToggleMeterSolo),
    ),
    binding(
        "u",
        "  Toggle mute",
        None,
        &[TuiKeyContext::NormalRoot],
        &[KeyChord::plain(KeyCode::Char('u'))],
        TuiCommand::Shared(SharedCommand::ToggleMute),
    ),
    binding(
        "g",
        "  Toggle ReplayGain",
        None,
        &[TuiKeyContext::NormalRoot],
        &[KeyChord::plain(KeyCode::Char('g'))],
        TuiCommand::Shared(SharedCommand::ToggleReplayGain),
    ),
    binding(
        "G",
        "  Cycle ReplayGain mode",
        None,
        &[TuiKeyContext::NormalRoot],
        &[KeyChord::plain(KeyCode::Char('G'))],
        TuiCommand::Shared(SharedCommand::CycleReplayGainMode),
    ),
];

const LEVEL_METER_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "Left/Right",
        "  Navigate between channel groups",
        None,
        &[TuiKeyContext::LevelMeters],
        &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
        ],
        TuiCommand::Shared(SharedCommand::FocusedMeterGroup),
    ),
    binding(
        "Up/Down",
        "  Select mute/solo control",
        None,
        &[TuiKeyContext::LevelMeters],
        &[KeyChord::plain(KeyCode::Up), KeyChord::plain(KeyCode::Down)],
        TuiCommand::Shared(SharedCommand::FocusedMeterControl),
    ),
    binding(
        "m",
        "  Toggle mute/solo on selected group",
        None,
        &[TuiKeyContext::LevelMeters],
        &[KeyChord::plain(KeyCode::Char('m'))],
        TuiCommand::Shared(SharedCommand::FocusedMeterMute),
    ),
    binding(
        "s",
        "  Toggle mute/solo on selected group",
        None,
        &[TuiKeyContext::LevelMeters],
        &[KeyChord::plain(KeyCode::Char('s'))],
        TuiCommand::Shared(SharedCommand::FocusedMeterSolo),
    ),
    binding(
        "d",
        "  Toggle dim on selected group",
        None,
        &[TuiKeyContext::LevelMeters],
        &[KeyChord::plain(KeyCode::Char('d'))],
        TuiCommand::Shared(SharedCommand::FocusedMeterDim),
    ),
    binding(
        "c/C",
        "  Clear all mutes and solos",
        None,
        &[TuiKeyContext::LevelMeters],
        &[
            KeyChord::plain(KeyCode::Char('c')),
            KeyChord::plain(KeyCode::Char('C')),
        ],
        TuiCommand::Shared(SharedCommand::FocusedMeterClear),
    ),
    binding(
        "Esc/Tab",
        "  Return to main pane",
        None,
        &[TuiKeyContext::LevelMeters],
        &[KeyChord::plain(KeyCode::Esc), KeyChord::plain(KeyCode::Tab)],
        TuiCommand::Shared(SharedCommand::ExitLevelMeters),
    ),
];

const LIBRARY_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "↑/↓ or k/j",
        "Navigate albums/artists",
        Some("Browse"),
        &[TuiKeyContext::Library],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::Library(LibraryCommand::Navigate),
    ),
    binding(
        "PageUp/PageDown",
        "Jump by page",
        None,
        &[TuiKeyContext::Library],
        &[
            KeyChord::plain(KeyCode::PageUp),
            KeyChord::plain(KeyCode::PageDown),
        ],
        TuiCommand::Library(LibraryCommand::Page),
    ),
    binding(
        "/",
        "Search albums",
        Some("Search"),
        &[TuiKeyContext::Library],
        &[KeyChord::plain(KeyCode::Char('/'))],
        TuiCommand::Library(LibraryCommand::Search),
    ),
    binding(
        "t",
        "Toggle tree view / flat view",
        Some("Tree/flat view"),
        &[TuiKeyContext::Library],
        &[KeyChord::plain(KeyCode::Char('t'))],
        TuiCommand::Library(LibraryCommand::ToggleTree),
    ),
    binding(
        "h/l or ←/→",
        "Collapse/expand artists in tree view",
        None,
        &[TuiKeyContext::Library],
        &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
            KeyChord::plain(KeyCode::Char('h')),
            KeyChord::plain(KeyCode::Char('l')),
        ],
        TuiCommand::Library(LibraryCommand::ToggleArtist),
    ),
    binding(
        "s or 1/2/3/4",
        "Sort: cycle / Year / Genre / Artist / Album",
        Some("Sort"),
        &[TuiKeyContext::Library],
        &[
            KeyChord::plain(KeyCode::Char('s')),
            KeyChord::plain(KeyCode::Char('1')),
            KeyChord::plain(KeyCode::Char('2')),
            KeyChord::plain(KeyCode::Char('3')),
            KeyChord::plain(KeyCode::Char('4')),
        ],
        TuiCommand::Library(LibraryCommand::Sort),
    ),
    binding(
        "c or 5/6/7/8/9",
        "Filter: All/Mono/Stereo/Surround/Mixed",
        Some("Filter"),
        &[TuiKeyContext::Library],
        &[
            KeyChord::plain(KeyCode::Char('c')),
            KeyChord::plain(KeyCode::Char('5')),
            KeyChord::plain(KeyCode::Char('6')),
            KeyChord::plain(KeyCode::Char('7')),
            KeyChord::plain(KeyCode::Char('8')),
            KeyChord::plain(KeyCode::Char('9')),
        ],
        TuiCommand::Library(LibraryCommand::Filter),
    ),
    binding(
        "a or Enter",
        "Add album to queue",
        Some("Queue album"),
        &[TuiKeyContext::Library],
        &[
            KeyChord::plain(KeyCode::Char('a')),
            KeyChord::plain(KeyCode::Enter),
        ],
        TuiCommand::Library(LibraryCommand::AddToQueue),
    ),
    binding(
        "q",
        "Go to queue screen",
        Some("Queue"),
        &[TuiKeyContext::Library],
        &[KeyChord::plain(KeyCode::Char('q'))],
        TuiCommand::Library(LibraryCommand::OpenQueue),
    ),
];

const CONFIGURE_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "←/→ or ↑/↓ or Tab/BackTab",
        "Navigate tabs",
        Some("Navigate tabs"),
        &[TuiKeyContext::ConfigureTabs],
        &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Tab),
            KeyChord::plain(KeyCode::BackTab),
        ],
        TuiCommand::Configure(ConfigureCommand::NavigateTabs),
    ),
    binding(
        "Enter",
        "Open tab",
        Some("Open tab"),
        &[TuiKeyContext::ConfigureTabs],
        &[KeyChord::plain(KeyCode::Enter)],
        TuiCommand::Configure(ConfigureCommand::OpenTab),
    ),
    binding(
        "Esc",
        "Back",
        Some("Back"),
        &[TuiKeyContext::ConfigureTabs],
        &[KeyChord::plain(KeyCode::Esc)],
        TuiCommand::Configure(ConfigureCommand::Exit),
    ),
    binding(
        "1-8",
        "Jump to tab",
        Some("Jump to tab"),
        &[TuiKeyContext::ConfigureTabs],
        &[
            KeyChord::plain(KeyCode::Char('1')),
            KeyChord::plain(KeyCode::Char('2')),
            KeyChord::plain(KeyCode::Char('3')),
            KeyChord::plain(KeyCode::Char('4')),
            KeyChord::plain(KeyCode::Char('5')),
            KeyChord::plain(KeyCode::Char('6')),
            KeyChord::plain(KeyCode::Char('7')),
            KeyChord::plain(KeyCode::Char('8')),
        ],
        TuiCommand::Configure(ConfigureCommand::JumpToTab),
    ),
    binding(
        "?",
        "Help",
        Some("Help"),
        &[TuiKeyContext::ConfigureTabs],
        &[KeyChord::plain(KeyCode::Char('?'))],
        TuiCommand::Configure(ConfigureCommand::Help),
    ),
    section("", ""),
    section("DIRECTORIES:", "(when on Directories sub-screen)"),
    binding(
        "↑/↓ or k/j",
        "Navigate directories",
        None,
        &[TuiKeyContext::Directories],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::Directory(DirectoryCommand::Navigate),
    ),
    binding(
        "a or F2",
        "Add directory",
        None,
        &[TuiKeyContext::Directories],
        &[
            KeyChord::plain(KeyCode::Char('a')),
            KeyChord::plain(KeyCode::F(2)),
        ],
        TuiCommand::Directory(DirectoryCommand::Add),
    ),
    binding(
        "d/Delete",
        "Remove selected directory",
        None,
        &[TuiKeyContext::Directories],
        &[
            KeyChord::plain(KeyCode::Char('d')),
            KeyChord::plain(KeyCode::Delete),
        ],
        TuiCommand::Directory(DirectoryCommand::Remove),
    ),
    binding(
        "s",
        "Scan library (incremental)",
        None,
        &[TuiKeyContext::Directories],
        &[KeyChord::plain(KeyCode::Char('s'))],
        TuiCommand::Directory(DirectoryCommand::Scan),
    ),
    binding(
        "S",
        "Force rescan ALL files (preserves ReplayGain)",
        None,
        &[TuiKeyContext::Directories],
        &[KeyChord::plain(KeyCode::Char('S'))],
        TuiCommand::Directory(DirectoryCommand::ForceScan),
    ),
    binding(
        "m",
        "Database maintenance (clean missing files)",
        None,
        &[TuiKeyContext::Directories],
        &[KeyChord::plain(KeyCode::Char('m'))],
        TuiCommand::Directory(DirectoryCommand::Maintenance),
    ),
    binding(
        "r",
        "Analyze ReplayGain for all tracks",
        None,
        &[TuiKeyContext::Directories],
        &[KeyChord::plain(KeyCode::Char('r'))],
        TuiCommand::Directory(DirectoryCommand::ReplayGain),
    ),
];

const QUEUE_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "↑/↓ or k/j",
        "Navigate queue items",
        Some("Browse"),
        &[TuiKeyContext::Queue],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::Queue(QueueCommand::Navigate),
    ),
    binding(
        "Enter",
        "Play selected album from start",
        Some("Play selection"),
        &[TuiKeyContext::Queue],
        &[KeyChord::plain(KeyCode::Enter)],
        TuiCommand::Queue(QueueCommand::PlaySelected),
    ),
    binding(
        "h/l or ←/→",
        "Expand/collapse album tracks",
        None,
        &[TuiKeyContext::Queue],
        &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
            KeyChord::plain(KeyCode::Char('h')),
            KeyChord::plain(KeyCode::Char('l')),
        ],
        TuiCommand::Queue(QueueCommand::ToggleExpanded),
    ),
    binding(
        "p/Space",
        "Play/pause",
        Some("Play/pause"),
        &[TuiKeyContext::Queue],
        &[
            KeyChord::plain(KeyCode::Char('p')),
            KeyChord::plain(KeyCode::Char(' ')),
        ],
        TuiCommand::Queue(QueueCommand::PlayPause),
    ),
    binding(
        "n or >",
        "Next track",
        None,
        &[TuiKeyContext::Queue],
        &[
            KeyChord::plain(KeyCode::Char('n')),
            KeyChord::plain(KeyCode::Char('>')),
        ],
        TuiCommand::Queue(QueueCommand::NextTrack),
    ),
    binding(
        "b or <",
        "Previous track",
        None,
        &[TuiKeyContext::Queue],
        &[
            KeyChord::plain(KeyCode::Char('b')),
            KeyChord::plain(KeyCode::Char('<')),
        ],
        TuiCommand::Queue(QueueCommand::PreviousTrack),
    ),
    binding(
        "d/Delete",
        "Remove from queue",
        Some("Remove track"),
        &[TuiKeyContext::Queue],
        &[
            KeyChord::plain(KeyCode::Char('d')),
            KeyChord::plain(KeyCode::Delete),
        ],
        TuiCommand::Queue(QueueCommand::Remove),
    ),
    binding(
        "c",
        "Clear entire queue",
        Some("Clear"),
        &[TuiKeyContext::Queue],
        &[KeyChord::plain(KeyCode::Char('c'))],
        TuiCommand::Queue(QueueCommand::Clear),
    ),
    binding(
        "A",
        "Add album (or selected track) to active playlist",
        Some("Add active playlist"),
        &[TuiKeyContext::Queue],
        &[KeyChord::plain(KeyCode::Char('A'))],
        TuiCommand::Queue(QueueCommand::AddToPlaylist),
    ),
];

const PLUGIN_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "↑/↓ or k/j",
        "Navigate plugin chain",
        Some("Browse"),
        &[TuiKeyContext::PluginList],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::PluginList(PluginListCommand::Navigate),
    ),
    binding(
        "a",
        "Add plugin (opens selection dialog)",
        Some("Add plugin"),
        &[TuiKeyContext::PluginList],
        &[KeyChord::plain(KeyCode::Char('a'))],
        TuiCommand::PluginList(PluginListCommand::Add),
    ),
    binding(
        "e or Enter",
        "Edit selected plugin",
        Some("Edit"),
        &[TuiKeyContext::PluginList],
        &[
            KeyChord::plain(KeyCode::Char('e')),
            KeyChord::plain(KeyCode::Enter),
        ],
        TuiCommand::PluginList(PluginListCommand::Edit),
    ),
    binding(
        "t",
        "Toggle plugin enabled/disabled",
        Some("Enable/disable"),
        &[TuiKeyContext::PluginList],
        &[KeyChord::plain(KeyCode::Char('t'))],
        TuiCommand::PluginList(PluginListCommand::Toggle),
    ),
    binding(
        "d/Delete",
        "Remove plugin",
        Some("Remove plugin"),
        &[TuiKeyContext::PluginList],
        &[
            KeyChord::plain(KeyCode::Char('d')),
            KeyChord::plain(KeyCode::Delete),
        ],
        TuiCommand::PluginList(PluginListCommand::Remove),
    ),
    binding(
        "u/U or Shift+↑",
        "Move plugin up in chain",
        None,
        &[TuiKeyContext::PluginList],
        &[
            KeyChord::plain(KeyCode::Char('u')),
            KeyChord::plain(KeyCode::Char('U')),
            KeyChord::shift(KeyCode::Up),
        ],
        TuiCommand::PluginList(PluginListCommand::MoveUp),
    ),
    binding(
        "w/W or Shift+↓",
        "Move plugin down in chain",
        None,
        &[TuiKeyContext::PluginList],
        &[
            KeyChord::plain(KeyCode::Char('w')),
            KeyChord::plain(KeyCode::Char('W')),
            KeyChord::shift(KeyCode::Down),
        ],
        TuiCommand::PluginList(PluginListCommand::MoveDown),
    ),
    binding(
        "s",
        "Save plugin chain to file",
        Some("Save"),
        &[TuiKeyContext::PluginList],
        &[KeyChord::plain(KeyCode::Char('s'))],
        TuiCommand::PluginList(PluginListCommand::Save),
    ),
    binding(
        "l",
        "Load plugin chain from file",
        Some("Load"),
        &[TuiKeyContext::PluginList],
        &[KeyChord::plain(KeyCode::Char('l'))],
        TuiCommand::PluginList(PluginListCommand::Load),
    ),
    section("", ""),
    section("ADD PLUGIN:", "(↑/↓ navigate, Enter select, Esc cancel)"),
    binding(
        "↑/↓ or k/j",
        "Navigate plugin chain",
        None,
        &[TuiKeyContext::AddPlugin],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::AddPlugin(AddPluginCommand::Navigate),
    ),
    binding(
        "Enter",
        "Add plugin",
        None,
        &[TuiKeyContext::AddPlugin],
        &[KeyChord::plain(KeyCode::Enter)],
        TuiCommand::AddPlugin(AddPluginCommand::Select),
    ),
    binding(
        "Esc",
        "Back",
        None,
        &[TuiKeyContext::AddPlugin],
        &[KeyChord::plain(KeyCode::Esc)],
        TuiCommand::AddPlugin(AddPluginCommand::Cancel),
    ),
    section("", ""),
    section("EDIT MODE:", "(when editing a plugin)"),
    binding(
        "↑/↓ or k/j",
        "Navigate parameters",
        None,
        &[TuiKeyContext::PluginEdit],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::PluginEdit(PluginEditCommand::NavigateParameter),
    ),
    binding(
        "←/→ or h/l",
        "Adjust parameter value (small)",
        None,
        &[TuiKeyContext::PluginEdit],
        &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
            KeyChord::plain(KeyCode::Char('h')),
            KeyChord::plain(KeyCode::Char('l')),
        ],
        TuiCommand::PluginEdit(PluginEditCommand::AdjustSmall),
    ),
    binding(
        "[/]",
        "Adjust parameter value (large)",
        None,
        &[TuiKeyContext::PluginEdit],
        &[
            KeyChord::plain(KeyCode::Char('[')),
            KeyChord::plain(KeyCode::Char(']')),
        ],
        TuiCommand::PluginEdit(PluginEditCommand::AdjustLarge),
    ),
    binding(
        "a",
        "Load APO file (EQ plugins only)",
        None,
        &[TuiKeyContext::PluginEdit],
        &[KeyChord::plain(KeyCode::Char('a'))],
        TuiCommand::PluginEdit(PluginEditCommand::LoadApo),
    ),
    binding(
        "o",
        "Load SOFA file (Binaural only)",
        None,
        &[TuiKeyContext::PluginEdit],
        &[KeyChord::plain(KeyCode::Char('o'))],
        TuiCommand::PluginEdit(PluginEditCommand::LoadSofa),
    ),
    binding(
        "Esc",
        "Exit edit mode",
        None,
        &[TuiKeyContext::PluginEdit],
        &[KeyChord::plain(KeyCode::Esc)],
        TuiCommand::PluginEdit(PluginEditCommand::Exit),
    ),
];

const PLAYLIST_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "↑/↓ or k/j",
        "Navigate playlists/tracks",
        Some("Browse"),
        &[TuiKeyContext::PlaylistList, TuiKeyContext::PlaylistTracks],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::Playlist(PlaylistCommand::Navigate),
    ),
    binding(
        "Enter or l",
        "Open playlist",
        Some("Open"),
        &[TuiKeyContext::PlaylistList],
        &[
            KeyChord::plain(KeyCode::Enter),
            KeyChord::plain(KeyCode::Char('l')),
        ],
        TuiCommand::Playlist(PlaylistCommand::Open),
    ),
    binding(
        "Esc or h",
        "Close playlist (back to list)",
        Some("Back"),
        &[TuiKeyContext::PlaylistTracks],
        &[
            KeyChord::plain(KeyCode::Esc),
            KeyChord::plain(KeyCode::Char('h')),
        ],
        TuiCommand::Playlist(PlaylistCommand::Back),
    ),
    binding(
        "n",
        "Create new playlist",
        Some("Create/rename/delete"),
        &[TuiKeyContext::PlaylistList],
        &[KeyChord::plain(KeyCode::Char('n'))],
        TuiCommand::Playlist(PlaylistCommand::Create),
    ),
    binding(
        "r",
        "Rename selected playlist",
        None,
        &[TuiKeyContext::PlaylistList],
        &[KeyChord::plain(KeyCode::Char('r'))],
        TuiCommand::Playlist(PlaylistCommand::Rename),
    ),
    binding(
        "d",
        "Delete selected playlist",
        None,
        &[TuiKeyContext::PlaylistList],
        &[KeyChord::plain(KeyCode::Char('d'))],
        TuiCommand::Playlist(PlaylistCommand::Delete),
    ),
    binding(
        "p",
        "Play all tracks",
        Some("Play all"),
        &[TuiKeyContext::PlaylistList, TuiKeyContext::PlaylistTracks],
        &[KeyChord::plain(KeyCode::Char('p'))],
        TuiCommand::Playlist(PlaylistCommand::PlayAll),
    ),
    binding(
        "x",
        "Remove track (in tracks view)",
        None,
        &[TuiKeyContext::PlaylistTracks],
        &[KeyChord::plain(KeyCode::Char('x'))],
        TuiCommand::Playlist(PlaylistCommand::RemoveTrack),
    ),
    binding(
        "K/J",
        "Move track up/down",
        None,
        &[TuiKeyContext::PlaylistTracks],
        &[
            KeyChord::plain(KeyCode::Char('K')),
            KeyChord::plain(KeyCode::Char('J')),
        ],
        TuiCommand::Playlist(PlaylistCommand::MoveTrack),
    ),
    binding(
        "i",
        "Import M3U playlist",
        None,
        &[TuiKeyContext::PlaylistList],
        &[KeyChord::plain(KeyCode::Char('i'))],
        TuiCommand::Playlist(PlaylistCommand::Import),
    ),
    binding(
        "e",
        "Export playlist to M3U",
        None,
        &[TuiKeyContext::PlaylistList],
        &[KeyChord::plain(KeyCode::Char('e'))],
        TuiCommand::Playlist(PlaylistCommand::Export),
    ),
];

const DEVICE_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "↑/↓ or k/j",
        "Navigate output devices",
        Some("Browse"),
        &[TuiKeyContext::Devices],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::Device(DeviceCommand::Navigate),
    ),
    binding(
        "Enter/Space",
        "Select output device",
        Some("Select"),
        &[TuiKeyContext::Devices],
        &[
            KeyChord::plain(KeyCode::Enter),
            KeyChord::plain(KeyCode::Char(' ')),
        ],
        TuiCommand::Device(DeviceCommand::Select),
    ),
    binding(
        "r/R",
        "Rescan audio and cast devices",
        Some("Rescan"),
        &[TuiKeyContext::Devices],
        &[
            KeyChord::plain(KeyCode::Char('r')),
            KeyChord::plain(KeyCode::Char('R')),
        ],
        TuiCommand::Device(DeviceCommand::Rescan),
    ),
];

const EAR_TRAINING_KEYBINDINGS: &[TuiKeybindingHelp] = &[
    binding(
        "F1/F2/F3",
        "Practice / Courses / Progress",
        Some("Practice/Courses/Progress"),
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::F(1)),
            KeyChord::plain(KeyCode::F(2)),
            KeyChord::plain(KeyCode::F(3)),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::SwitchTab),
    ),
    binding(
        "s",
        "Start or restart session",
        Some("Start session"),
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Char('s'))],
        TuiCommand::EarTraining(EarTrainingCommand::StartSession),
    ),
    binding(
        "1/2",
        "Listen to original / filtered",
        Some("Original/filtered"),
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Char('1')),
            KeyChord::plain(KeyCode::Char('2')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::Audition),
    ),
    binding(
        "←/→ or h/l",
        "Select answer",
        Some("Choose/submit"),
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Left),
            KeyChord::plain(KeyCode::Right),
            KeyChord::plain(KeyCode::Char('h')),
            KeyChord::plain(KeyCode::Char('l')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::SelectAnswer),
    ),
    binding(
        "Enter",
        "Submit answer / next trial",
        None,
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Enter)],
        TuiCommand::EarTraining(EarTrainingCommand::Activate),
    ),
    binding(
        "n",
        "Submit answer / next trial",
        None,
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Char('n'))],
        TuiCommand::EarTraining(EarTrainingCommand::NextTrial),
    ),
    binding(
        "e",
        "Exercise / adaptive / boost-cut mode",
        Some("Exercise/adaptive/change"),
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Char('e'))],
        TuiCommand::EarTraining(EarTrainingCommand::CycleExercise),
    ),
    binding(
        "a",
        "Exercise / adaptive / boost-cut mode",
        None,
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Char('a'))],
        TuiCommand::EarTraining(EarTrainingCommand::ToggleAdaptive),
    ),
    binding(
        "c",
        "Exercise / adaptive / boost-cut mode",
        None,
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Char('c'))],
        TuiCommand::EarTraining(EarTrainingCommand::CycleChangeMode),
    ),
    binding(
        "b/B",
        "Adjust bands, gain, Q, trials",
        None,
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Char('b')),
            KeyChord::plain(KeyCode::Char('B')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::AdjustBandCount),
    ),
    binding(
        "g/G",
        "Adjust bands, gain, Q, trials",
        None,
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Char('g')),
            KeyChord::plain(KeyCode::Char('G')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::AdjustGain),
    ),
    binding(
        "v/V",
        "Adjust bands, gain, Q, trials",
        None,
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Char('v')),
            KeyChord::plain(KeyCode::Char('V')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::AdjustQ),
    ),
    binding(
        "t/T",
        "Adjust bands, gain, Q, trials",
        None,
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Char('t')),
            KeyChord::plain(KeyCode::Char('T')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::AdjustTrialCount),
    ),
    binding(
        "i",
        "Add / previous / next training source",
        None,
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Char('i'))],
        TuiCommand::EarTraining(EarTrainingCommand::AddSource),
    ),
    binding(
        ", / .",
        "Add / previous / next training source",
        None,
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Char(',')),
            KeyChord::plain(KeyCode::Char('.')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::NavigateSource),
    ),
    binding(
        "[ / ]",
        "Set loop bounds / toggle loop",
        Some("Loop controls"),
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Char('[')),
            KeyChord::plain(KeyCode::Char(']')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::SetLoopBoundary),
    ),
    binding(
        "\\",
        "Set loop bounds / toggle loop",
        None,
        &[TuiKeyContext::EarTraining],
        &[KeyChord::plain(KeyCode::Char('\\'))],
        TuiCommand::EarTraining(EarTrainingCommand::ToggleLoop),
    ),
    binding(
        "↑/↓ or k/j",
        "Navigate courses",
        None,
        &[TuiKeyContext::EarTraining],
        &[
            KeyChord::plain(KeyCode::Up),
            KeyChord::plain(KeyCode::Down),
            KeyChord::plain(KeyCode::Char('k')),
            KeyChord::plain(KeyCode::Char('j')),
        ],
        TuiCommand::EarTraining(EarTrainingCommand::NavigateCourse),
    ),
    section("Esc", "Return to library and clean audition path"),
];

const ALL_CATALOGS: &[&[TuiKeybindingHelp]] = &[
    ALWAYS_KEYBINDINGS,
    SHARED_ROOT_KEYBINDINGS,
    NORMAL_ROOT_KEYBINDINGS,
    LEVEL_METER_KEYBINDINGS,
    LIBRARY_KEYBINDINGS,
    CONFIGURE_KEYBINDINGS,
    QUEUE_KEYBINDINGS,
    PLUGIN_KEYBINDINGS,
    PLAYLIST_KEYBINDINGS,
    DEVICE_KEYBINDINGS,
    EAR_TRAINING_KEYBINDINGS,
];

pub(super) fn keybindings_for_contexts(
    contexts: &[TuiKeyContext],
) -> Vec<&'static TuiKeybindingHelp> {
    ALL_CATALOGS
        .iter()
        .flat_map(|catalog| catalog.iter())
        .filter(|binding| {
            binding.command.is_some()
                && binding
                    .contexts
                    .iter()
                    .any(|context| contexts.contains(context))
        })
        .collect()
}

pub(super) fn keybindings_for_screen(screen: Screen) -> &'static [TuiKeybindingHelp] {
    match screen {
        Screen::Loading => &[],
        Screen::Library => LIBRARY_KEYBINDINGS,
        Screen::Configure => CONFIGURE_KEYBINDINGS,
        Screen::Queue => QUEUE_KEYBINDINGS,
        Screen::Plugins => PLUGIN_KEYBINDINGS,
        Screen::Playlists => PLAYLIST_KEYBINDINGS,
        Screen::Devices => DEVICE_KEYBINDINGS,
        Screen::Tools | Screen::AbTesting => &[],
        Screen::EarTraining => EAR_TRAINING_KEYBINDINGS,
    }
}

fn matching_commands(context: TuiKeyContext, key: KeyEvent) -> Vec<TuiCommand> {
    ALL_CATALOGS
        .iter()
        .flat_map(|catalog| catalog.iter())
        .filter(|binding| binding.contexts.contains(&context))
        .filter(|binding| binding.chords.iter().any(|chord| chord.matches(key)))
        .filter_map(|binding| binding.command)
        .collect()
}

pub(crate) fn resolve_command(context: TuiKeyContext, key: KeyEvent) -> Option<TuiCommand> {
    let matches = matching_commands(context, key);
    debug_assert!(
        matches.len() <= 1,
        "conflicting TUI bindings for {context:?} and {key:?}: {matches:?}"
    );
    matches.first().copied()
}

#[cfg(test)]
fn validate_keybinding_registry() -> Result<(), String> {
    let bindings = ALL_CATALOGS
        .iter()
        .flat_map(|catalog| catalog.iter())
        .filter(|binding| binding.command.is_some())
        .collect::<Vec<_>>();

    for binding in &bindings {
        if binding.key.is_empty() || binding.description.is_empty() {
            return Err(format!("binding metadata is empty: {binding:?}"));
        }
        if binding.contexts.is_empty() || binding.chords.is_empty() {
            return Err(format!("binding has no context or chord: {binding:?}"));
        }
    }

    for (index, binding) in bindings.iter().enumerate() {
        for other in bindings.iter().skip(index + 1) {
            for context in binding.contexts {
                if !other.contexts.contains(context) {
                    continue;
                }
                for chord in binding.chords {
                    if other.chords.contains(chord) {
                        return Err(format!(
                            "duplicate binding in {context:?} for {chord:?}: {:?} and {:?}",
                            binding.command, other.command
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_no_duplicate_or_incomplete_bindings() {
        validate_keybinding_registry().expect("valid TUI binding registry");
    }

    #[test]
    fn every_documented_chord_resolves_to_exactly_one_intended_command() {
        for binding in ALL_CATALOGS
            .iter()
            .flat_map(|catalog| catalog.iter())
            .filter(|binding| binding.command.is_some())
        {
            for context in binding.contexts {
                for chord in binding.chords {
                    let key = KeyEvent::new(chord.code, chord.modifiers);
                    let matches = matching_commands(*context, key);
                    assert_eq!(
                        matches,
                        vec![binding.command.expect("documented command")],
                        "{context:?} {} did not resolve exactly once",
                        binding.key
                    );
                }
            }
        }
    }
}
