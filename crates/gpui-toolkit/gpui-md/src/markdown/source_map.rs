use std::collections::HashMap;

/// A span in the source markdown text (1-indexed lines from comrak).
#[derive(Debug, Clone, Copy)]
pub struct SourceSpan {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl SourceSpan {
    pub fn from_comrak(pos: comrak::nodes::Sourcepos) -> Self {
        Self {
            start_line: pos.start.line,
            start_col: pos.start.column,
            end_line: pos.end.line,
            end_col: pos.end.column,
        }
    }
}

/// Maps rendered element IDs back to source positions.
#[derive(Clone)]
pub struct SourceMap {
    spans: HashMap<String, SourceSpan>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            spans: HashMap::new(),
        }
    }

    pub fn insert(&mut self, element_id: String, span: SourceSpan) {
        self.spans.insert(element_id, span);
    }

    pub fn get(&self, element_id: &str) -> Option<&SourceSpan> {
        self.spans.get(element_id)
    }

    /// Find the source span whose start_line is closest to (but not after) the given line.
    pub fn find_block_at_line(&self, line: usize) -> Option<&SourceSpan> {
        self.spans
            .values()
            .filter(|s| s.start_line <= line)
            .max_by_key(|s| s.start_line)
    }

    /// Get all block IDs sorted by their start line.
    pub fn blocks_by_line(&self) -> Vec<(&String, &SourceSpan)> {
        let mut entries: Vec<_> = self.spans.iter().collect();
        entries.sort_by_key(|(_, s)| s.start_line);
        entries
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    pub fn len(&self) -> usize {
        self.spans.len()
    }
}

impl Default for SourceMap {
    fn default() -> Self {
        Self::new()
    }
}
