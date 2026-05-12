/// Text segmentation and normalization, ported from chenglou/pretext.
///
/// Handles whitespace normalization, word boundary segmentation via
/// `unicode-segmentation`, segment classification by break kind,
/// URL/numeric/punctuation merging, and CJK/Arabic/Myanmar special rules.
use unicode_segmentation::UnicodeSegmentation;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpaceMode {
    Normal,
    PreWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentBreakKind {
    Text,
    Space,
    PreservedSpace,
    Tab,
    Glue,
    ZeroWidthBreak,
    SoftHyphen,
    HardBreak,
}

#[derive(Debug, Clone)]
pub struct AnalysisChunk {
    pub start_segment_index: usize,
    pub end_segment_index: usize,
    pub consumed_end_segment_index: usize,
}

#[derive(Debug, Clone)]
pub struct MergedSegmentation {
    pub texts: Vec<String>,
    pub is_word_like: Vec<bool>,
    pub kinds: Vec<SegmentBreakKind>,
    pub starts: Vec<usize>,
}

impl MergedSegmentation {
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct TextAnalysis {
    pub normalized: String,
    pub chunks: Vec<AnalysisChunk>,
    pub texts: Vec<String>,
    pub is_word_like: Vec<bool>,
    pub kinds: Vec<SegmentBreakKind>,
    pub starts: Vec<usize>,
}

impl TextAnalysis {
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AnalysisProfile {
    pub carry_cjk_after_closing_quote: bool,
}

// ---------------------------------------------------------------------------
// Whitespace profiles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct WhiteSpaceProfile {
    mode: WhiteSpaceMode,
    preserve_ordinary_spaces: bool,
    preserve_hard_breaks: bool,
}

fn get_white_space_profile(ws: WhiteSpaceMode) -> WhiteSpaceProfile {
    match ws {
        WhiteSpaceMode::PreWrap => WhiteSpaceProfile {
            mode: ws,
            preserve_ordinary_spaces: true,
            preserve_hard_breaks: true,
        },
        WhiteSpaceMode::Normal => WhiteSpaceProfile {
            mode: ws,
            preserve_ordinary_spaces: false,
            preserve_hard_breaks: false,
        },
    }
}

// ---------------------------------------------------------------------------
// Whitespace normalization
// ---------------------------------------------------------------------------

pub fn normalize_whitespace_normal(text: &str) -> String {
    // Collapse runs of [ \t\n\r\f]+ to a single space, strip leading/trailing spaces
    let mut result = String::with_capacity(text.len());
    let mut in_ws = false;
    for ch in text.chars() {
        if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' || ch == '\x0C' {
            in_ws = true;
        } else {
            if in_ws && !result.is_empty() {
                result.push(' ');
            }
            in_ws = false;
            result.push(ch);
        }
    }
    result
}

pub fn normalize_whitespace_pre_wrap(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            // Consume all consecutive \r and at most one following \n
            // so that \r\r\n produces a single \n instead of double.
            while chars.peek() == Some(&'\r') {
                chars.next();
            }
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            result.push('\n');
        } else if ch == '\x0C' {
            result.push('\n');
        } else {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Unicode classification helpers
// ---------------------------------------------------------------------------

pub fn is_cjk(s: &str) -> bool {
    for ch in s.chars() {
        let c = ch as u32;
        if (0x4E00..=0x9FFF).contains(&c)
            || (0x3400..=0x4DBF).contains(&c)
            || (0x20000..=0x2A6DF).contains(&c)
            || (0x2A700..=0x2B73F).contains(&c)
            || (0x2B740..=0x2B81F).contains(&c)
            || (0x2B820..=0x2CEAF).contains(&c)
            || (0x2CEB0..=0x2EBEF).contains(&c)
            || (0x30000..=0x3134F).contains(&c)
            || (0xF900..=0xFAFF).contains(&c)
            || (0x2F800..=0x2FA1F).contains(&c)
            || (0x3000..=0x303F).contains(&c)
            || (0x3040..=0x309F).contains(&c)
            || (0x30A0..=0x30FF).contains(&c)
            || (0xAC00..=0xD7AF).contains(&c)
            || (0xFF00..=0xFFEF).contains(&c)
        {
            return true;
        }
    }
    false
}

fn is_combining_mark(ch: char) -> bool {
    let c = ch as u32;
    (0x0300..=0x036F).contains(&c) // Combining Diacritical Marks
        || (0x1AB0..=0x1AFF).contains(&c) // Combining Diacritical Marks Extended
        || (0x1DC0..=0x1DFF).contains(&c) // Combining Diacritical Marks Supplement
        || (0x20D0..=0x20FF).contains(&c) // Combining Diacritical Marks for Symbols
        || (0xFE20..=0xFE2F).contains(&c) // Combining Half Marks
        || (0x0483..=0x0489).contains(&c) // Cyrillic
        || (0x0591..=0x05BD).contains(&c) // Hebrew
        || (0x05BF..=0x05BF).contains(&c)
        || (0x05C1..=0x05C2).contains(&c)
        || (0x05C4..=0x05C5).contains(&c)
        || (0x05C7..=0x05C7).contains(&c)
        || (0x0610..=0x061A).contains(&c) // Arabic
        || (0x064B..=0x065F).contains(&c)
        || (0x0670..=0x0670).contains(&c)
        || (0x06D6..=0x06DC).contains(&c)
        || (0x06DF..=0x06E4).contains(&c)
        || (0x06E7..=0x06E8).contains(&c)
        || (0x06EA..=0x06ED).contains(&c)
        || (0x0900..=0x0903).contains(&c) // Devanagari
        || (0x093A..=0x094F).contains(&c)
        || (0x0951..=0x0957).contains(&c)
        || (0x0962..=0x0963).contains(&c)
        || (0x0981..=0x0983).contains(&c) // Bengali
        || (0x09BC..=0x09BC).contains(&c)
        || (0x09BE..=0x09C4).contains(&c)
        || (0x0A01..=0x0A03).contains(&c) // Gurmukhi
        || (0x0A3C..=0x0A3C).contains(&c)
        || (0x0A3E..=0x0A42).contains(&c)
        || (0x0A47..=0x0A48).contains(&c)
        || (0x0A4B..=0x0A4D).contains(&c)
        || (0x0A70..=0x0A71).contains(&c)
}

fn is_decimal_digit(ch: char) -> bool {
    // \p{Nd} — all Unicode decimal digits
    ch.is_ascii_digit()
        || (0x0660..=0x0669).contains(&(ch as u32)) // Arabic-Indic
        || (0x06F0..=0x06F9).contains(&(ch as u32)) // Extended Arabic-Indic
        || (0x0966..=0x096F).contains(&(ch as u32)) // Devanagari
        || (0x09E6..=0x09EF).contains(&(ch as u32)) // Bengali
        || (0x0A66..=0x0A6F).contains(&(ch as u32)) // Gurmukhi
        || (0x0AE6..=0x0AEF).contains(&(ch as u32)) // Gujarati
        || (0x0B66..=0x0B6F).contains(&(ch as u32)) // Oriya
        || (0x0BE6..=0x0BEF).contains(&(ch as u32)) // Tamil
        || (0x0C66..=0x0C6F).contains(&(ch as u32)) // Telugu
        || (0x0CE6..=0x0CEF).contains(&(ch as u32)) // Kannada
        || (0x0D66..=0x0D6F).contains(&(ch as u32)) // Malayalam
        || (0x0E50..=0x0E59).contains(&(ch as u32)) // Thai
        || (0x0ED0..=0x0ED9).contains(&(ch as u32)) // Lao
        || (0x0F20..=0x0F29).contains(&(ch as u32)) // Tibetan
        || (0x1040..=0x1049).contains(&(ch as u32)) // Myanmar
        || (0xFF10..=0xFF19).contains(&(ch as u32)) // Fullwidth
}

fn contains_arabic_script(text: &str) -> bool {
    text.chars().any(|ch| {
        (0x0600..=0x06FF).contains(&(ch as u32)) || (0x0750..=0x077F).contains(&(ch as u32))
    })
}

// ---------------------------------------------------------------------------
// Character sets (kinsoku, punctuation, etc.)
// ---------------------------------------------------------------------------

pub fn is_kinsoku_start(ch: char) -> bool {
    matches!(
        ch,
        '\u{FF0C}'
            | '\u{FF0E}'
            | '\u{FF01}'
            | '\u{FF1A}'
            | '\u{FF1B}'
            | '\u{FF1F}'
            | '\u{3001}'
            | '\u{3002}'
            | '\u{30FB}'
            | '\u{FF09}'
            | '\u{3015}'
            | '\u{3009}'
            | '\u{300B}'
            | '\u{300D}'
            | '\u{300F}'
            | '\u{3011}'
            | '\u{3017}'
            | '\u{3019}'
            | '\u{301B}'
            | '\u{30FC}'
            | '\u{3005}'
            | '\u{303B}'
            | '\u{309D}'
            | '\u{309E}'
            | '\u{30FD}'
            | '\u{30FE}'
    )
}

pub fn is_kinsoku_end(ch: char) -> bool {
    matches!(
        ch,
        '"' | '('
            | '['
            | '{'
            | '\u{201C}'
            | '\u{2018}'
            | '\u{00AB}'
            | '\u{2039}'
            | '\u{FF08}'
            | '\u{3014}'
            | '\u{3008}'
            | '\u{300A}'
            | '\u{300C}'
            | '\u{300E}'
            | '\u{3010}'
            | '\u{3016}'
            | '\u{3018}'
            | '\u{301A}'
    )
}

fn is_forward_sticky_glue(ch: char) -> bool {
    matches!(ch, '\'' | '\u{2019}')
}

pub fn is_left_sticky_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | '!'
            | '?'
            | ':'
            | ';'
            | '\u{060C}'
            | '\u{061B}'
            | '\u{061F}'
            | '\u{0964}'
            | '\u{0965}'
            | '\u{104A}'
            | '\u{104B}'
            | '\u{104C}'
            | '\u{104D}'
            | '\u{104F}'
            | ')'
            | ']'
            | '}'
            | '%'
            | '"'
            | '\u{201D}'
            | '\u{2019}'
            | '\u{00BB}'
            | '\u{203A}'
            | '\u{2026}'
    )
}

fn is_arabic_no_space_trailing_punctuation(ch: char) -> bool {
    matches!(ch, ':' | '.' | '\u{060C}' | '\u{061B}')
}

fn is_myanmar_medial_glue(ch: char) -> bool {
    ch == '\u{104F}'
}

fn is_closing_quote(ch: char) -> bool {
    matches!(
        ch,
        '\u{201D}'
            | '\u{2019}'
            | '\u{00BB}'
            | '\u{203A}'
            | '\u{300D}'
            | '\u{300F}'
            | '\u{3011}'
            | '\u{300B}'
            | '\u{3009}'
            | '\u{3015}'
            | '\u{FF09}'
    )
}

pub fn ends_with_closing_quote(text: &str) -> bool {
    for ch in text.chars().rev() {
        if is_closing_quote(ch) {
            return true;
        }
        if !is_left_sticky_punctuation(ch) {
            return false;
        }
    }
    false
}

fn is_left_sticky_punctuation_segment(segment: &str) -> bool {
    if is_escaped_quote_cluster_segment(segment) {
        return true;
    }
    let mut saw_punctuation = false;
    for ch in segment.chars() {
        if is_left_sticky_punctuation(ch) {
            saw_punctuation = true;
            continue;
        }
        if saw_punctuation && is_combining_mark(ch) {
            continue;
        }
        return false;
    }
    saw_punctuation
}

fn is_cjk_line_start_prohibited_segment(segment: &str) -> bool {
    if segment.is_empty() {
        return false;
    }
    segment
        .chars()
        .all(|ch| is_kinsoku_start(ch) || is_left_sticky_punctuation(ch))
}

fn is_forward_sticky_cluster_segment(segment: &str) -> bool {
    if is_escaped_quote_cluster_segment(segment) {
        return true;
    }
    if segment.is_empty() {
        return false;
    }
    segment
        .chars()
        .all(|ch| is_kinsoku_end(ch) || is_forward_sticky_glue(ch) || is_combining_mark(ch))
}

fn is_escaped_quote_cluster_segment(segment: &str) -> bool {
    let mut saw_quote = false;
    for ch in segment.chars() {
        if ch == '\\' || is_combining_mark(ch) {
            continue;
        }
        if is_kinsoku_end(ch) || is_left_sticky_punctuation(ch) || is_forward_sticky_glue(ch) {
            saw_quote = true;
            continue;
        }
        return false;
    }
    saw_quote
}

fn split_trailing_forward_sticky_cluster(text: &str) -> Option<(&str, &str)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut split_index = chars.len();

    while split_index > 0 {
        let ch = chars[split_index - 1].1;
        if is_combining_mark(ch) || is_kinsoku_end(ch) || is_forward_sticky_glue(ch) {
            split_index -= 1;
            continue;
        }
        break;
    }

    if split_index == 0 || split_index == chars.len() {
        return None;
    }

    let byte_pos = chars[split_index].0;
    Some((&text[..byte_pos], &text[byte_pos..]))
}

fn is_repeated_single_char_run(segment: &str, ch: char) -> bool {
    if segment.is_empty() {
        return false;
    }
    segment.chars().all(|c| c == ch)
}

fn ends_with_arabic_no_space_punctuation(segment: &str) -> bool {
    if !contains_arabic_script(segment) || segment.is_empty() {
        return false;
    }
    segment
        .chars()
        .next_back()
        .map(is_arabic_no_space_trailing_punctuation)
        .unwrap_or(false)
}

fn ends_with_myanmar_medial_glue(segment: &str) -> bool {
    segment
        .chars()
        .next_back()
        .map(is_myanmar_medial_glue)
        .unwrap_or(false)
}

fn split_leading_space_and_marks(segment: &str) -> Option<(&str, &str)> {
    if segment.len() < 2 || !segment.starts_with(' ') {
        return None;
    }
    let rest = &segment[1..];
    if rest.chars().all(is_combining_mark) {
        Some((&segment[..1], rest))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Segment break classification
// ---------------------------------------------------------------------------

struct SegmentationPiece {
    text: String,
    is_word_like: bool,
    kind: SegmentBreakKind,
    start: usize,
}

fn classify_segment_break_char(ch: char, profile: &WhiteSpaceProfile) -> SegmentBreakKind {
    if profile.preserve_ordinary_spaces || profile.preserve_hard_breaks {
        if ch == ' ' {
            return SegmentBreakKind::PreservedSpace;
        }
        if ch == '\t' {
            return SegmentBreakKind::Tab;
        }
        if profile.preserve_hard_breaks && ch == '\n' {
            return SegmentBreakKind::HardBreak;
        }
    }
    if ch == ' ' {
        return SegmentBreakKind::Space;
    }
    if ch == '\u{00A0}' || ch == '\u{202F}' || ch == '\u{2060}' || ch == '\u{FEFF}' {
        return SegmentBreakKind::Glue;
    }
    if ch == '\u{200B}' {
        return SegmentBreakKind::ZeroWidthBreak;
    }
    if ch == '\u{00AD}' {
        return SegmentBreakKind::SoftHyphen;
    }
    SegmentBreakKind::Text
}

fn split_segment_by_break_kind(
    segment: &str,
    is_word_like: bool,
    start: usize,
    ws_profile: &WhiteSpaceProfile,
) -> Vec<SegmentationPiece> {
    let mut pieces = Vec::new();
    let mut current_kind: Option<SegmentBreakKind> = None;
    let mut current_text = String::new();
    let mut current_start = start;
    let mut current_word_like = false;
    let mut offset = 0;

    for ch in segment.chars() {
        let kind = classify_segment_break_char(ch, ws_profile);
        let word_like = kind == SegmentBreakKind::Text && is_word_like;

        if let Some(ck) = current_kind {
            if kind == ck && word_like == current_word_like {
                current_text.push(ch);
                offset += ch.len_utf8();
                continue;
            }
            pieces.push(SegmentationPiece {
                text: std::mem::take(&mut current_text),
                is_word_like: current_word_like,
                kind: ck,
                start: current_start,
            });
        }

        current_kind = Some(kind);
        current_text.push(ch);
        current_start = start + offset;
        current_word_like = word_like;
        offset += ch.len_utf8();
    }

    if let Some(ck) = current_kind {
        pieces.push(SegmentationPiece {
            text: current_text,
            is_word_like: current_word_like,
            kind: ck,
            start: current_start,
        });
    }

    pieces
}

// ---------------------------------------------------------------------------
// Word-likeness heuristic
// ---------------------------------------------------------------------------

/// Determine if a word-boundary segment is "word-like" (contains letters/numbers).
/// Equivalent to `Intl.Segmenter`'s `isWordLike`.
fn segment_is_word_like(segment: &str) -> bool {
    segment
        .chars()
        .any(|ch| ch.is_alphanumeric() || is_cjk_char(ch))
}

fn is_cjk_char(ch: char) -> bool {
    let c = ch as u32;
    (0x4E00..=0x9FFF).contains(&c)
        || (0x3400..=0x4DBF).contains(&c)
        || (0x20000..=0x2A6DF).contains(&c)
        || (0x3040..=0x309F).contains(&c)
        || (0x30A0..=0x30FF).contains(&c)
        || (0xAC00..=0xD7AF).contains(&c)
}

// ---------------------------------------------------------------------------
// Text run boundary check
// ---------------------------------------------------------------------------

fn is_text_run_boundary(kind: SegmentBreakKind) -> bool {
    matches!(
        kind,
        SegmentBreakKind::Space
            | SegmentBreakKind::PreservedSpace
            | SegmentBreakKind::ZeroWidthBreak
            | SegmentBreakKind::HardBreak
    )
}

// ---------------------------------------------------------------------------
// URL detection
// ---------------------------------------------------------------------------

fn is_url_scheme_segment(text: &str) -> bool {
    if text.is_empty() || !text.ends_with(':') {
        return false;
    }
    let prefix = &text[..text.len() - 1];
    if prefix.is_empty() {
        return false;
    }
    let mut chars = prefix.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
}

#[allow(dead_code)]
fn is_url_like_run_start(seg: &MergedSegmentation, index: usize) -> bool {
    let text = &seg.texts[index];
    if text.starts_with("www.") {
        return true;
    }
    is_url_scheme_segment(text)
        && index + 1 < seg.len()
        && seg.kinds[index + 1] == SegmentBreakKind::Text
        && seg.texts[index + 1] == "//"
}

fn is_url_query_boundary_segment(text: &str) -> bool {
    text.contains('?') && (text.contains("://") || text.starts_with("www."))
}

fn merge_url_like_runs(seg: MergedSegmentation) -> MergedSegmentation {
    let len = seg.len();
    let mut texts = seg.texts;
    let mut is_word_like = seg.is_word_like;
    let mut kinds = seg.kinds;
    let starts = seg.starts;

    let orig_texts = texts.clone();
    let orig_kinds = kinds.clone();

    for i in 0..len {
        if kinds[i] != SegmentBreakKind::Text
            || !is_url_like_run_start_raw(&orig_texts, &orig_kinds, i, len)
        {
            continue;
        }
        let mut j = i + 1;
        while j < len && !is_text_run_boundary(orig_kinds[j]) {
            texts[i].push_str(&orig_texts[j]);
            is_word_like[i] = true;
            let ends_query = orig_texts[j].contains('?');
            kinds[j] = SegmentBreakKind::Text;
            texts[j].clear();
            j += 1;
            if ends_query {
                break;
            }
        }
    }

    compact(texts, is_word_like, kinds, starts)
}

fn is_url_like_run_start_raw(
    texts: &[String],
    kinds: &[SegmentBreakKind],
    index: usize,
    len: usize,
) -> bool {
    let text = &texts[index];
    if text.starts_with("www.") {
        return true;
    }
    is_url_scheme_segment(text)
        && index + 1 < len
        && kinds[index + 1] == SegmentBreakKind::Text
        && texts[index + 1] == "//"
}

fn merge_url_query_runs(seg: MergedSegmentation) -> MergedSegmentation {
    let mut texts = Vec::new();
    let mut is_word_like = Vec::new();
    let mut kinds = Vec::new();
    let mut starts = Vec::new();

    let mut i = 0;
    while i < seg.len() {
        let text = &seg.texts[i];
        texts.push(text.clone());
        is_word_like.push(seg.is_word_like[i]);
        kinds.push(seg.kinds[i]);
        starts.push(seg.starts[i]);

        if !is_url_query_boundary_segment(text) {
            i += 1;
            continue;
        }

        let next = i + 1;
        if next >= seg.len() || is_text_run_boundary(seg.kinds[next]) {
            i += 1;
            continue;
        }

        let mut query_text = String::new();
        let query_start = seg.starts[next];
        let mut j = next;
        while j < seg.len() && !is_text_run_boundary(seg.kinds[j]) {
            query_text.push_str(&seg.texts[j]);
            j += 1;
        }

        if !query_text.is_empty() {
            texts.push(query_text);
            is_word_like.push(true);
            kinds.push(SegmentBreakKind::Text);
            starts.push(query_start);
            i = j;
        } else {
            i += 1;
        }
    }

    MergedSegmentation {
        texts,
        is_word_like,
        kinds,
        starts,
    }
}

// ---------------------------------------------------------------------------
// Numeric merging
// ---------------------------------------------------------------------------

fn is_numeric_joiner(ch: char) -> bool {
    matches!(
        ch,
        ':' | '-' | '/' | '\u{00D7}' | ',' | '.' | '+' | '\u{2013}' | '\u{2014}'
    )
}

fn segment_contains_decimal_digit(text: &str) -> bool {
    text.chars().any(is_decimal_digit)
}

fn is_numeric_run_segment(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| is_decimal_digit(ch) || is_numeric_joiner(ch))
}

fn merge_numeric_runs(seg: MergedSegmentation) -> MergedSegmentation {
    let mut texts = Vec::new();
    let mut is_word_like = Vec::new();
    let mut kinds = Vec::new();
    let mut starts = Vec::new();

    let mut i = 0;
    while i < seg.len() {
        let text = &seg.texts[i];
        let kind = seg.kinds[i];

        if kind == SegmentBreakKind::Text
            && is_numeric_run_segment(text)
            && segment_contains_decimal_digit(text)
        {
            let mut merged = text.clone();
            let mut j = i + 1;
            while j < seg.len()
                && seg.kinds[j] == SegmentBreakKind::Text
                && is_numeric_run_segment(&seg.texts[j])
            {
                merged.push_str(&seg.texts[j]);
                j += 1;
            }
            texts.push(merged);
            is_word_like.push(true);
            kinds.push(SegmentBreakKind::Text);
            starts.push(seg.starts[i]);
            i = j;
            continue;
        }

        texts.push(text.clone());
        is_word_like.push(seg.is_word_like[i]);
        kinds.push(kind);
        starts.push(seg.starts[i]);
        i += 1;
    }

    MergedSegmentation {
        texts,
        is_word_like,
        kinds,
        starts,
    }
}

// ---------------------------------------------------------------------------
// ASCII punctuation chains
// ---------------------------------------------------------------------------

fn is_ascii_punctuation_chain_segment(text: &str) -> bool {
    // /^[A-Za-z0-9_]+[,:;]*$/
    if text.is_empty() {
        return false;
    }
    let mut saw_alphanum = false;
    let mut in_trailing = false;
    for ch in text.chars() {
        if !in_trailing && (ch.is_ascii_alphanumeric() || ch == '_') {
            saw_alphanum = true;
            continue;
        }
        if saw_alphanum && matches!(ch, ',' | ':' | ';') {
            in_trailing = true;
            continue;
        }
        if in_trailing && matches!(ch, ',' | ':' | ';') {
            continue;
        }
        return false;
    }
    saw_alphanum
}

fn has_trailing_punctuation_joiners(text: &str) -> bool {
    text.chars()
        .next_back()
        .map(|ch| matches!(ch, ',' | ':' | ';'))
        .unwrap_or(false)
}

fn merge_ascii_punctuation_chains(seg: MergedSegmentation) -> MergedSegmentation {
    let mut texts = Vec::new();
    let mut is_word_like = Vec::new();
    let mut kinds = Vec::new();
    let mut starts = Vec::new();

    let mut i = 0;
    while i < seg.len() {
        let text = &seg.texts[i];
        let kind = seg.kinds[i];
        let wl = seg.is_word_like[i];

        if kind == SegmentBreakKind::Text && wl && is_ascii_punctuation_chain_segment(text) {
            let mut merged = text.clone();
            let mut j = i + 1;
            while has_trailing_punctuation_joiners(&merged)
                && j < seg.len()
                && seg.kinds[j] == SegmentBreakKind::Text
                && seg.is_word_like[j]
                && is_ascii_punctuation_chain_segment(&seg.texts[j])
            {
                merged.push_str(&seg.texts[j]);
                j += 1;
            }
            texts.push(merged);
            is_word_like.push(true);
            kinds.push(SegmentBreakKind::Text);
            starts.push(seg.starts[i]);
            i = j;
            continue;
        }

        texts.push(text.clone());
        is_word_like.push(wl);
        kinds.push(kind);
        starts.push(seg.starts[i]);
        i += 1;
    }

    MergedSegmentation {
        texts,
        is_word_like,
        kinds,
        starts,
    }
}

// ---------------------------------------------------------------------------
// Hyphenated numeric splitting
// ---------------------------------------------------------------------------

fn split_hyphenated_numeric_runs(seg: MergedSegmentation) -> MergedSegmentation {
    let mut texts = Vec::new();
    let mut is_word_like = Vec::new();
    let mut kinds = Vec::new();
    let mut starts = Vec::new();

    for i in 0..seg.len() {
        let text = &seg.texts[i];
        if seg.kinds[i] == SegmentBreakKind::Text && text.contains('-') {
            let parts: Vec<&str> = text.split('-').collect();
            let should_split = parts.len() > 1
                && parts.iter().all(|part| {
                    !part.is_empty()
                        && segment_contains_decimal_digit(part)
                        && is_numeric_run_segment(part)
                });

            if should_split {
                let mut offset = 0;
                for (j, part) in parts.iter().enumerate() {
                    let split_text = if j < parts.len() - 1 {
                        format!("{part}-")
                    } else {
                        (*part).to_string()
                    };
                    texts.push(split_text.clone());
                    is_word_like.push(true);
                    kinds.push(SegmentBreakKind::Text);
                    starts.push(seg.starts[i] + offset);
                    offset += split_text.len();
                }
                continue;
            }
        }

        texts.push(text.clone());
        is_word_like.push(seg.is_word_like[i]);
        kinds.push(seg.kinds[i]);
        starts.push(seg.starts[i]);
    }

    MergedSegmentation {
        texts,
        is_word_like,
        kinds,
        starts,
    }
}

// ---------------------------------------------------------------------------
// Glue-connected text runs
// ---------------------------------------------------------------------------

fn merge_glue_connected_text_runs(seg: MergedSegmentation) -> MergedSegmentation {
    let mut texts = Vec::new();
    let mut is_word_like = Vec::new();
    let mut kinds = Vec::new();
    let mut starts = Vec::new();

    let mut read = 0;
    while read < seg.len() {
        let mut text = seg.texts[read].clone();
        let mut wl = seg.is_word_like[read];
        let mut kind = seg.kinds[read];
        let start = seg.starts[read];

        if kind == SegmentBreakKind::Glue {
            let mut glue_text = text;
            let glue_start = start;
            read += 1;
            while read < seg.len() && seg.kinds[read] == SegmentBreakKind::Glue {
                glue_text.push_str(&seg.texts[read]);
                read += 1;
            }

            if read < seg.len() && seg.kinds[read] == SegmentBreakKind::Text {
                text = glue_text;
                text.push_str(&seg.texts[read]);
                wl = seg.is_word_like[read];
                kind = SegmentBreakKind::Text;
                // start is glue_start
                read += 1;

                // Continue absorbing glue+text
                while read < seg.len() && seg.kinds[read] == SegmentBreakKind::Glue {
                    let mut gt = String::new();
                    while read < seg.len() && seg.kinds[read] == SegmentBreakKind::Glue {
                        gt.push_str(&seg.texts[read]);
                        read += 1;
                    }
                    if read < seg.len() && seg.kinds[read] == SegmentBreakKind::Text {
                        text.push_str(&gt);
                        text.push_str(&seg.texts[read]);
                        wl = wl || seg.is_word_like[read];
                        read += 1;
                        continue;
                    }
                    text.push_str(&gt);
                    break;
                }

                texts.push(text);
                is_word_like.push(wl);
                kinds.push(kind);
                starts.push(glue_start);
                continue;
            }

            texts.push(glue_text);
            is_word_like.push(false);
            kinds.push(SegmentBreakKind::Glue);
            starts.push(glue_start);
            continue;
        }

        read += 1;

        texts.push(text);
        is_word_like.push(wl);
        kinds.push(kind);
        starts.push(start);
    }

    MergedSegmentation {
        texts,
        is_word_like,
        kinds,
        starts,
    }
}

// ---------------------------------------------------------------------------
// CJK forward-sticky carry
// ---------------------------------------------------------------------------

fn carry_trailing_forward_sticky_across_cjk_boundary(
    seg: MergedSegmentation,
) -> MergedSegmentation {
    let mut texts = seg.texts;
    let is_word_like = seg.is_word_like;
    let kinds = seg.kinds;
    let mut starts = seg.starts;
    let len = texts.len();

    for i in 0..len.saturating_sub(1) {
        if kinds[i] != SegmentBreakKind::Text || kinds[i + 1] != SegmentBreakKind::Text {
            continue;
        }
        if !is_cjk(&texts[i]) || !is_cjk(&texts[i + 1]) {
            continue;
        }
        if let Some((head, tail)) = split_trailing_forward_sticky_cluster(&texts[i]) {
            let head = head.to_string();
            let tail = tail.to_string();
            let new_next = format!("{}{}", tail, &texts[i + 1]);
            starts[i + 1] = starts[i] + head.len();
            texts[i] = head;
            texts[i + 1] = new_next;
        }
    }

    MergedSegmentation {
        texts,
        is_word_like,
        kinds,
        starts,
    }
}

// ---------------------------------------------------------------------------
// Compact helper
// ---------------------------------------------------------------------------

fn compact(
    texts: Vec<String>,
    is_word_like: Vec<bool>,
    kinds: Vec<SegmentBreakKind>,
    starts: Vec<usize>,
) -> MergedSegmentation {
    let mut out_texts = Vec::new();
    let mut out_wl = Vec::new();
    let mut out_kinds = Vec::new();
    let mut out_starts = Vec::new();

    for i in 0..texts.len() {
        if texts[i].is_empty() {
            continue;
        }
        out_texts.push(texts[i].clone());
        out_wl.push(is_word_like[i]);
        out_kinds.push(kinds[i]);
        out_starts.push(starts[i]);
    }

    MergedSegmentation {
        texts: out_texts,
        is_word_like: out_wl,
        kinds: out_kinds,
        starts: out_starts,
    }
}

// ---------------------------------------------------------------------------
// Build merged segmentation
// ---------------------------------------------------------------------------

fn build_merged_segmentation(
    normalized: &str,
    profile: &AnalysisProfile,
    ws_profile: &WhiteSpaceProfile,
) -> MergedSegmentation {
    let mut merged_texts: Vec<String> = Vec::new();
    let mut merged_word_like: Vec<bool> = Vec::new();
    let mut merged_kinds: Vec<SegmentBreakKind> = Vec::new();
    let mut merged_starts: Vec<usize> = Vec::new();

    // Use unicode-segmentation for word boundary detection
    for (seg_byte_start, segment) in normalized.split_word_bound_indices() {
        let is_wl = segment_is_word_like(segment);
        for piece in split_segment_by_break_kind(segment, is_wl, seg_byte_start, ws_profile) {
            let is_text = piece.kind == SegmentBreakKind::Text;
            let merged_len = merged_texts.len();

            // CJK closing quote carry (engine-specific)
            if profile.carry_cjk_after_closing_quote
                && is_text
                && merged_len > 0
                && merged_kinds[merged_len - 1] == SegmentBreakKind::Text
                && is_cjk(&piece.text)
                && is_cjk(&merged_texts[merged_len - 1])
                && ends_with_closing_quote(&merged_texts[merged_len - 1])
            {
                merged_texts[merged_len - 1].push_str(&piece.text);
                merged_word_like[merged_len - 1] =
                    merged_word_like[merged_len - 1] || piece.is_word_like;
                continue;
            }

            // Kinsoku start prohibition
            if is_text
                && merged_len > 0
                && merged_kinds[merged_len - 1] == SegmentBreakKind::Text
                && is_cjk_line_start_prohibited_segment(&piece.text)
                && is_cjk(&merged_texts[merged_len - 1])
            {
                merged_texts[merged_len - 1].push_str(&piece.text);
                merged_word_like[merged_len - 1] =
                    merged_word_like[merged_len - 1] || piece.is_word_like;
                continue;
            }

            // Myanmar medial glue
            if is_text
                && merged_len > 0
                && merged_kinds[merged_len - 1] == SegmentBreakKind::Text
                && ends_with_myanmar_medial_glue(&merged_texts[merged_len - 1])
            {
                merged_texts[merged_len - 1].push_str(&piece.text);
                merged_word_like[merged_len - 1] =
                    merged_word_like[merged_len - 1] || piece.is_word_like;
                continue;
            }

            // Arabic no-space punctuation
            if is_text
                && piece.is_word_like
                && merged_len > 0
                && merged_kinds[merged_len - 1] == SegmentBreakKind::Text
                && contains_arabic_script(&piece.text)
                && ends_with_arabic_no_space_punctuation(&merged_texts[merged_len - 1])
            {
                merged_texts[merged_len - 1].push_str(&piece.text);
                merged_word_like[merged_len - 1] = true;
                continue;
            }

            // Repeated single char
            if is_text
                && !piece.is_word_like
                && merged_len > 0
                && merged_kinds[merged_len - 1] == SegmentBreakKind::Text
                && piece.text.chars().count() == 1
            {
                let ch = piece.text.chars().next().unwrap();
                if ch != '-'
                    && ch != '\u{2014}'
                    && is_repeated_single_char_run(&merged_texts[merged_len - 1], ch)
                {
                    merged_texts[merged_len - 1].push_str(&piece.text);
                    continue;
                }
            }

            // Left-sticky punctuation or trailing hyphen
            if is_text
                && !piece.is_word_like
                && merged_len > 0
                && merged_kinds[merged_len - 1] == SegmentBreakKind::Text
                && (is_left_sticky_punctuation_segment(&piece.text)
                    || (piece.text == "-" && merged_word_like[merged_len - 1]))
            {
                merged_texts[merged_len - 1].push_str(&piece.text);
                continue;
            }

            // Default: new segment
            merged_texts.push(piece.text);
            merged_word_like.push(piece.is_word_like);
            merged_kinds.push(piece.kind);
            merged_starts.push(piece.start);
        }
    }

    let merged_len = merged_texts.len();

    // Escaped quote cluster merge (forward pass)
    for i in 1..merged_len {
        if merged_kinds[i] == SegmentBreakKind::Text
            && !merged_word_like[i]
            && is_escaped_quote_cluster_segment(&merged_texts[i])
            && merged_kinds[i - 1] == SegmentBreakKind::Text
        {
            let text = merged_texts[i].clone();
            merged_texts[i - 1].push_str(&text);
            merged_word_like[i - 1] = merged_word_like[i - 1] || merged_word_like[i];
            merged_texts[i].clear();
        }
    }

    // Forward-sticky cluster merge (backward pass)
    for i in (0..merged_len.saturating_sub(1)).rev() {
        if merged_kinds[i] == SegmentBreakKind::Text
            && !merged_word_like[i]
            && is_forward_sticky_cluster_segment(&merged_texts[i])
        {
            let mut j = i + 1;
            while j < merged_len && merged_texts[j].is_empty() {
                j += 1;
            }
            if j < merged_len && merged_kinds[j] == SegmentBreakKind::Text {
                let prefix = merged_texts[i].clone();
                merged_texts[j] = format!("{}{}", prefix, &merged_texts[j]);
                merged_starts[j] = merged_starts[i];
                merged_texts[i].clear();
            }
        }
    }

    let compacted = compact(merged_texts, merged_word_like, merged_kinds, merged_starts);
    let with_glue = merge_glue_connected_text_runs(compacted);
    let with_merged = carry_trailing_forward_sticky_across_cjk_boundary(
        merge_ascii_punctuation_chains(split_hyphenated_numeric_runs(merge_numeric_runs(
            merge_url_query_runs(merge_url_like_runs(with_glue)),
        ))),
    );

    // Arabic combining mark split
    let mut result = with_merged;
    for i in 0..result.len().saturating_sub(1) {
        if let Some((_space, marks)) = split_leading_space_and_marks(&result.texts[i]) {
            let kind_i = result.kinds[i];
            if (kind_i != SegmentBreakKind::Space && kind_i != SegmentBreakKind::PreservedSpace)
                || result.kinds[i + 1] != SegmentBreakKind::Text
                || !contains_arabic_script(&result.texts[i + 1])
            {
                continue;
            }
            let marks_str = marks.to_string();
            result.texts[i] = " ".to_string();
            result.is_word_like[i] = false;
            result.kinds[i] = if kind_i == SegmentBreakKind::PreservedSpace {
                SegmentBreakKind::PreservedSpace
            } else {
                SegmentBreakKind::Space
            };
            let next_text = result.texts[i + 1].clone();
            result.texts[i + 1] = format!("{marks_str}{next_text}");
            result.starts[i + 1] = result.starts[i] + 1; // 1 byte for space
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Analysis chunks
// ---------------------------------------------------------------------------

fn compile_analysis_chunks(
    seg: &MergedSegmentation,
    ws_profile: &WhiteSpaceProfile,
) -> Vec<AnalysisChunk> {
    if seg.is_empty() {
        return Vec::new();
    }
    if !ws_profile.preserve_hard_breaks {
        return vec![AnalysisChunk {
            start_segment_index: 0,
            end_segment_index: seg.len(),
            consumed_end_segment_index: seg.len(),
        }];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    for i in 0..seg.len() {
        if seg.kinds[i] != SegmentBreakKind::HardBreak {
            continue;
        }
        chunks.push(AnalysisChunk {
            start_segment_index: start,
            end_segment_index: i,
            consumed_end_segment_index: i + 1,
        });
        start = i + 1;
    }

    if start < seg.len() {
        chunks.push(AnalysisChunk {
            start_segment_index: start,
            end_segment_index: seg.len(),
            consumed_end_segment_index: seg.len(),
        });
    }

    chunks
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn analyze_text(
    text: &str,
    profile: &AnalysisProfile,
    white_space: WhiteSpaceMode,
) -> TextAnalysis {
    let ws_profile = get_white_space_profile(white_space);
    let normalized = match ws_profile.mode {
        WhiteSpaceMode::PreWrap => normalize_whitespace_pre_wrap(text),
        WhiteSpaceMode::Normal => normalize_whitespace_normal(text),
    };

    if normalized.is_empty() {
        return TextAnalysis {
            normalized,
            chunks: Vec::new(),
            texts: Vec::new(),
            is_word_like: Vec::new(),
            kinds: Vec::new(),
            starts: Vec::new(),
        };
    }

    let segmentation = build_merged_segmentation(&normalized, profile, &ws_profile);
    let chunks = compile_analysis_chunks(&segmentation, &ws_profile);

    TextAnalysis {
        normalized,
        chunks,
        texts: segmentation.texts,
        is_word_like: segmentation.is_word_like,
        kinds: segmentation.kinds,
        starts: segmentation.starts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_normal() {
        assert_eq!(
            normalize_whitespace_normal("  hello   world  "),
            "hello world"
        );
        assert_eq!(normalize_whitespace_normal("a\tb\nc"), "a b c");
        assert_eq!(normalize_whitespace_normal("hello"), "hello");
    }

    #[test]
    fn test_normalize_pre_wrap() {
        assert_eq!(normalize_whitespace_pre_wrap("a\r\nb"), "a\nb");
        assert_eq!(normalize_whitespace_pre_wrap("a\rb"), "a\nb");
    }

    #[test]
    fn test_is_cjk() {
        assert!(is_cjk("你好"));
        assert!(is_cjk("こんにちは"));
        assert!(!is_cjk("hello"));
    }

    #[test]
    fn test_analyze_empty() {
        let profile = AnalysisProfile {
            carry_cjk_after_closing_quote: false,
        };
        let result = analyze_text("", &profile, WhiteSpaceMode::Normal);
        assert!(result.is_empty());
    }

    #[test]
    fn test_analyze_simple() {
        let profile = AnalysisProfile {
            carry_cjk_after_closing_quote: false,
        };
        let result = analyze_text("hello world", &profile, WhiteSpaceMode::Normal);
        assert!(result.len() >= 3); // "hello", " ", "world"
    }

    #[test]
    fn test_analyze_pre_wrap_hard_breaks() {
        let profile = AnalysisProfile {
            carry_cjk_after_closing_quote: false,
        };
        let result = analyze_text("a\nb\nc", &profile, WhiteSpaceMode::PreWrap);
        assert!(result.chunks.len() >= 3);
    }
}
