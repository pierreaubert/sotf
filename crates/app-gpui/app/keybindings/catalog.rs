use crate::app::{Screen, actions};
use gpui::{Action, KeyBinding};

use super::{DocumentedKeybinding, KeybindingCategory};

struct DocumentationSpec {
    action_names: Vec<&'static str>,
    description: &'static str,
    category: KeybindingCategory,
    palette_action_name: Option<&'static str>,
}

impl DocumentationSpec {
    fn help_only(mut self) -> Self {
        self.palette_action_name = None;
        self
    }
}

macro_rules! documentation {
    ($category:ident, $description:literal, $first:path $(, $action:path)* $(,)?) => {
        DocumentationSpec {
            action_names: vec![Action::name(&$first), $(Action::name(&$action)),*],
            description: $description,
            category: KeybindingCategory::$category,
            palette_action_name: Some(Action::name(&$first)),
        }
    };
}

/// Build help rows from the exact runtime bindings. The catalog owns only
/// semantic grouping/copy; key labels can therefore never drift from what
/// GPUI actually registered for the selected preset.
pub(super) fn documented_keybindings_from_runtime(
    runtime_bindings: &[KeyBinding],
) -> Vec<DocumentedKeybinding> {
    documented_keybindings_from_specs(runtime_bindings, documentation_specs())
}

/// Build the contextual help for one screen from the same runtime bindings
/// registered with GPUI. Screen specifications own only semantic grouping and
/// copy; they never own key strings.
pub(super) fn documented_keybindings_for_screen_from_runtime(
    screen: Screen,
    runtime_bindings: &[KeyBinding],
) -> Vec<DocumentedKeybinding> {
    documented_keybindings_from_specs(runtime_bindings, screen_documentation_specs(screen))
}

fn documented_keybindings_from_specs(
    runtime_bindings: &[KeyBinding],
    specs: Vec<DocumentationSpec>,
) -> Vec<DocumentedKeybinding> {
    specs
        .into_iter()
        .filter_map(|spec| {
            let mut keys = runtime_bindings
                .iter()
                .filter(|binding| spec.action_names.contains(&binding.action().name()))
                .map(|binding| (binding_key_label(binding), binding_raw_key_spec(binding)))
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| {
                key_display_rank(&left.0)
                    .cmp(&key_display_rank(&right.0))
                    .then_with(|| left.0.cmp(&right.0))
            });
            keys.dedup_by(|left, right| left.0 == right.0);
            let raw_key_spec = keys.first().map(|(_, raw)| raw.clone()).unwrap_or_default();
            let action_name = spec.palette_action_name.filter(|action_name| {
                runtime_bindings
                    .iter()
                    .any(|binding| binding.action().name() == *action_name)
            });
            (!keys.is_empty()).then(|| DocumentedKeybinding {
                key: keys
                    .into_iter()
                    .map(|(label, _)| label)
                    .collect::<Vec<_>>()
                    .join(" / "),
                raw_key_spec,
                description: spec.description,
                category: spec.category,
                action_name,
            })
        })
        .collect()
}

pub(super) fn missing_documented_commands(runtime_bindings: &[KeyBinding]) -> Vec<&'static str> {
    documentation_specs()
        .into_iter()
        .filter(|spec| {
            !runtime_bindings
                .iter()
                .any(|binding| spec.action_names.contains(&binding.action().name()))
        })
        .map(|spec| spec.description)
        .collect()
}

fn documentation_specs() -> Vec<DocumentationSpec> {
    vec![
        documentation!(Playback, "Play/Pause", actions::PlayPause),
        documentation!(Playback, "Next track", actions::NextTrack),
        documentation!(Playback, "Previous track", actions::PrevTrack),
        documentation!(
            Playback,
            "Volume up",
            actions::VolumeUp,
            actions::VolumeUpSmall,
            actions::VolumeUpLarge,
            actions::VolumeMax,
        ),
        documentation!(
            Playback,
            "Volume down",
            actions::VolumeDown,
            actions::VolumeDownSmall,
            actions::VolumeDownLarge,
            actions::VolumeMin,
        ),
        documentation!(
            Navigation,
            "Select next",
            actions::SelectNext,
            actions::SelectDown,
        ),
        documentation!(
            Navigation,
            "Select previous",
            actions::SelectPrev,
            actions::SelectUp,
        ),
        documentation!(
            Navigation,
            "Expand/collapse",
            actions::SelectLeft,
            actions::SelectRight,
            actions::ToggleExpand,
        )
        .help_only(),
        documentation!(
            Navigation,
            "Page up/down",
            actions::SelectPrevPage,
            actions::SelectNextPage,
        )
        .help_only(),
        documentation!(
            Navigation,
            "Previous/next page",
            actions::PrevPage,
            actions::NextPage,
        )
        .help_only(),
        documentation!(
            Navigation,
            "Navigate between steps",
            actions::PreviousWorkflowStep,
        ),
        documentation!(
            Navigation,
            "Proceed to next step or finish",
            actions::NextWorkflowStep,
        ),
        documentation!(ScreenSwitch, "Library", actions::SwitchToLibrary),
        documentation!(ScreenSwitch, "Queue", actions::SwitchToQueue),
        documentation!(ScreenSwitch, "Plugins", actions::SwitchToStudio),
        documentation!(ScreenSwitch, "Devices", actions::SwitchToDevices),
        documentation!(
            ScreenSwitch,
            "Directory Manager",
            actions::SwitchToDirectories,
        ),
        documentation!(
            ScreenSwitch,
            "Settings",
            actions::SwitchToSettings,
            actions::OpenConfig,
        ),
        documentation!(Library, "Search", actions::ToggleSearch),
        documentation!(Library, "Toggle tree/list view", actions::ToggleLibraryView,),
        documentation!(Library, "Cycle sort order", actions::CycleSortOrder),
        documentation!(Library, "Cycle channel filter", actions::CycleChannelFilter,),
        documentation!(
            Library,
            "Set sort (Artist/Album/Title/Year)",
            actions::SetSortArtist,
            actions::SetSortAlbum,
            actions::SetSortTitle,
            actions::SetSortYear,
        )
        .help_only(),
        documentation!(
            Library,
            "Set filter (All/Mono/Stereo/Multi/Mixed)",
            actions::SetFilterAll,
            actions::SetFilterMono,
            actions::SetFilterStereo,
            actions::SetFilterSurround,
            actions::SetFilterSurround71,
            actions::SetFilterSurroundPlus,
        )
        .help_only(),
        documentation!(Library, "Add to queue", actions::Enter),
        documentation!(Library, "Add library directory", actions::AddDirectory),
        documentation!(Library, "Scan library", actions::ScanLibrary),
        documentation!(Queue, "Remove item", actions::RemoveItem),
        documentation!(Plugins, "Move plugin up", actions::MovePluginUp),
        documentation!(Plugins, "Move plugin down", actions::MovePluginDown),
        documentation!(Plugins, "Toggle on/off", actions::TogglePlugin),
        documentation!(
            Plugins,
            "Quick add plugins",
            actions::QuickAddEQ,
            actions::QuickAddUpmixer,
            actions::QuickAddCompressor,
            actions::QuickAddGate,
            actions::QuickAddLimiter,
            actions::QuickAddLoudness,
            actions::QuickAddBinaural,
            actions::QuickAddDownmix,
            actions::QuickAddMonoToStereo,
            actions::QuickAddSpectrum,
        )
        .help_only(),
        documentation!(
            ListeningTests,
            "Capture current chain as path A or B",
            actions::ListeningCapturePathA,
            actions::ListeningCapturePathB,
        )
        .help_only(),
        documentation!(
            ListeningTests,
            "Prepare deterministic level and latency matching",
            actions::ListeningPrepare,
        ),
        documentation!(
            ListeningTests,
            "Start blind A/B or ABX trial",
            actions::ListeningStartBlindAb,
            actions::ListeningStartAbx,
        )
        .help_only(),
        documentation!(
            ListeningTests,
            "Play available trial cues",
            actions::ListeningPlayCue1,
            actions::ListeningPlayCue2,
            actions::ListeningPlayCue3,
        )
        .help_only(),
        documentation!(
            ListeningTests,
            "Commit the first/A or second/B answer",
            actions::ListeningCommitAnswer1,
            actions::ListeningCommitAnswer2,
        )
        .help_only(),
        documentation!(
            LevelMeters,
            "Next/prev meter group",
            actions::SelectNextMeterGroup,
            actions::SelectPrevMeterGroup,
        )
        .help_only(),
        documentation!(LevelMeters, "Toggle mute", actions::ToggleMeterMute),
        documentation!(LevelMeters, "Toggle solo", actions::ToggleMeterSolo),
        documentation!(LevelMeters, "Toggle dim", actions::ToggleMeterDim),
        documentation!(
            LevelMeters,
            "Clear mutes/solos",
            actions::ClearMeterMutesSolos,
        ),
        documentation!(System, "Cycle theme", actions::CycleTheme),
        documentation!(System, "Cycle language", actions::CycleLanguage),
        documentation!(
            System,
            "Show help",
            actions::ToggleHelp,
            actions::ToggleHelpSupport,
        )
        .help_only(),
        documentation!(System, "Command palette", actions::ToggleCommandPalette,),
        documentation!(System, "Screen guide", actions::ToggleScreenGuide),
        documentation!(System, "Cancel/close", actions::Cancel),
        documentation!(System, "Quit", actions::QuitApp),
        documentation!(System, "Increase font size", actions::IncreaseFontSize),
        documentation!(System, "Decrease font size", actions::DecreaseFontSize),
        documentation!(System, "Reset font size", actions::ResetFontSize),
        documentation!(
            Plugins,
            "Select next / previous node",
            actions::GraphSelectNextNode,
            actions::GraphSelectPreviousNode,
        ),
        documentation!(
            Plugins,
            "Choose the plugin added by A",
            actions::GraphSelectPreviousPluginType,
            actions::GraphSelectNextPluginType,
        ),
        documentation!(
            Plugins,
            "Add selected plugin",
            actions::GraphAddSelectedPlugin
        ),
        documentation!(
            Plugins,
            "Edit selected plugin",
            actions::GraphEditSelectedNode,
        ),
        documentation!(
            Plugins,
            "Toggle selected plugin bypass",
            actions::GraphToggleSelectedBypass,
        ),
        documentation!(
            Plugins,
            "Arm source / connect to selected node",
            actions::GraphConnectSelectedNode,
        ),
        documentation!(
            Plugins,
            "Select connection port",
            actions::GraphSelectPreviousPort,
            actions::GraphSelectNextPort,
        ),
        documentation!(
            Plugins,
            "Disconnect selected node",
            actions::GraphDisconnectSelectedNode,
        ),
        documentation!(
            Plugins,
            "Move selected node",
            actions::GraphMoveSelectedLeft,
            actions::GraphMoveSelectedRight,
            actions::GraphMoveSelectedUp,
            actions::GraphMoveSelectedDown,
            actions::GraphMoveSelectedLeftLarge,
            actions::GraphMoveSelectedRightLarge,
            actions::GraphMoveSelectedUpLarge,
            actions::GraphMoveSelectedDownLarge,
        ),
        documentation!(
            Plugins,
            "Remove selected plugin",
            actions::GraphRemoveSelectedNode,
        ),
    ]
}

fn screen_documentation_specs(screen: Screen) -> Vec<DocumentationSpec> {
    match screen {
        Screen::Home | Screen::HomeShelf => vec![
            documentation!(
                Navigation,
                "Navigate albums/artists",
                actions::SelectNext,
                actions::SelectPrev,
                actions::SelectUp,
                actions::SelectDown,
                actions::SelectLeft,
                actions::SelectRight,
            ),
            documentation!(Library, "Search albums", actions::ToggleSearch),
            documentation!(Library, "Add album to queue", actions::Enter),
            documentation!(Playback, "Play/Pause", actions::PlayPause),
        ],
        Screen::NowPlaying => vec![
            documentation!(Playback, "Play/Pause", actions::PlayPause),
            documentation!(Playback, "Next track", actions::NextTrack),
            documentation!(Playback, "Previous track", actions::PrevTrack),
            documentation!(
                Navigation,
                "Move through queue",
                actions::SelectNext,
                actions::SelectPrev,
            ),
        ],
        Screen::Library => vec![
            documentation!(
                Navigation,
                "Navigate albums/artists",
                actions::SelectNext,
                actions::SelectPrev,
                actions::SelectUp,
                actions::SelectDown,
            ),
            documentation!(
                Navigation,
                "Jump by page",
                actions::SelectPrevPage,
                actions::SelectNextPage,
            ),
            documentation!(Library, "Search albums", actions::ToggleSearch),
            documentation!(
                Library,
                "Toggle tree view / flat view",
                actions::ToggleLibraryView,
            ),
            documentation!(
                Navigation,
                "Collapse/expand artists in tree view",
                actions::SelectLeft,
                actions::SelectRight,
            ),
            documentation!(
                Library,
                "Sort by Artist/Album/Title/Year",
                actions::CycleSortOrder,
                actions::SetSortArtist,
                actions::SetSortAlbum,
                actions::SetSortTitle,
                actions::SetSortYear,
            ),
            documentation!(
                Library,
                "Filter: All/Mono/Stereo/Multi/Mixed",
                actions::CycleChannelFilter,
                actions::SetFilterAll,
                actions::SetFilterMono,
                actions::SetFilterStereo,
                actions::SetFilterSurround,
                actions::SetFilterSurround71,
                actions::SetFilterSurroundPlus,
            ),
            documentation!(Library, "Add album to queue", actions::Enter),
            documentation!(ScreenSwitch, "Go to queue screen", actions::SwitchToQueue),
        ],
        Screen::Streams => vec![
            documentation!(Playback, "Play stream", actions::Enter),
            documentation!(Playback, "Play/Pause", actions::PlayPause),
        ],
        Screen::Queue => vec![
            documentation!(
                Navigation,
                "Navigate queue items",
                actions::SelectNext,
                actions::SelectPrev,
                actions::SelectUp,
                actions::SelectDown,
            ),
            documentation!(Playback, "Play selected album from start", actions::Enter,),
            documentation!(
                Navigation,
                "Expand/collapse album tracks",
                actions::SelectLeft,
                actions::SelectRight,
                actions::ToggleExpand,
            ),
            documentation!(Playback, "Play/Pause", actions::PlayPause),
            documentation!(Playback, "Next track", actions::NextTrack),
            documentation!(Playback, "Previous track", actions::PrevTrack),
            documentation!(Queue, "Remove from queue", actions::RemoveItem),
        ],
        Screen::Spectrum => vec![
            documentation!(Playback, "Play/Pause", actions::PlayPause),
            documentation!(Playback, "Next track", actions::NextTrack),
        ],
        Screen::Settings => vec![
            documentation!(System, "Cycle theme", actions::CycleTheme),
            documentation!(System, "Cycle language", actions::CycleLanguage),
        ],
        Screen::SettingsDetail => vec![documentation!(System, "Back to settings", actions::Cancel)],
        Screen::StudioHub => vec![
            documentation!(Playback, "Play/Pause", actions::PlayPause),
            documentation!(System, "Cancel/close", actions::Cancel),
        ],
        Screen::EqCurve => vec![documentation!(System, "Back to Studio", actions::Cancel)],
        Screen::Studio => vec![
            documentation!(
                Plugins,
                "Quick add plugins",
                actions::QuickAddEQ,
                actions::QuickAddUpmixer,
                actions::QuickAddCompressor,
                actions::QuickAddGate,
                actions::QuickAddLimiter,
                actions::QuickAddLoudness,
                actions::QuickAddBinaural,
                actions::QuickAddDownmix,
                actions::QuickAddMonoToStereo,
                actions::QuickAddSpectrum,
            ),
            documentation!(
                Navigation,
                "Select next / previous plugin",
                actions::SelectNext,
                actions::SelectPrev,
            ),
            documentation!(Plugins, "Toggle on/off", actions::TogglePlugin),
            documentation!(
                Plugins,
                "Move up/down",
                actions::MovePluginUp,
                actions::MovePluginDown,
            ),
            documentation!(Plugins, "Delete plugin", actions::RemoveItem),
        ],
        Screen::Recording | Screen::RoomEq | Screen::HeadphoneEq | Screen::Spinorama => vec![
            documentation!(
                Navigation,
                "Navigate between steps",
                actions::PreviousWorkflowStep,
            ),
            documentation!(
                Navigation,
                "Proceed to next step or finish",
                actions::NextWorkflowStep,
            ),
        ],
        Screen::PluginGraph => vec![
            documentation!(
                Plugins,
                "Select next / previous node",
                actions::GraphSelectNextNode,
                actions::GraphSelectPreviousNode,
            ),
            documentation!(
                Plugins,
                "Choose the plugin added by A",
                actions::GraphSelectPreviousPluginType,
                actions::GraphSelectNextPluginType,
            ),
            documentation!(
                Plugins,
                "Add selected plugin",
                actions::GraphAddSelectedPlugin
            ),
            documentation!(
                Plugins,
                "Edit selected plugin",
                actions::GraphEditSelectedNode,
            ),
            documentation!(
                Plugins,
                "Toggle selected plugin bypass",
                actions::GraphToggleSelectedBypass,
            ),
            documentation!(
                Plugins,
                "Arm source / connect to selected node",
                actions::GraphConnectSelectedNode,
            ),
            documentation!(
                Plugins,
                "Select connection port",
                actions::GraphSelectPreviousPort,
                actions::GraphSelectNextPort,
            ),
            documentation!(
                Plugins,
                "Disconnect selected node",
                actions::GraphDisconnectSelectedNode,
            ),
            documentation!(
                Plugins,
                "Move selected node",
                actions::GraphMoveSelectedLeft,
                actions::GraphMoveSelectedRight,
                actions::GraphMoveSelectedUp,
                actions::GraphMoveSelectedDown,
                actions::GraphMoveSelectedLeftLarge,
                actions::GraphMoveSelectedRightLarge,
                actions::GraphMoveSelectedUpLarge,
                actions::GraphMoveSelectedDownLarge,
            ),
            documentation!(
                Plugins,
                "Remove selected plugin",
                actions::GraphRemoveSelectedNode,
            ),
        ],
        Screen::ListeningTest => vec![
            documentation!(
                ListeningTests,
                "Capture current chain as path A or B",
                actions::ListeningCapturePathA,
                actions::ListeningCapturePathB,
            ),
            documentation!(
                ListeningTests,
                "Prepare deterministic level and latency matching",
                actions::ListeningPrepare,
            ),
            documentation!(
                ListeningTests,
                "Start blind A/B or ABX trial",
                actions::ListeningStartBlindAb,
                actions::ListeningStartAbx,
            ),
            documentation!(
                ListeningTests,
                "Play available trial cues",
                actions::ListeningPlayCue1,
                actions::ListeningPlayCue2,
                actions::ListeningPlayCue3,
            ),
            documentation!(
                ListeningTests,
                "Commit the first/A or second/B answer",
                actions::ListeningCommitAnswer1,
                actions::ListeningCommitAnswer2,
            ),
            documentation!(
                ListeningTests,
                "Play/Pause synchronized transport",
                actions::PlayPause,
            ),
        ],
        Screen::Playlists => vec![
            documentation!(Playback, "Play/Pause", actions::PlayPause),
            documentation!(System, "Cancel/close", actions::Cancel),
        ],
    }
}

fn binding_key_label(binding: &KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(|keystroke| humanize_key_spec(&keystroke.to_string()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn binding_raw_key_spec(binding: &KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn humanize_key_spec(spec: &str) -> String {
    let mut remaining = spec;
    let mut modifiers = Vec::new();
    while let Some((label, rest)) = strip_modifier(remaining) {
        modifiers.push(label);
        remaining = rest;
    }

    modifiers.push(match remaining {
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "space" => "Space".to_string(),
        "enter" => "Enter".to_string(),
        "escape" => "Esc".to_string(),
        "delete" => "Del".to_string(),
        "backspace" => "Backspace".to_string(),
        "tab" => "Tab".to_string(),
        "pageup" => "PgUp".to_string(),
        "pagedown" => "PgDn".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        key if key.len() == 1 => key.to_uppercase(),
        key if key.starts_with('f') && key[1..].chars().all(|c| c.is_ascii_digit()) => {
            key.to_uppercase()
        }
        key => key.to_string(),
    });
    modifiers.join("+")
}

fn strip_modifier(spec: &str) -> Option<(String, &str)> {
    for (prefix, label) in [
        ("secondary-", secondary_modifier_label()),
        ("ctrl-", "Ctrl"),
        ("cmd-", "Cmd"),
        ("alt-", "Alt"),
        ("shift-", "Shift"),
    ] {
        if let Some(rest) = spec.strip_prefix(prefix) {
            return Some((label.to_string(), rest));
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn secondary_modifier_label() -> &'static str {
    "Cmd"
}

#[cfg(not(target_os = "macos"))]
fn secondary_modifier_label() -> &'static str {
    "Ctrl"
}

fn key_display_rank(key: &str) -> u8 {
    if key.to_ascii_lowercase().contains("media") || key.starts_with('F') {
        4
    } else if key.contains("Ctrl") || key.contains("Cmd") || key.contains("Alt") {
        2
    } else if key.chars().count() == 1 {
        0
    } else {
        1
    }
}
