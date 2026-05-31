//! Delimiter-separated values (DSV) parser
//!
//! Low-level DSV parsing that handles any delimiter.

use std::collections::{HashMap, HashSet};

/// A row from a DSV file, stored as a HashMap of column name to value.
pub type DsvRow = HashMap<String, String>;

/// Result type for DSV parser operations.
pub type DsvResult<T> = Result<T, DsvParseError>;

/// Policy for rows whose field count differs from the header count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnPolicy {
    /// Match D3's convenient behavior: missing cells become empty strings and
    /// extra cells are ignored by header-based row parsing.
    D3Compatible,
    /// Reject rows whose field count differs from the header count. Also
    /// rejects empty and duplicate headers because they cannot round-trip
    /// cleanly through `DsvRow`.
    Strict,
}

/// Structured DSV parse error kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DsvParseErrorKind {
    UnterminatedQuotedField,
    UnexpectedQuote,
    InvalidDelimiter,
    HeaderColumnMismatch { expected: usize, actual: usize },
    EmptyHeader { index: usize },
    DuplicateHeader { name: String },
}

impl std::fmt::Display for DsvParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnterminatedQuotedField => write!(f, "unterminated quoted field"),
            Self::UnexpectedQuote => write!(f, "unexpected quote in unquoted field"),
            Self::InvalidDelimiter => {
                write!(f, "delimiter cannot be quote, carriage return, or newline")
            }
            Self::HeaderColumnMismatch { expected, actual } => {
                write!(
                    f,
                    "row has {actual} columns but header has {expected} columns"
                )
            }
            Self::EmptyHeader { index } => write!(f, "header at index {index} is empty"),
            Self::DuplicateHeader { name } => write!(f, "duplicate header {name:?}"),
        }
    }
}

/// Recoverable DSV parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsvParseError {
    pub line: usize,
    pub column: usize,
    pub byte_offset: usize,
    pub kind: DsvParseErrorKind,
    pub message: String,
}

impl DsvParseError {
    pub fn new(line: usize, column: usize, byte_offset: usize, kind: DsvParseErrorKind) -> Self {
        let message = kind.to_string();
        Self {
            line,
            column,
            byte_offset,
            kind,
            message,
        }
    }
}

impl std::fmt::Display for DsvParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

impl std::error::Error for DsvParseError {}

#[derive(Debug, Clone)]
struct ParsedRecord {
    line: usize,
    byte_offset: usize,
    fields: Vec<String>,
}

/// A DSV parser that can be configured with any delimiter.
///
/// # Example
///
/// ```
/// use d3rs::fetch::DsvParser;
///
/// let parser = DsvParser::new(',');
/// let data = "name,age\nalice,30\nbob,25";
/// let rows = parser.parse(data).unwrap();
/// assert_eq!(rows.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct DsvParser {
    delimiter: char,
    skip_empty_lines: bool,
    trim_values: bool,
    column_policy: ColumnPolicy,
}

impl DsvParser {
    /// Create a new parser with the given delimiter.
    pub fn new(delimiter: char) -> Self {
        Self {
            delimiter,
            skip_empty_lines: true,
            trim_values: true,
            column_policy: ColumnPolicy::D3Compatible,
        }
    }

    /// Set whether to skip empty lines.
    pub fn skip_empty_lines(mut self, skip: bool) -> Self {
        self.skip_empty_lines = skip;
        self
    }

    /// Set whether to trim whitespace from values.
    pub fn trim_values(mut self, trim: bool) -> Self {
        self.trim_values = trim;
        self
    }

    /// Set row/header width validation policy.
    pub fn column_policy(mut self, policy: ColumnPolicy) -> Self {
        self.column_policy = policy;
        self
    }

    /// Parse a DSV string into rows.
    ///
    /// The first record is treated as the header row.
    pub fn parse(&self, text: &str) -> DsvResult<Vec<DsvRow>> {
        let mut records = self.parse_records(text)?;
        if records.is_empty() {
            return Ok(Vec::new());
        }

        let header = records.remove(0);
        self.validate_headers(&header.fields, header.line, header.byte_offset)?;

        let mut rows = Vec::with_capacity(records.len());
        for record in records {
            if self.column_policy == ColumnPolicy::Strict
                && record.fields.len() != header.fields.len()
            {
                return Err(DsvParseError::new(
                    record.line,
                    1,
                    record.byte_offset,
                    DsvParseErrorKind::HeaderColumnMismatch {
                        expected: header.fields.len(),
                        actual: record.fields.len(),
                    },
                ));
            }

            let mut row = DsvRow::new();
            for (i, name) in header.fields.iter().enumerate() {
                row.insert(
                    name.clone(),
                    record.fields.get(i).cloned().unwrap_or_default(),
                );
            }
            rows.push(row);
        }
        Ok(rows)
    }

    /// Parse a DSV string into rows and return an empty vector if parsing fails.
    ///
    /// Prefer [`Self::parse`] for data ingestion. This helper exists for
    /// D3-compatible demos where malformed static input should render as no data.
    pub fn parse_lossy(&self, text: &str) -> Vec<DsvRow> {
        self.parse(text).unwrap_or_default()
    }

    /// Compatibility alias for callers that still use the older fallible name.
    pub fn try_parse(&self, text: &str) -> DsvResult<Vec<DsvRow>> {
        self.parse(text)
    }

    /// Parse a DSV string without headers (returns arrays of strings).
    pub fn parse_rows(&self, text: &str) -> DsvResult<Vec<Vec<String>>> {
        Ok(self
            .parse_records(text)?
            .into_iter()
            .map(|record| record.fields)
            .collect())
    }

    /// Parse rows without headers and return an empty vector if parsing fails.
    pub fn parse_rows_lossy(&self, text: &str) -> Vec<Vec<String>> {
        self.parse_rows(text).unwrap_or_default()
    }

    /// Compatibility alias for callers that still use the older fallible name.
    pub fn try_parse_rows(&self, text: &str) -> DsvResult<Vec<Vec<String>>> {
        self.parse_rows(text)
    }

    fn parse_records(&self, text: &str) -> DsvResult<Vec<ParsedRecord>> {
        self.validate_delimiter()?;

        if text.is_empty() {
            return Ok(Vec::new());
        }

        let chars: Vec<(usize, char)> = text.char_indices().collect();
        let mut records = Vec::new();
        let mut record = Vec::new();
        let mut field = String::new();
        let mut in_quotes = false;
        let mut at_field_start = true;
        let mut after_quote = false;
        let mut last_was_record_terminator = false;
        let mut record_has_content = false;
        let mut line = 1usize;
        let mut column = 1usize;
        let mut record_start_line = 1usize;
        let mut record_start_byte = 0usize;
        let mut i = 0usize;

        while i < chars.len() {
            let (byte_offset, ch) = chars[i];
            last_was_record_terminator = false;

            if in_quotes {
                if ch == '"' {
                    if chars.get(i + 1).map(|(_, c)| *c) == Some('"') {
                        field.push('"');
                        i += 2;
                        column += 2;
                    } else {
                        in_quotes = false;
                        after_quote = true;
                        i += 1;
                        column += 1;
                    }
                    continue;
                }

                if ch == '\r' || ch == '\n' {
                    field.push('\n');
                    if ch == '\r' && chars.get(i + 1).map(|(_, c)| *c) == Some('\n') {
                        i += 2;
                    } else {
                        i += 1;
                    }
                    line += 1;
                    column = 1;
                    continue;
                }

                field.push(ch);
                i += 1;
                column += 1;
                continue;
            }

            if after_quote {
                if ch == self.delimiter {
                    self.finish_field(&mut record, &mut field);
                    after_quote = false;
                    at_field_start = true;
                    i += 1;
                    column += 1;
                    continue;
                }

                if ch == '\r' || ch == '\n' {
                    self.finish_field(&mut record, &mut field);
                    self.finish_record(
                        &mut records,
                        &mut record,
                        record_start_line,
                        record_start_byte,
                        record_has_content,
                    );
                    after_quote = false;
                    at_field_start = true;
                    if ch == '\r' && chars.get(i + 1).map(|(_, c)| *c) == Some('\n') {
                        i += 2;
                    } else {
                        i += 1;
                    }
                    line += 1;
                    column = 1;
                    record_start_line = line;
                    record_start_byte = chars.get(i).map(|(idx, _)| *idx).unwrap_or(text.len());
                    record_has_content = false;
                    last_was_record_terminator = true;
                    continue;
                }

                if self.trim_values && ch.is_whitespace() {
                    i += 1;
                    column += 1;
                    continue;
                }

                return Err(DsvParseError::new(
                    line,
                    column,
                    byte_offset,
                    DsvParseErrorKind::UnexpectedQuote,
                ));
            }

            if ch == '"' {
                if at_field_start || (self.trim_values && field.trim().is_empty()) {
                    if self.trim_values {
                        field.clear();
                    }
                    record_has_content = true;
                    in_quotes = true;
                    at_field_start = false;
                    i += 1;
                    column += 1;
                    continue;
                }

                return Err(DsvParseError::new(
                    line,
                    column,
                    byte_offset,
                    DsvParseErrorKind::UnexpectedQuote,
                ));
            }

            if ch == self.delimiter {
                record_has_content = true;
                self.finish_field(&mut record, &mut field);
                at_field_start = true;
                i += 1;
                column += 1;
                continue;
            }

            if ch == '\r' || ch == '\n' {
                self.finish_field(&mut record, &mut field);
                self.finish_record(
                    &mut records,
                    &mut record,
                    record_start_line,
                    record_start_byte,
                    record_has_content,
                );
                at_field_start = true;
                if ch == '\r' && chars.get(i + 1).map(|(_, c)| *c) == Some('\n') {
                    i += 2;
                } else {
                    i += 1;
                }
                line += 1;
                column = 1;
                record_start_line = line;
                record_start_byte = chars.get(i).map(|(idx, _)| *idx).unwrap_or(text.len());
                record_has_content = false;
                last_was_record_terminator = true;
                continue;
            }

            if !ch.is_whitespace() {
                record_has_content = true;
            }
            field.push(ch);
            at_field_start = false;
            i += 1;
            column += 1;
        }

        if in_quotes {
            return Err(DsvParseError::new(
                line,
                column,
                text.len(),
                DsvParseErrorKind::UnterminatedQuotedField,
            ));
        }

        if !last_was_record_terminator || !record.is_empty() || !field.is_empty() {
            self.finish_field(&mut record, &mut field);
            self.finish_record(
                &mut records,
                &mut record,
                record_start_line,
                record_start_byte,
                record_has_content,
            );
        }

        Ok(records)
    }

    fn validate_delimiter(&self) -> DsvResult<()> {
        if matches!(self.delimiter, '"' | '\r' | '\n') {
            Err(DsvParseError::new(
                1,
                1,
                0,
                DsvParseErrorKind::InvalidDelimiter,
            ))
        } else {
            Ok(())
        }
    }

    fn finish_field(&self, record: &mut Vec<String>, field: &mut String) {
        let value = std::mem::take(field);
        if self.trim_values {
            record.push(value.trim().to_string());
        } else {
            record.push(value);
        }
    }

    fn finish_record(
        &self,
        records: &mut Vec<ParsedRecord>,
        record: &mut Vec<String>,
        line: usize,
        byte_offset: usize,
        record_has_content: bool,
    ) {
        if self.skip_empty_lines && !record_has_content {
            record.clear();
            return;
        }

        records.push(ParsedRecord {
            line,
            byte_offset,
            fields: std::mem::take(record),
        });
    }

    fn validate_headers(
        &self,
        headers: &[String],
        line: usize,
        byte_offset: usize,
    ) -> DsvResult<()> {
        if self.column_policy != ColumnPolicy::Strict {
            return Ok(());
        }

        let mut seen = HashSet::new();
        for (index, header) in headers.iter().enumerate() {
            if header.is_empty() {
                return Err(DsvParseError::new(
                    line,
                    index + 1,
                    byte_offset,
                    DsvParseErrorKind::EmptyHeader { index },
                ));
            }
            if !seen.insert(header) {
                return Err(DsvParseError::new(
                    line,
                    index + 1,
                    byte_offset,
                    DsvParseErrorKind::DuplicateHeader {
                        name: header.clone(),
                    },
                ));
            }
        }

        Ok(())
    }

    /// Format rows as DSV text.
    pub fn format(&self, rows: &[DsvRow], columns: &[&str]) -> String {
        let mut result = String::new();

        // Header
        result.push_str(&columns.join(&self.delimiter.to_string()));
        result.push('\n');

        // Data rows
        for row in rows {
            let values: Vec<String> = columns
                .iter()
                .map(|col| {
                    let value = row.get(*col).cloned().unwrap_or_default();
                    self.format_value(&value)
                })
                .collect();
            result.push_str(&values.join(&self.delimiter.to_string()));
            result.push('\n');
        }

        result
    }

    /// Format rows from arrays as DSV text.
    pub fn format_rows(&self, rows: &[Vec<String>]) -> String {
        rows.iter()
            .map(|row| {
                row.iter()
                    .map(|v| self.format_value(v))
                    .collect::<Vec<_>>()
                    .join(&self.delimiter.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Format a value, quoting if necessary.
    fn format_value(&self, value: &str) -> String {
        if value.contains(self.delimiter) || value.contains('"') || value.contains('\n') {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_string()
        }
    }
}

/// Parse a DSV string with the given delimiter.
///
/// # Example
///
/// ```
/// use d3rs::fetch::parse_dsv;
///
/// let data = "name|age\nalice|30\nbob|25";
/// let rows = parse_dsv(data, '|').unwrap();
/// assert_eq!(rows.len(), 2);
/// assert_eq!(rows[0].get("age"), Some(&"30".to_string()));
/// ```
pub fn parse_dsv(text: &str, delimiter: char) -> DsvResult<Vec<DsvRow>> {
    DsvParser::new(delimiter).parse(text)
}

/// Parse a DSV string and return an empty vector if parsing fails.
pub fn parse_dsv_lossy(text: &str, delimiter: char) -> Vec<DsvRow> {
    DsvParser::new(delimiter).parse_lossy(text)
}

/// Compatibility alias for callers that still use the older fallible name.
pub fn try_parse_dsv(text: &str, delimiter: char) -> DsvResult<Vec<DsvRow>> {
    parse_dsv(text, delimiter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let data = "name,value\nalice,10\nbob,20";
        let rows = parse_dsv(data, ',').unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("name"), Some(&"alice".to_string()));
        assert_eq!(rows[0].get("value"), Some(&"10".to_string()));
    }

    #[test]
    fn test_parse_quoted() {
        let data = "name,message\nalice,\"hello, world\"\nbob,\"say \"\"hi\"\"\"";
        let rows = parse_dsv(data, ',').unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("message"), Some(&"hello, world".to_string()));
        assert_eq!(rows[1].get("message"), Some(&"say \"hi\"".to_string()));
    }

    #[test]
    fn test_parse_quoted_newline() {
        let data = "name,message\nalice,\"hello\nworld\"\nbob,done";
        let rows = parse_dsv(data, ',').unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("message"), Some(&"hello\nworld".to_string()));
        assert_eq!(rows[1].get("message"), Some(&"done".to_string()));
    }

    #[test]
    fn test_parse_crlf() {
        let data = "name,value\r\nalice,10\r\nbob,20\r\n";
        let rows = parse_dsv(data, ',').unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].get("name"), Some(&"bob".to_string()));
    }

    #[test]
    fn test_parse_empty_values() {
        let data = "a,b,c\n1,,3\n,2,";
        let rows = parse_dsv(data, ',').unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("b"), Some(&String::new()));
        assert_eq!(rows[1].get("a"), Some(&String::new()));
    }

    #[test]
    fn test_parse_rows() {
        let parser = DsvParser::new(',');
        let data = "1,2,3\n4,5,6";
        let rows = parser.parse_rows(data).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["1", "2", "3"]);
    }

    #[test]
    fn test_empty_field_records_are_not_empty_lines() {
        let parser = DsvParser::new(',');
        let rows = parser.parse_rows(",\n\"\"\n   ").unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["", ""]);
        assert_eq!(rows[1], vec![""]);
    }

    #[test]
    fn test_try_parse_reports_unclosed_quote() {
        let parser = DsvParser::new(',');
        let err = parser
            .parse("name,value\nalice,\"broken")
            .expect_err("unterminated quote should be an error");

        assert_eq!(err.line, 2);
        assert!(err.message.contains("unterminated"));
        assert_eq!(err.kind, DsvParseErrorKind::UnterminatedQuotedField);
        assert!(parser.parse_lossy("name,value\nalice,\"broken").is_empty());
    }

    #[test]
    fn test_unexpected_quote_reports_location() {
        let parser = DsvParser::new(',');
        let err = parser.parse("name,value\nalice,br\"oken").unwrap_err();

        assert_eq!(err.line, 2);
        assert_eq!(err.column, 9);
        assert_eq!(err.kind, DsvParseErrorKind::UnexpectedQuote);
    }

    #[test]
    fn test_strict_column_mismatch() {
        let parser = DsvParser::new(',').column_policy(ColumnPolicy::Strict);
        let err = parser.parse("a,b\n1,2,3").unwrap_err();

        assert_eq!(
            err.kind,
            DsvParseErrorKind::HeaderColumnMismatch {
                expected: 2,
                actual: 3
            }
        );
    }

    #[test]
    fn test_strict_empty_and_duplicate_headers() {
        let empty = DsvParser::new(',')
            .column_policy(ColumnPolicy::Strict)
            .parse("a,,c\n1,2,3")
            .unwrap_err();
        assert_eq!(empty.kind, DsvParseErrorKind::EmptyHeader { index: 1 });

        let duplicate = DsvParser::new(',')
            .column_policy(ColumnPolicy::Strict)
            .parse("a,b,a\n1,2,3")
            .unwrap_err();
        assert_eq!(
            duplicate.kind,
            DsvParseErrorKind::DuplicateHeader {
                name: "a".to_string()
            }
        );
    }

    #[test]
    fn test_invalid_delimiter() {
        let err = DsvParser::new('\n').parse("a,b\n1,2").unwrap_err();
        assert_eq!(err.kind, DsvParseErrorKind::InvalidDelimiter);
    }

    #[test]
    fn test_format() {
        let parser = DsvParser::new(',');
        let mut row1 = DsvRow::new();
        row1.insert("name".to_string(), "alice".to_string());
        row1.insert("value".to_string(), "10".to_string());

        let mut row2 = DsvRow::new();
        row2.insert("name".to_string(), "bob".to_string());
        row2.insert("value".to_string(), "20".to_string());

        let result = parser.format(&[row1, row2], &["name", "value"]);
        assert!(result.contains("alice,10"));
        assert!(result.contains("bob,20"));
    }

    #[test]
    fn test_format_quoted() {
        let parser = DsvParser::new(',');
        let mut row = DsvRow::new();
        row.insert("text".to_string(), "hello, \"world\"".to_string());

        let result = parser.format(&[row], &["text"]);
        assert!(result.contains("\"hello, \"\"world\"\"\""));
    }
}
