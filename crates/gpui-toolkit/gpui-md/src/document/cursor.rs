/// Editor cursor with optional selection.
#[derive(Debug, Clone, Copy)]
pub struct EditorCursor {
    /// Current cursor position (character offset in the rope).
    pub position: usize,
    /// Selection anchor (character offset). None means no selection.
    /// When set, the selection range is [min(anchor, position)..max(anchor, position)).
    pub anchor: Option<usize>,
    /// Preferred column for vertical movement (preserves column when moving through shorter lines).
    pub preferred_column: usize,
}

impl EditorCursor {
    pub fn new() -> Self {
        Self {
            position: 0,
            anchor: None,
            preferred_column: 0,
        }
    }

    pub fn at(position: usize) -> Self {
        Self {
            position,
            anchor: None,
            preferred_column: 0,
        }
    }

    /// Returns the selection range as (start, end) if there is a selection.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.anchor.map(|anchor| {
            let start = anchor.min(self.position);
            let end = anchor.max(self.position);
            (start, end)
        })
    }

    pub fn has_selection(&self) -> bool {
        self.anchor.is_some() && self.anchor != Some(self.position)
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    pub fn start_selection(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.position);
        }
    }

    /// Move cursor, optionally extending selection.
    pub fn move_to(&mut self, position: usize, extend_selection: bool) {
        if extend_selection {
            self.start_selection();
        } else {
            self.clear_selection();
        }
        self.position = position;
    }
}

impl Default for EditorCursor {
    fn default() -> Self {
        Self::new()
    }
}
