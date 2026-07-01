//! Shared source-line tokenizer used by the scanner lanes.
//!
//! Each scanner needs the same primitive: walk a line of Rust source,
//! drop block/line comments, and replace string literals with spaces
//! so token searches don't fire on the *content* of strings. The
//! [`SourceLine::parse`] function does exactly that and remembers
//! whether a `/* … */` block comment is still open across lines (the
//! caller threads the `&mut bool` through the loop).
//!
//! Kept in the library crate so the panic-surface, forbidden-scan, and
//! future scan-style lanes share one well-tested lexer. See
//! `bin/forbidden_scan/lane.rs` and `bin/check_panic_surface.rs` for
//! the consumers.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]
#![deny(clippy::unimplemented)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::string_slice)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::arithmetic_side_effects)]
#![deny(clippy::dbg_macro)]
#![deny(clippy::as_conversions)]
#![forbid(unsafe_code)]

use std::{iter::Peekable, str::Chars};

/// A source line after stripping comments and string contents.
///
/// `Code` carries the surviving runes (with string contents replaced
/// by spaces so the byte count and column positions are preserved).
/// `NonCode` means the whole line was a comment or string; the
/// scanner can skip it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceLine {
    Code(String),
    NonCode,
}

impl SourceLine {
    /// Tokenize one line. `block_comment` is the carry-over flag from
    /// the previous line: `true` if we are inside a `/* … */` block
    /// that has not yet been closed. The function updates it in place.
    #[must_use]
    pub fn parse(raw: &str, block_comment: &mut bool) -> Self {
        let parser = SourceLineParser::new(raw, *block_comment);
        let parsed = parser.parse();
        *block_comment = parsed.block_comment;
        parsed.line
    }

    /// Fast path: classify a line without invoking the full parser if it
    /// contains no comment markers or string delimiters. The line is
    /// then guaranteed to be pure code (or whitespace), so the parser
    /// is skipped entirely and the code is just cloned as-is.
    #[must_use]
    pub fn parse_simple(raw: &str) -> Self {
        if raw.bytes().any(|b| matches!(b, b'/' | b'"' | b'\\')) {
            Self::parse(raw, &mut false)
        } else if raw.trim().is_empty() {
            Self::NonCode
        } else {
            Self::Code(raw.to_owned())
        }
    }
    /// True if the line was entirely comments or string contents.
    #[must_use]
    pub fn is_non_code(&self) -> bool {
        matches!(self, Self::NonCode)
    }

    /// The surviving code bytes. Returns an empty slice for `NonCode`.
    #[must_use]
    pub fn code(&self) -> &str {
        match self {
            Self::Code(code) => code.as_str(),
            Self::NonCode => "",
        }
    }
}

struct ParsedLine {
    line: SourceLine,
    block_comment: bool,
}

struct SourceLineParser<'a> {
    chars: Peekable<Chars<'a>>,
    code: String,
    block_comment: bool,
    in_string: bool,
    escaped: bool,
}

impl<'a> SourceLineParser<'a> {
    fn new(raw: &'a str, block_comment: bool) -> Self {
        Self {
            chars: raw.chars().peekable(),
            code: String::with_capacity(raw.len()),
            block_comment,
            in_string: false,
            escaped: false,
        }
    }

    fn parse(mut self) -> ParsedLine {
        while let Some(ch) = self.chars.next() {
            if self.consume_block_comment(ch) || self.consume_string(ch) {
                continue;
            }
            if self.starts_line_comment(ch) {
                break;
            }
            if self.starts_block_comment(ch) {
                continue;
            }
            self.consume_code(ch);
        }
        self.finish()
    }

    fn consume_block_comment(&mut self, ch: char) -> bool {
        if !self.block_comment {
            return false;
        }
        if ch == '*' && self.chars.peek().is_some_and(|next| *next == '/') {
            let _slash = self.chars.next();
            self.block_comment = false;
        }
        true
    }

    fn consume_string(&mut self, ch: char) -> bool {
        if !self.in_string {
            return false;
        }
        if self.escaped {
            self.escaped = false;
        } else if ch == '\\' {
            self.escaped = true;
        } else if ch == '"' {
            self.in_string = false;
        }
        self.code.push(' ');
        true
    }

    fn starts_line_comment(&mut self, ch: char) -> bool {
        ch == '/' && self.chars.peek().is_some_and(|next| *next == '/')
    }

    fn starts_block_comment(&mut self, ch: char) -> bool {
        if ch != '/' || !self.chars.peek().is_some_and(|next| *next == '*') {
            return false;
        }
        let _star = self.chars.next();
        self.block_comment = true;
        true
    }

    fn consume_code(&mut self, ch: char) {
        if ch == '"' {
            self.in_string = true;
            self.code.push(' ');
        } else {
            self.code.push(ch);
        }
    }

    fn finish(self) -> ParsedLine {
        let line = if self.code.trim().is_empty() {
            SourceLine::NonCode
        } else {
            SourceLine::Code(self.code)
        };
        ParsedLine { line, block_comment: self.block_comment }
    }
}

#[cfg(test)]
mod tests {
    use super::SourceLine;

    fn parse_lines(text: &str) -> Vec<SourceLine> {
        let mut block_comment = false;
        text.lines().map(|line| SourceLine::parse(line, &mut block_comment)).collect()
    }

    #[test]
    fn line_comment_is_skipped() {
        let lines = parse_lines("// hello\nlet x = 1;");
        assert!(lines[0].is_non_code());
        assert_eq!(lines[1].code(), "let x = 1;");
    }

    #[test]
    fn block_comment_within_one_line_is_skipped() {
        let lines = parse_lines("/* foo */ let x = 1;");
        // The whole line collapses to whitespace when only the
        // comment was real code, but `is_non_code` here returns
        // false because the line still has visible code.
        let line = &lines[0];
        // `code()` returns the surviving runes with the comment
        // replaced by spaces.
        let code = line.code();
        assert!(!code.contains("foo"));
        assert!(code.contains("let"));
    }

    #[test]
    fn block_comment_spans_multiple_lines() {
        let mut block_comment = false;
        let line1 = SourceLine::parse("/* spans", &mut block_comment);
        assert!(block_comment, "block_comment should remain open");
        let line2 = SourceLine::parse("more lines */ let x = 1;", &mut block_comment);
        assert!(!block_comment, "block_comment should be closed");
        assert!(line2.code().contains("let x = 1;"));
        let _ = line1;
    }

    #[test]
    fn string_literal_contents_are_blanked_out() {
        let lines = parse_lines("let s = \"assert!\";");
        let code = lines[0].code();
        assert!(!code.contains("assert!"));
        assert!(code.contains("let s = "));
    }

    #[test]
    fn escaped_quote_in_string_does_not_close() {
        let lines = parse_lines(r#"let s = "a\"b";"#);
        let code = lines[0].code();
        // The closing `"` after `b` ends the string; the literal
        // contents between the quotes are blanked but the
        // surrounding code survives.
        assert!(code.starts_with("let s = "));
        assert!(code.ends_with(";"));
        assert!(!code.contains(r#"a\"b"#));
    }

    #[test]
    fn preserves_columns_across_string() {
        let raw = r#"let _ = panic!("nope");"#;
        let parsed = SourceLine::parse(raw, &mut false);
        match parsed {
            SourceLine::Code(s) => assert_eq!(s.len(), raw.len(), "column count must be preserved"),
            SourceLine::NonCode => panic!("expected Code"),
        }
    }

    #[test]
    fn parse_simple_no_special_chars_returns_code() {
        let raw = "let x = 1;";
        let parsed = SourceLine::parse_simple(raw);
        match parsed {
            SourceLine::Code(s) => assert_eq!(s, raw),
            SourceLine::NonCode => panic!("expected Code, got NonCode"),
        }
    }

    #[test]
    fn parse_simple_whitespace_returns_non_code() {
        let parsed = SourceLine::parse_simple("   \t  ");
        assert!(matches!(parsed, SourceLine::NonCode));
    }

    #[test]
    fn parse_simple_with_slash_falls_back_to_full_parse() {
        // Line comment triggers the fallback branch (any '/' in the line).
        let parsed = SourceLine::parse_simple("// pure comment");
        assert!(parsed.is_non_code());
    }

    #[test]
    fn parse_simple_with_string_falls_back_to_full_parse() {
        let parsed = SourceLine::parse_simple(r#"let s = "hi";"#);
        match parsed {
            SourceLine::Code(s) => assert!(s.contains("let s = ")),
            SourceLine::NonCode => panic!("expected Code, got NonCode"),
        }
    }
}
