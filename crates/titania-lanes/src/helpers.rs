//! Shared helpers for the titania-lanes bin implementations.

#![allow(clippy::implicit_saturating_sub)]
#![allow(clippy::only_used_in_recursion)]
#![allow(clippy::manual_unwrap_or_default)]
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

use std::path::{Path, PathBuf};

#[must_use]
pub fn line_no_from_idx(idx: usize) -> u32 {
    let Ok(n) = u32::try_from(idx) else {
        return 0;
    };
    n.checked_add(1).map_or(0, |v| v)
}

#[must_use]
pub fn saturating_add_usize(a: usize, b: usize) -> usize {
    a.checked_add(b).map_or(usize::MAX, |v| v)
}

#[must_use]
pub fn brace_delta(text: &str) -> i32 {
    let mut delta: i32 = 0;
    for ch in text.chars() {
        if ch == '{' {
            delta = delta.saturating_add(1);
        } else if ch == '}' {
            delta = delta.saturating_sub(1);
        }
    }
    delta
}

#[must_use]
pub fn strip_leading_whitespace(s: &str) -> &str {
    s.trim_start()
}

#[must_use]
pub fn strip_whitespace(s: &str) -> &str {
    s.trim()
}

#[must_use]
pub fn normalize_slashes(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[must_use]
pub fn relative_path(root: &Path, p: &Path) -> String {
    match p.strip_prefix(root) {
        Ok(r) => normalize_slashes(r),
        Err(_) => normalize_slashes(p),
    }
}

pub fn walk_rs_files(dir: &Path, _root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, _root, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LineNo(pub u32);

impl LineNo {
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[must_use]
pub fn line_diff(start: usize, end: usize) -> usize {
    end.saturating_sub(start)
}

pub fn for_each_byte<F: FnMut(u8) -> bool>(text: &str, mut f: F) {
    let bytes = text.as_bytes();
    let mut i: usize = 0;
    while let Some(b) = bytes.get(i).copied() {
        if !f(b) {
            return;
        }
        i = i.saturating_add(1);
    }
}
