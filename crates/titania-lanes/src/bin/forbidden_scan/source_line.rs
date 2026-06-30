use std::{iter::Peekable, str::Chars};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SourceLine {
    Code(String),
    NonCode,
}

impl SourceLine {
    pub(super) fn parse(raw: &str, block_comment: &mut bool) -> Self {
        let parser = SourceLineParser::new(raw, *block_comment);
        let parsed = parser.parse();
        *block_comment = parsed.block_comment;
        parsed.line
    }

    pub(super) fn is_non_code(&self) -> bool {
        matches!(self, Self::NonCode)
    }

    pub(super) fn code(&self) -> &str {
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
