use std::{iter::Peekable, str::Chars};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LineKind {
    NonCode,
    Signature,
    Expression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceLine {
    code: String,
    kind: LineKind,
}

impl SourceLine {
    pub(super) fn parse(raw: &str, block_comment: &mut bool) -> Self {
        let code = strip_non_code(raw, block_comment);
        let trimmed = code.trim().to_owned();
        let kind = classify_kind(&trimmed);
        Self { code: trimmed, kind }
    }

    pub(super) fn code(&self) -> &str {
        self.code.as_str()
    }

    pub(super) fn is_signature(&self) -> bool {
        self.kind == LineKind::Signature
    }

    pub(super) fn is_code_expression(&self) -> bool {
        self.kind == LineKind::Expression
    }
}

fn classify_kind(trimmed: &str) -> LineKind {
    if trimmed.is_empty() {
        LineKind::NonCode
    } else if is_signature_line(trimmed) {
        LineKind::Signature
    } else {
        LineKind::Expression
    }
}

fn is_signature_line(trimmed: &str) -> bool {
    let looks_like_fn = [
        "fn ",
        "pub fn ",
        "pub(crate) fn ",
        "pub(super) fn ",
        "async fn ",
        "pub async fn ",
        "const fn ",
        "pub const fn ",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix));
    looks_like_fn && trimmed.contains('(')
}

#[derive(Clone, Copy)]
struct StripState {
    block_comment: bool,
    in_string: bool,
    escaped: bool,
}

impl StripState {
    const fn new(block_comment: bool) -> Self {
        Self { block_comment, in_string: false, escaped: false }
    }

    fn consume_block_comment(&mut self, ch: char, chars: &mut Peekable<Chars<'_>>) -> bool {
        if !self.block_comment {
            return false;
        }
        if ch == '*' && chars.peek().is_some_and(|next| *next == '/') {
            let _slash = chars.next();
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

    fn start_block_comment_or_string(&mut self, ch: char, chars: &mut Peekable<Chars<'_>>) -> bool {
        if ch == '/' && chars.peek().is_some_and(|next| *next == '*') {
            let _star = chars.next();
            self.block_comment = true;
            return true;
        }
        if ch == '"' {
            self.in_string = true;
            return true;
        }
        false
    }
}

fn begins_line_comment(ch: char, chars: &mut Peekable<Chars<'_>>) -> bool {
    ch == '/' && chars.peek().is_some_and(|next| *next == '/')
}

fn strip_non_code(raw: &str, block_comment: &mut bool) -> String {
    let mut code = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    let mut state = StripState::new(*block_comment);
    while let Some(ch) = chars.next() {
        if state.consume_block_comment(ch, &mut chars) || state.consume_string(ch) {
            continue;
        }
        if begins_line_comment(ch, &mut chars) {
            break;
        }
        if state.start_block_comment_or_string(ch, &mut chars) {
            code.push(' ');
            continue;
        }
        code.push(ch);
    }
    *block_comment = state.block_comment;
    code
}
