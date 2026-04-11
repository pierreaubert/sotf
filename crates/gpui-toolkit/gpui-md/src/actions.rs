use gpui::actions;

actions!(
    gpui_md,
    [
        // File operations
        NewFile,
        OpenFile,
        SaveFile,
        SaveFileAs,
        // Edit operations
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
        // Formatting
        ToggleBold,
        ToggleItalic,
        ToggleCode,
        ToggleStrikethrough,
        InsertHeading1,
        InsertHeading2,
        InsertHeading3,
        InsertLink,
        InsertImage,
        InsertCodeBlock,
        InsertTable,
        InsertHorizontalRule,
        InsertTaskList,
        // View
        TogglePreview,
        FocusEditor,
        FocusPreview,
        ToggleLineNumbers,
        // Keybindings preset
        SetKeymapDefault,
        SetKeymapEmacs,
        SetKeymapVim,
        // Search
        Find,
        FindReplace,
        // Import/Export
        ImportDocx,
        ExportDocx,
        ExportPdf,
        // Navigation
        PageUp,
        PageDown,
        WordLeft,
        WordRight,
        // Font size
        IncreaseFontSize,
        DecreaseFontSize,
        // Emacs commands
        Abort,
        UpcaseWord,
        DowncaseWord,
        IsearchForward,
        IsearchBackward,
        CommandPalette,
        ExchangePointAndMark,
        Recenter,
        UniversalArgument,
    ]
);
