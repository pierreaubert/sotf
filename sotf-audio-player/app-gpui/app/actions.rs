//! Action definitions for the GPUI audio player.
//!
//! All keyboard actions are defined here and used by both keybindings
//! and the UI layer.

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
        SwitchToDevices,
        SwitchToDirectoryManager,
        SwitchToHeadphoneEQ,
        SwitchToLibrary,
        SwitchToQueue,
        SwitchToPlugins,
        SwitchToStudio,
        SwitchToPluginGraph,
        SwitchToRoomEQ,
        SwitchToSpectrum,
        SwitchToSettings,
        SwitchToRecording,
        SwitchToSpinorma,
        OpenConfig, // Menu bar: open config (cmd-,)
        QuitApp,    // Menu bar: quit app (cmd-q)
        CycleTheme,
        CycleLanguage,
        ToggleSearch,
        ToggleLibraryView,
        ToggleHelp,
        About,
        CycleSortOrder,
        SetSortArtist,
        SetSortAlbum,
        SetSortTitle,
        SetSortYear,
        CycleChannelFilter,
        SetFilterAll,
        SetFilterMono,
        SetFilterStereo,
        SetFilterSurround,
        SetFilterSurround71,
        SetFilterSurroundPlus,
        SetFilterMixed,
        SelectNext,
        SelectPrev,
        SelectNextPage,
        SelectPrevPage,
        NextPage,    // For library pagination
        PrevPage,    // For library pagination
        SelectLeft,  // Grid: move left
        SelectRight, // Grid: move right
        SelectUp,    // Grid: move up
        SelectDown,  // Grid: move down
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
        QuickAddEQ,
        QuickAddUpmixer,
        QuickAddCompressor,
        QuickAddGate,
        QuickAddLimiter,
        QuickAddLoudness,
        QuickAddBinaural,
        // Level meter actions
        SelectNextMeterGroup,
        SelectPrevMeterGroup,
        ToggleMeterMute,
        ToggleMeterSolo,
        ToggleMeterDim,
        ClearMeterMutesSolos,
    ]
);
