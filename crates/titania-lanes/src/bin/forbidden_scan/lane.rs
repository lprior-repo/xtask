use aho_corasick::{AhoCorasick, MatchKind};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use titania_lanes::{
    Finding, LaneExit, LaneReport, SourceLine, current_target_project, exit,
    helpers::{line_no_from_idx, relative_path},
};

/// Default forbidden tokens (Holzman Rust slice 1).
///
/// Tokens are stored as their canonical surface (`panic!`, `unwrap`,
/// `expect`, `todo!`, `unimplemented!`, `dbg!`). Macro tokens match as
/// raw substrings because the `!` is part of the macro syntax. Method
/// tokens (`unwrap`, `expect`) match only when preceded by a method
/// receiver (`.` or `::`) so we do not false-positive on identifiers
/// like `myexpect`.
const DEFAULT_FORBIDDEN: &[&str] =
    &["panic!", "unwrap", "expect", "todo!", "unimplemented!", "dbg!"];
const FORBIDDEN_FLAG: &str = "--forbidden=";
pub(crate) fn main_exit(args: Vec<String>) -> ExitCode {
    let forbidden = match parse_forbidden(&args) {
        Ok(set) => set,
        Err(message) => {
            eprintln!("[forbidden-scan] {message}");
            return exit(LaneExit::Usage);
        }
    };
    let root = match target_root() {
        Ok(root) => root,
        Err(code) => return code,
    };
    let ac = match build_forbidden_ac(&forbidden) {
        Ok(ac) => ac,
        Err(message) => {
            eprintln!("[forbidden-scan] {message}");
            return exit(LaneExit::Usage);
        }
    };
    emit_scan_header(&root, &forbidden);
    scan_and_exit(&root, &forbidden, &ac)
}

fn target_root() -> Result<PathBuf, ExitCode> {
    current_target_project().map(|target| target.as_std_path().to_path_buf()).map_err(|error| {
        eprintln!("[forbidden-scan] cannot resolve target project: {error}");
        exit(LaneExit::Usage)
    })
}

fn emit_scan_header(root: &Path, forbidden: &[ForbiddenToken]) {
    eprintln!("CWD: {}", root.display());
    eprintln!("ScanDomain: crates/*/src");
    eprintln!(
        "ForbiddenTokens: {}",
        forbidden.iter().map(ForbiddenToken::as_str).collect::<Vec<_>>().join(",")
    );
}
fn scan_and_exit(root: &Path, forbidden: &[ForbiddenToken], ac: &AhoCorasick) -> ExitCode {
    let mut report = LaneReport::new();
    titania_lanes::walk::production_rust_files_par(root)
        .iter()
        .for_each(|file| scan_file(root, file, forbidden, ac, &mut report));
    eprint!("{}", report.render());
    if report.is_clean() { clean_exit() } else { violations_exit() }
}

fn clean_exit() -> ExitCode {
    eprintln!("NoViolationFound");
    exit(LaneExit::Clean)
}

fn violations_exit() -> ExitCode {
    eprintln!("ViolationFound: forbidden token surface is non-empty");
    exit(LaneExit::Violations)
}

fn parse_forbidden(args: &[String]) -> Result<Vec<ForbiddenToken>, String> {
    let override_set = args
        .iter()
        .find(|arg| arg.starts_with(FORBIDDEN_FLAG))
        .map(|arg| parse_override_set(arg.as_str()));
    Ok(match override_set {
        Some(set) if !set.is_empty() => set,
        Some(_) | None => default_forbidden_set(),
    })
}

fn parse_override_set(arg: &str) -> Vec<ForbiddenToken> {
    let body = arg.strip_prefix(FORBIDDEN_FLAG).map_or("", core::convert::identity);
    body.split(',').filter_map(ForbiddenToken::parse).collect()
}

fn default_forbidden_set() -> Vec<ForbiddenToken> {
    DEFAULT_FORBIDDEN.iter().filter_map(|s| ForbiddenToken::parse(s)).collect()
}

/// Build the Aho-Corasick automaton from the forbidden token surface.
/// Construction is infallible in practice (the patterns are all static
/// literals), but the underlying builder returns a `Result`; we map any
/// builder failure into a typed diagnostic for the caller. An empty
/// forbidden set also returns an empty-pattern error which we propagate
/// the same way.
fn build_forbidden_ac(forbidden: &[ForbiddenToken]) -> Result<AhoCorasick, String> {
    let names: Vec<&str> = forbidden.iter().map(ForbiddenToken::as_str).collect();
    AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(&names)
        .map_err(|error| format!("forbidden AC build failed for {} patterns: {error}", names.len()))
}

fn scan_file(
    root: &Path,
    path: &Path,
    forbidden: &[ForbiddenToken],
    ac: &AhoCorasick,
    report: &mut LaneReport,
) {
    report.record_scan();
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };
    let display = relative_path(root, path);
    scan_content(&content, &display, forbidden, ac, report);
}

fn scan_content(
    content: &str,
    display: &str,
    forbidden: &[ForbiddenToken],
    ac: &AhoCorasick,
    report: &mut LaneReport,
) {
    // File-level prefilter: if Aho-Corasick finds no matches anywhere in
    // the file, skip the per-line parse+match work entirely. This is
    // the dominant wall-time cost when most files contain zero
    // forbidden tokens (the common case for non-self-test sources).
    if ac.find_iter(content).next().is_none() {
        return;
    }
    let mut block_comment = false;
    content.lines().enumerate().for_each(|(idx, line)| {
        // Per-line gate: skip SourceLine::parse on lines that can't
        // possibly contain any forbidden token.
        if !ac.is_match(line) {
            return;
        }
        let source_line = SourceLine::parse(line, &mut block_comment);
        scan_source_line(&source_line, idx, display, forbidden, report);
    });
}

fn scan_source_line(
    line: &SourceLine,
    idx: usize,
    display: &str,
    forbidden: &[ForbiddenToken],
    report: &mut LaneReport,
) {
    if line.is_non_code() {
        return;
    }
    let line_no = line_no_from_idx(idx);
    forbidden.iter().filter(|token| token.is_present_in(line.code())).for_each(|token| {
        report.push(Finding::new(
            "FORBIDDEN-001",
            display,
            line_no,
            format!("forbidden token `{}`", token.as_str()),
        ));
    });
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ForbiddenToken {
    name: String,
    kind: TokenKind,
}

/// What shape of Rust construct the forbidden surface is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    /// Macro invocation, e.g. `panic!(...)` — matched as a raw
    /// substring because the `!` is part of the surface.
    Macro,
    /// Method call, e.g. `x.unwrap()` — matched only when preceded by
    /// a method-call receiver (`.` or `::`) and followed by `(`. This
    /// prevents false positives on identifiers like `myexpect` or
    /// `myexpect()`.
    Method,
}

impl ForbiddenToken {
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let kind = if trimmed.ends_with('!') { TokenKind::Macro } else { TokenKind::Method };
        Some(Self { name: trimmed.to_owned(), kind })
    }

    fn as_str(&self) -> &str {
        &self.name
    }

    fn is_present_in(&self, code: &str) -> bool {
        let mut search_start = 0usize;
        while let Some(idx) =
            code.get(search_start..).and_then(|tail| tail.find(self.name.as_str()))
        {
            let abs_idx = search_start.saturating_add(idx);
            if self.matches_at(code, abs_idx) {
                return true;
            }
            // Advance past the match to find the next occurrence.
            search_start = abs_idx.saturating_add(1);
        }
        false
    }

    /// Decide whether the match at `idx` is a real surface occurrence
    /// (per [`TokenKind`]) rather than a substring of a larger
    /// identifier.
    fn matches_at(&self, code: &str, idx: usize) -> bool {
        let bytes = code.as_bytes();
        let name_bytes = self.name.as_bytes();
        let name_len = name_bytes.len();
        let after = idx.saturating_add(name_len);
        match self.kind {
            TokenKind::Macro => {
                // Reject identifier-prefix matches: the byte before
                // the match (if any) must not be alphanumeric/underscore.
                idx == 0 || bytes.get(idx.wrapping_sub(1)).is_none_or(|b| !is_word_byte(*b))
            }
            TokenKind::Method => {
                // Require a method-call receiver directly before:
                // `.unwrap` or `::unwrap` (e.g. `Result::unwrap(...)`).
                // Reject identifier-prefix matches so `myexpect` is not
                // flagged.
                let before_ok = match bytes.get(idx.wrapping_sub(1)) {
                    Some(b'.' | b':') => true,
                    Some(b) if is_word_byte(*b) => return false,
                    _ => idx == 0,
                };
                let after_ok = bytes.get(after).is_some_and(|b| *b == b'(');
                before_ok && after_ok
            }
        }
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::ForbiddenToken;
    fn macro_token(name: &str) -> ForbiddenToken {
        ForbiddenToken::parse(name).expect("parse")
    }

    #[test]
    fn macro_token_matches_panic_bang() {
        let token = macro_token("panic!");
        assert!(token.is_present_in("panic!(\"boom\")"));
        assert!(token.is_present_in("let _ = panic!();"));
    }

    #[test]
    fn macro_token_rejects_identifier_prefixed_match() {
        // `mypanic!` must not be flagged as `panic!`.
        let token = macro_token("panic!");
        assert!(!token.is_present_in("mypanic!()"));
    }

    #[test]
    fn method_token_matches_dot_receiver() {
        let token = macro_token("unwrap");
        assert!(token.is_present_in("x.unwrap()"));
        // `unwrap_or_default` is a different method, not `unwrap`.
        assert!(!token.is_present_in("x.unwrap_or_default()"));
    }

    #[test]
    fn method_token_matches_double_colon_receiver() {
        let token = macro_token("unwrap");
        assert!(token.is_present_in("Result::unwrap(r)"));
    }

    #[test]
    fn method_token_matches_expect_with_message() {
        // Regression: the old plain-substring matcher missed this
        // because `.expect("msg")` never contains the literal
        // `expect()` token.
        let token = macro_token("expect");
        assert!(token.is_present_in("fs::read_to_string(\"/tmp/x\").expect(\"boom\")"));
    }

    #[test]
    fn method_token_rejects_identifier_prefixed_match() {
        // `myexpect()` must not be flagged as the `expect` method.
        let token = macro_token("expect");
        assert!(!token.is_present_in("myexpect()"));
        assert!(!token.is_present_in("myexpect"));
    }

    #[test]
    fn method_token_requires_open_paren() {
        let token = macro_token("unwrap");
        // No `(` after the name means it's just an identifier in scope.
        assert!(!token.is_present_in("let unwrap = 1;"));
        assert!(!token.is_present_in("x.unwrap"));
    }

    #[test]
    fn empty_token_string_is_rejected() {
        assert!(ForbiddenToken::parse("").is_none());
        assert!(ForbiddenToken::parse("   ").is_none());
    }

    #[test]
    fn dbg_macro_token_matches_bang_form() {
        let token = macro_token("dbg!");
        assert!(token.is_present_in("dbg!(x)"));
    }
}
