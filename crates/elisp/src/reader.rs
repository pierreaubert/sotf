use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub struct Reader {
    input: Vec<char>,
    pos: usize,
}

fn is_symbol_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '*' | '/'
                | '='
                | '<'
                | '>'
                | '_'
                | '-'
                | '+'
                | '?'
                | '!'
                | '&'
                | ':'
                | '.'
                | '%'
                | '$'
                | '@'
                | '~'
                | '^'
        )
}

fn is_symbol_initial(c: char) -> bool {
    c.is_alphabetic()
        || matches!(
            c,
            '*' | '/'
                | '='
                | '<'
                | '>'
                | '_'
                | '-'
                | '+'
                | '?'
                | '!'
                | '&'
                | ':'
                | '%'
                | '$'
                | '@'
                | '~'
                | '^'
        )
}

impl Reader {
    pub fn new(source: &str) -> Self {
        Reader {
            input: source.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_ahead(&self, n: usize) -> Option<char> {
        self.input.get(self.pos + n).copied()
    }

    fn advance(&mut self) -> Option<char> {
        if self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else if c == ';' {
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn is_delimiter(c: char) -> bool {
        c.is_whitespace() || c == '(' || c == ')' || c == '"' || c == ';' || c == '\''
    }

    pub fn read(&mut self) -> ElispResult<LispObject> {
        self.skip_whitespace();

        let c = match self.advance() {
            Some(c) => c,
            None => {
                return Err(ElispError::ReaderError(
                    "unexpected end of input".to_string(),
                ))
            }
        };

        match c {
            '(' => self.read_list(),
            ')' => Err(ElispError::ReaderError("unexpected )".to_string())),
            '\'' => {
                let quoted = self.read()?;
                Ok(LispObject::cons(
                    LispObject::symbol("quote"),
                    LispObject::cons(quoted, LispObject::nil()),
                ))
            }
            '`' => {
                let form = self.read()?;
                Ok(LispObject::cons(
                    LispObject::symbol("\\`"),
                    LispObject::cons(form, LispObject::nil()),
                ))
            }
            ',' => {
                if self.peek() == Some('@') {
                    self.advance();
                    let form = self.read()?;
                    Ok(LispObject::cons(
                        LispObject::symbol("\\,@"),
                        LispObject::cons(form, LispObject::nil()),
                    ))
                } else {
                    let form = self.read()?;
                    Ok(LispObject::cons(
                        LispObject::symbol("\\,"),
                        LispObject::cons(form, LispObject::nil()),
                    ))
                }
            }
            '#' => self.read_hash(),
            '?' => self.read_char_literal(),
            '"' => self.read_string(),
            '+' | '-' => {
                let next = self.peek();
                if next.map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    self.read_number_from(c)
                } else if c == '-' && next == Some('.') {
                    // Check for -.5 style floats
                    if self
                        .peek_ahead(1)
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                    {
                        self.read_number_from(c)
                    } else {
                        Ok(LispObject::symbol("-"))
                    }
                } else if c == '+' && next == Some('.') {
                    if self
                        .peek_ahead(1)
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                    {
                        self.read_number_from(c)
                    } else {
                        Ok(LispObject::symbol("+"))
                    }
                } else {
                    let s = String::from(c);
                    Ok(LispObject::symbol(&s))
                }
            }
            c if c.is_ascii_digit() => self.read_number_from(c),
            '.' => {
                // Could be a float like .5 or a symbol starting with .
                if self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    self.read_number_from('.')
                } else {
                    self.read_symbol('.')
                }
            }
            '\\' => {
                // Escaped symbol: \` \, \,@ etc.
                // The backslash makes the next character(s) part of a symbol name
                let next = self.advance().ok_or_else(|| {
                    ElispError::ReaderError("unexpected end of input after \\".to_string())
                })?;
                self.read_escaped_symbol(next)
            }
            c if is_symbol_initial(c) => self.read_symbol(c),
            '[' => self.read_vector(),
            _ => Err(ElispError::ReaderError(format!(
                "unexpected character: {}",
                c
            ))),
        }
    }

    fn read_list(&mut self) -> ElispResult<LispObject> {
        self.skip_whitespace();

        if let Some(c) = self.peek() {
            if c == ')' {
                self.advance();
                return Ok(LispObject::nil());
            }
        }

        let car = self.read()?;

        self.skip_whitespace();

        // Check for dotted pair: (a . b)
        if self.peek() == Some('.') {
            // Peek ahead to check it's a dot delimiter, not a symbol or number
            let after_dot = self.peek_ahead(1);
            if after_dot.map(|c| Self::is_delimiter(c)).unwrap_or(true) {
                self.advance(); // consume '.'
                let cdr = self.read()?;
                self.skip_whitespace();
                if self.peek() != Some(')') {
                    return Err(ElispError::ReaderError(
                        "expected ) after dotted pair".to_string(),
                    ));
                }
                self.advance(); // consume ')'
                return Ok(LispObject::cons(car, cdr));
            }
        }

        let cdr = self.read_list()?;
        Ok(LispObject::cons(car, cdr))
    }

    fn read_vector(&mut self) -> ElispResult<LispObject> {
        let mut elements = Vec::new();
        loop {
            self.skip_whitespace();
            if let Some(c) = self.peek() {
                if c == ']' {
                    self.advance();
                    break;
                }
            } else {
                return Err(ElispError::ReaderError(
                    "unterminated vector literal".to_string(),
                ));
            }
            elements.push(self.read()?);
        }
        // Build as a list tagged with 'vector for now (proper Vector type comes in 1B)
        let mut list = LispObject::nil();
        for e in elements.into_iter().rev() {
            list = LispObject::cons(e, list);
        }
        Ok(LispObject::cons(LispObject::symbol("vector"), list))
    }

    fn read_hash(&mut self) -> ElispResult<LispObject> {
        let c = self.peek().ok_or_else(|| {
            ElispError::ReaderError("unexpected end of input after #".to_string())
        })?;
        match c {
            '\'' => {
                self.advance();
                let form = self.read()?;
                Ok(LispObject::cons(
                    LispObject::symbol("function"),
                    LispObject::cons(form, LispObject::nil()),
                ))
            }
            'x' | 'X' => {
                self.advance();
                self.read_radix_number(16)
            }
            'o' | 'O' => {
                self.advance();
                self.read_radix_number(8)
            }
            'b' | 'B' => {
                self.advance();
                self.read_radix_number(2)
            }
            's' => {
                self.advance();
                // #s(hash-table ...) — read as tagged form for now
                if self.peek() == Some('(') {
                    self.advance();
                    let inner = self.read_list_to_vec()?;
                    let mut list = LispObject::nil();
                    for e in inner.into_iter().rev() {
                        list = LispObject::cons(e, list);
                    }
                    Ok(LispObject::cons(
                        LispObject::symbol("hash-table-literal"),
                        list,
                    ))
                } else {
                    Err(ElispError::ReaderError("expected ( after #s".to_string()))
                }
            }
            _ => Err(ElispError::ReaderError(format!(
                "unknown # dispatch: #{}",
                c
            ))),
        }
    }

    fn read_list_to_vec(&mut self) -> ElispResult<Vec<LispObject>> {
        let mut elements = Vec::new();
        loop {
            self.skip_whitespace();
            if let Some(c) = self.peek() {
                if c == ')' {
                    self.advance();
                    break;
                }
            } else {
                return Err(ElispError::ReaderError("unterminated list".to_string()));
            }
            elements.push(self.read()?);
        }
        Ok(elements)
    }

    fn read_char_literal(&mut self) -> ElispResult<LispObject> {
        let c = self.advance().ok_or_else(|| {
            ElispError::ReaderError("unexpected end of input in char literal".to_string())
        })?;
        if c == '\\' {
            // Escape sequence
            let esc = self.advance().ok_or_else(|| {
                ElispError::ReaderError("unexpected end of input in char escape".to_string())
            })?;
            let ch = match esc {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                'a' => '\x07', // bell
                'b' => '\x08', // backspace
                'f' => '\x0C', // form feed
                'e' => '\x1B', // escape
                's' => ' ',    // space
                'd' => '\x7F', // delete
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                '(' => '(',
                ')' => ')',
                '[' => '[',
                ']' => ']',
                'x' => {
                    // Hex character: ?\xNN
                    let mut hex = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() {
                            hex.push(c);
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    let code = u32::from_str_radix(&hex, 16).map_err(|_| {
                        ElispError::ReaderError(format!("invalid hex char: \\x{}", hex))
                    })?;
                    char::from_u32(code).ok_or_else(|| {
                        ElispError::ReaderError(format!("invalid unicode: \\x{}", hex))
                    })?
                }
                c => c, // ?\X for any other X is just X
            };
            Ok(LispObject::Integer(ch as i64))
        } else {
            // Plain character: ?a
            Ok(LispObject::Integer(c as i64))
        }
    }

    fn read_string(&mut self) -> ElispResult<LispObject> {
        let mut s = String::new();
        let mut escaped = false;

        while let Some(c) = self.advance() {
            if escaped {
                match c {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    'a' => s.push('\x07'),
                    'b' => s.push('\x08'),
                    'f' => s.push('\x0C'),
                    'e' => s.push('\x1B'),
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    '\n' => {} // backslash-newline: skip both
                    _ => {
                        s.push('\\');
                        s.push(c);
                    }
                }
                escaped = false;
                continue;
            }
            if c == '\\' {
                escaped = true;
                continue;
            }
            if c == '"' {
                return Ok(LispObject::string(&s));
            }
            s.push(c);
        }

        Err(ElispError::ReaderError("unterminated string".to_string()))
    }

    fn read_number_from(&mut self, first: char) -> ElispResult<LispObject> {
        let mut s = String::new();
        s.push(first);
        let mut has_dot = first == '.';
        let mut has_exp = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' && !has_dot && !has_exp {
                // Check it's a decimal dot, not a dotted-pair dot
                let next = self.peek_ahead(1);
                if next
                    .map(|c| c.is_ascii_digit() || c == 'e' || c == 'E')
                    .unwrap_or(false)
                {
                    has_dot = true;
                    s.push(c);
                    self.advance();
                } else {
                    // Trailing dot: 1. means float 1.0
                    has_dot = true;
                    s.push(c);
                    self.advance();
                    break;
                }
            } else if (c == 'e' || c == 'E') && !has_exp {
                has_exp = true;
                has_dot = true; // exponent makes it a float
                s.push(c);
                self.advance();
                // Optional sign after exponent
                if let Some(sign) = self.peek() {
                    if sign == '+' || sign == '-' {
                        s.push(sign);
                        self.advance();
                    }
                }
            } else {
                break;
            }
        }

        if has_dot || has_exp {
            let f: f64 = s
                .parse()
                .map_err(|_| ElispError::ReaderError(format!("invalid float: {}", s)))?;
            Ok(LispObject::float(f))
        } else {
            let n: i64 = s
                .parse()
                .map_err(|_| ElispError::ReaderError(format!("invalid integer: {}", s)))?;
            Ok(LispObject::integer(n))
        }
    }

    fn read_radix_number(&mut self, radix: u32) -> ElispResult<LispObject> {
        let mut s = String::new();
        let mut has_sign = false;
        if let Some(c) = self.peek() {
            if c == '+' || c == '-' {
                has_sign = true;
                s.push(c);
                self.advance();
            }
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let digits = if has_sign { &s[1..] } else { &s };
        if digits.is_empty() {
            return Err(ElispError::ReaderError(format!(
                "invalid radix-{} number: #{}{}",
                radix,
                match radix {
                    16 => "x",
                    8 => "o",
                    2 => "b",
                    _ => "?",
                },
                s
            )));
        }
        let n = i64::from_str_radix(digits, radix).map_err(|_| {
            ElispError::ReaderError(format!("invalid radix-{} number: {}", radix, s))
        })?;
        let n = if has_sign && s.starts_with('-') {
            -n
        } else {
            n
        };
        Ok(LispObject::integer(n))
    }

    fn read_escaped_symbol(&mut self, first_escaped: char) -> ElispResult<LispObject> {
        // First char was already escaped by \, so it's always literal
        let mut s = String::new();
        s.push(first_escaped);

        while let Some(c) = self.peek() {
            if c == '\\' {
                self.advance();
                if let Some(escaped) = self.advance() {
                    s.push(escaped);
                }
            } else if is_symbol_char(c) {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        // Escaped symbols are never nil or t
        Ok(LispObject::symbol(&s))
    }

    fn read_symbol(&mut self, first: char) -> ElispResult<LispObject> {
        let mut s = String::new();
        s.push(first);
        let mut had_escape = false;

        while let Some(c) = self.peek() {
            if c == '\\' {
                had_escape = true;
                self.advance();
                if let Some(escaped) = self.advance() {
                    s.push(escaped);
                }
            } else if is_symbol_char(c) {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }

        if had_escape {
            // Escaped symbols are never interned as nil/t
            Ok(LispObject::symbol(&s))
        } else {
            match s.as_str() {
                "nil" => Ok(LispObject::nil()),
                "t" => Ok(LispObject::t()),
                _ => Ok(LispObject::symbol(&s)),
            }
        }
    }

    pub fn read_all(&mut self) -> ElispResult<Vec<LispObject>> {
        let mut result = Vec::new();
        self.skip_whitespace();
        while self.pos < self.input.len() {
            result.push(self.read()?);
            self.skip_whitespace();
        }
        Ok(result)
    }
}

pub fn read(source: &str) -> ElispResult<LispObject> {
    let mut reader = Reader::new(source);
    reader.read()
}

pub fn read_all(source: &str) -> ElispResult<Vec<LispObject>> {
    let mut reader = Reader::new(source);
    reader.read_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_nil() {
        assert_eq!(read("nil").unwrap(), LispObject::nil());
    }

    #[test]
    fn test_read_empty_list() {
        assert_eq!(read("()").unwrap(), LispObject::nil());
    }

    #[test]
    fn test_read_t() {
        assert_eq!(read("t").unwrap(), LispObject::t());
    }

    #[test]
    fn test_read_integer() {
        assert_eq!(read("42").unwrap(), LispObject::integer(42));
        assert_eq!(read("-10").unwrap(), LispObject::integer(-10));
        assert_eq!(read("+5").unwrap(), LispObject::integer(5));
    }

    #[test]
    fn test_read_float() {
        assert_eq!(read("3.14").unwrap(), LispObject::float(3.14));
        assert_eq!(read("-1.5").unwrap(), LispObject::float(-1.5));
        assert_eq!(read("1e10").unwrap(), LispObject::float(1e10));
        assert_eq!(read("1.5e-3").unwrap(), LispObject::float(1.5e-3));
        assert_eq!(read("2.0").unwrap(), LispObject::float(2.0));
        assert_eq!(read(".5").unwrap(), LispObject::float(0.5));
        // Trailing dot: 1. is float 1.0
        assert_eq!(read("1.").unwrap(), LispObject::float(1.0));
    }

    #[test]
    fn test_read_symbol() {
        assert_eq!(read("foo").unwrap(), LispObject::symbol("foo"));
        assert_eq!(read("bar-baz").unwrap(), LispObject::symbol("bar-baz"));
        assert_eq!(read(":keyword").unwrap(), LispObject::symbol(":keyword"));
        assert_eq!(read("&rest").unwrap(), LispObject::symbol("&rest"));
        assert_eq!(read("&optional").unwrap(), LispObject::symbol("&optional"));
    }

    #[test]
    fn test_read_string() {
        assert_eq!(read("\"hello\"").unwrap(), LispObject::string("hello"));
        assert_eq!(
            read("\"say \\\"hi\\\"\"").unwrap(),
            LispObject::string("say \"hi\"")
        );
    }

    #[test]
    fn test_read_quote() {
        let result = read("'foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("quote"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_backquote() {
        let result = read("`foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("\\`"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_unquote() {
        let result = read(",foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("\\,"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_unquote_splice() {
        let result = read(",@foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("\\,@"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_function_shorthand() {
        let result = read("#'foo").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("function"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_char_literal() {
        assert_eq!(read("?a").unwrap(), LispObject::integer(97));
        assert_eq!(read("?A").unwrap(), LispObject::integer(65));
        assert_eq!(read("?\\n").unwrap(), LispObject::integer(10));
        assert_eq!(read("?\\t").unwrap(), LispObject::integer(9));
        assert_eq!(read("?\\\n").unwrap(), LispObject::integer(10));
        assert_eq!(read("?\\x41").unwrap(), LispObject::integer(65));
        assert_eq!(read("? ").unwrap(), LispObject::integer(32));
    }

    #[test]
    fn test_read_radix() {
        assert_eq!(read("#xff").unwrap(), LispObject::integer(255));
        assert_eq!(read("#o77").unwrap(), LispObject::integer(63));
        assert_eq!(read("#b1010").unwrap(), LispObject::integer(10));
        assert_eq!(read("#xFF").unwrap(), LispObject::integer(255));
    }

    #[test]
    fn test_read_dotted_pair() {
        let result = read("(a . b)").unwrap();
        assert_eq!(
            result,
            LispObject::cons(LispObject::symbol("a"), LispObject::symbol("b"))
        );
    }

    #[test]
    fn test_read_dotted_pair_numbers() {
        let result = read("(1 . 2)").unwrap();
        assert_eq!(
            result,
            LispObject::cons(LispObject::integer(1), LispObject::integer(2))
        );
    }

    #[test]
    fn test_read_list() {
        let result = read("(a b c)").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("a"),
            LispObject::cons(
                LispObject::symbol("b"),
                LispObject::cons(LispObject::symbol("c"), LispObject::nil()),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_read_multiple() {
        let result = read_all("nil t 42").unwrap();
        assert_eq!(
            result,
            vec![LispObject::nil(), LispObject::t(), LispObject::integer(42),]
        );
    }

    #[test]
    fn test_reader_quote_detail() {
        let r = read("'(2 3)").unwrap();
        let expected = LispObject::cons(
            LispObject::symbol("quote"),
            LispObject::cons(
                LispObject::cons(
                    LispObject::integer(2),
                    LispObject::cons(LispObject::integer(3), LispObject::nil()),
                ),
                LispObject::nil(),
            ),
        );
        assert_eq!(r, expected);
    }

    #[test]
    fn test_read_comments() {
        assert_eq!(
            read("42 ; this is a comment").unwrap(),
            LispObject::integer(42)
        );
        assert_eq!(
            read_all("; comment\n42").unwrap(),
            vec![LispObject::integer(42)]
        );
        assert_eq!(
            read_all("; first\n; second\n42").unwrap(),
            vec![LispObject::integer(42)]
        );
        assert_eq!(
            read("(a ; comment\n b)").unwrap(),
            LispObject::cons(
                LispObject::symbol("a"),
                LispObject::cons(LispObject::symbol("b"), LispObject::nil()),
            )
        );
        assert!(read_all("; just a comment").unwrap().is_empty());
    }

    #[test]
    fn test_read_nested_backquote() {
        // `(a ,b ,@c)
        let result = read("`(a ,b ,@c)").unwrap();
        // Should be (\` (a (\, b) (\,@ c)))
        assert!(result.is_cons());
        assert_eq!(result.first().unwrap(), LispObject::symbol("\\`"));
    }

    #[test]
    fn test_read_vector_literal() {
        let result = read("[1 2 3]").unwrap();
        // Currently represented as (vector 1 2 3)
        assert_eq!(result.first().unwrap(), LispObject::symbol("vector"));
    }

    #[test]
    fn test_read_real_elisp() {
        let source = r#"
;; Test file for reader
(defun my-func (x y &optional z)
  "A docstring."
  (let ((a (+ x y))
        (b (* x 2)))
    (if (> a b)
        (message "a > b: %d" a)
      (message "b >= a: %d" b))))

;; Backquote usage
(defmacro my-when (cond &rest body)
  `(if ,cond (progn ,@body)))

;; Character literals
(defvar my-char ?a)
(defvar my-newline ?\n)

;; Dotted pairs
(setq my-alist '((a . 1) (b . 2) (c . 3)))

;; Hex numbers
(setq my-hex #xff)

;; Float
(setq my-float 3.14)
(setq my-sci 1.5e-3)

;; Function reference
(mapcar #'1+ '(1 2 3))
"#;
        let forms = read_all(source).unwrap();
        assert_eq!(forms.len(), 9); // defun, defmacro, 2x defvar, 3x setq, mapcar
    }

    #[test]
    fn test_parse_debug_early_el() {
        let source = std::fs::read_to_string("/tmp/elisp-stdlib/debug-early.el");
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse debug-early.el");
            assert!(
                forms.len() >= 5,
                "expected at least 5 forms, got {}",
                forms.len()
            );
        }
    }

    #[test]
    fn test_parse_byte_run_el() {
        let source = std::fs::read_to_string("/tmp/elisp-stdlib/byte-run.el");
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse byte-run.el");
            assert!(
                forms.len() >= 10,
                "expected at least 10 forms, got {}",
                forms.len()
            );
        }
    }

    #[test]
    fn test_parse_backquote_el() {
        let source = std::fs::read_to_string("/tmp/elisp-stdlib/backquote.el");
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse backquote.el");
            assert!(
                forms.len() >= 5,
                "expected at least 5 forms, got {}",
                forms.len()
            );
        }
    }

    #[test]
    fn test_parse_subr_el() {
        let source = std::fs::read_to_string("/tmp/elisp-stdlib/subr.el");
        if let Ok(source) = source {
            let forms = read_all(&source).expect("failed to parse subr.el");
            assert!(
                forms.len() >= 100,
                "expected at least 100 forms, got {}",
                forms.len()
            );
        }
    }
}
