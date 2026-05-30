//! Delimiter-separated values (DSV) parser
//!
//! Low-level DSV parsing that handles any delimiter.

use std::collections::HashMap;

/// A row from a DSV file, stored as a HashMap of column name to value.
pub type DsvRow = HashMap<String, String>;

/// Recoverable DSV parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsvParseError {
    pub line: usize,
    pub message: String,
}

impl DsvParseError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DsvParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for DsvParseError {}

/// A DSV parser that can be configured with any delimiter.
///
/// # Example
///
/// ```
/// use d3rs::fetch::DsvParser;
///
/// let parser = DsvParser::new(',');
/// let data = "name,age\nalice,30\nbob,25";
/// let rows = parser.parse(data);
/// assert_eq!(rows.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct DsvParser {
    delimiter: char,
    skip_empty_lines: bool,
    trim_values: bool,
}

impl DsvParser {
    /// Create a new parser with the given delimiter.
    pub fn new(delimiter: char) -> Self {
        Self {
            delimiter,
            skip_empty_lines: true,
            trim_values: true,
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

    /// Parse a DSV string into rows.
    ///
    /// The first line is treated as the header row.
    pub fn parse(&self, text: &str) -> Vec<DsvRow> {
        self.try_parse(text).unwrap_or_default()
    }

    /// Parse a DSV string into rows, returning structured errors.
    pub fn try_parse(&self, text: &str) -> Result<Vec<DsvRow>, DsvParseError> {
        let mut lines = text.lines();

        // Get header line
        let header_line = match lines.next() {
            Some(line) => line,
            None => return Ok(Vec::new()),
        };

        let headers: Vec<String> = self
            .try_parse_line(header_line, 1)?
            .into_iter()
            .map(|s| {
                if self.trim_values {
                    s.trim().to_string()
                } else {
                    s
                }
            })
            .collect();

        // Parse data lines
        let mut rows = Vec::new();
        for (line_index, line) in lines.enumerate() {
            if self.skip_empty_lines && line.trim().is_empty() {
                continue;
            }
            let values = self.try_parse_line(line, line_index + 2)?;
            let mut row = DsvRow::new();
            for (i, header) in headers.iter().enumerate() {
                let value = values.get(i).cloned().unwrap_or_default();
                let value = if self.trim_values {
                    value.trim().to_string()
                } else {
                    value
                };
                row.insert(header.clone(), value);
            }
            rows.push(row);
        }
        Ok(rows)
    }

    /// Parse a DSV string without headers (returns arrays of strings).
    pub fn parse_rows(&self, text: &str) -> Vec<Vec<String>> {
        self.try_parse_rows(text).unwrap_or_default()
    }

    /// Parse rows without headers, returning structured errors.
    pub fn try_parse_rows(&self, text: &str) -> Result<Vec<Vec<String>>, DsvParseError> {
        text.lines()
            .enumerate()
            .filter(|(_, line)| !self.skip_empty_lines || !line.trim().is_empty())
            .map(|(line_index, line)| {
                self.try_parse_line(line, line_index + 1).map(|values| {
                    if self.trim_values {
                        values.into_iter().map(|s| s.trim().to_string()).collect()
                    } else {
                        values
                    }
                })
            })
            .collect()
    }

    /// Parse a single line, handling quoted fields and invalid quote state.
    fn try_parse_line(&self, line: &str, line_number: usize) -> Result<Vec<String>, DsvParseError> {
        let mut result = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            if in_quotes {
                if c == '"' {
                    // Check for escaped quote
                    if chars.peek() == Some(&'"') {
                        current.push('"');
                        chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    current.push(c);
                }
            } else if c == '"' {
                in_quotes = true;
            } else if c == self.delimiter {
                result.push(current);
                current = String::new();
            } else {
                current.push(c);
            }
        }

        result.push(current);
        if in_quotes {
            Err(DsvParseError::new(line_number, "unterminated quoted field"))
        } else {
            Ok(result)
        }
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
/// let rows = parse_dsv(data, '|');
/// assert_eq!(rows.len(), 2);
/// assert_eq!(rows[0].get("age"), Some(&"30".to_string()));
/// ```
pub fn parse_dsv(text: &str, delimiter: char) -> Vec<DsvRow> {
    DsvParser::new(delimiter).parse(text)
}

/// Parse a DSV string with the given delimiter, returning structured errors.
pub fn try_parse_dsv(text: &str, delimiter: char) -> Result<Vec<DsvRow>, DsvParseError> {
    DsvParser::new(delimiter).try_parse(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let data = "name,value\nalice,10\nbob,20";
        let rows = parse_dsv(data, ',');
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("name"), Some(&"alice".to_string()));
        assert_eq!(rows[0].get("value"), Some(&"10".to_string()));
    }

    #[test]
    fn test_parse_quoted() {
        let data = "name,message\nalice,\"hello, world\"\nbob,\"say \"\"hi\"\"\"";
        let rows = parse_dsv(data, ',');
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("message"), Some(&"hello, world".to_string()));
        assert_eq!(rows[1].get("message"), Some(&"say \"hi\"".to_string()));
    }

    #[test]
    fn test_parse_empty_values() {
        let data = "a,b,c\n1,,3\n,2,";
        let rows = parse_dsv(data, ',');
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].get("b"), Some(&String::new()));
        assert_eq!(rows[1].get("a"), Some(&String::new()));
    }

    #[test]
    fn test_parse_rows() {
        let parser = DsvParser::new(',');
        let data = "1,2,3\n4,5,6";
        let rows = parser.parse_rows(data);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["1", "2", "3"]);
    }

    #[test]
    fn test_try_parse_reports_unclosed_quote() {
        let parser = DsvParser::new(',');
        let err = parser
            .try_parse("name,value\nalice,\"broken")
            .expect_err("unterminated quote should be an error");

        assert_eq!(err.line, 2);
        assert!(err.message.contains("unterminated"));
        assert!(parser.parse("name,value\nalice,\"broken").is_empty());
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
