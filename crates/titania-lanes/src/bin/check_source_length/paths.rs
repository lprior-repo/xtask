use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use titania_lanes::helpers::{relative_path, walk_rs_files};

pub(crate) fn is_excluded_source_path(file: &str) -> bool {
    is_bad_prefix(file) || is_bad_segment(file) || is_bad_leaf(file)
}

pub(crate) fn is_test_like_source_path(file: &str) -> bool {
    file.ends_with("/tests.rs")
        || file.ends_with("_tests.rs")
        || file.contains("/tests/")
        || file.starts_with("tests/")
        || file.contains("/kani/")
        || file.starts_with("kani_")
        || file.contains("kani_")
        || file.contains("/verification/")
        || file.starts_with("verification/")
        || file.contains("/proptest")
        || file.contains("/benches/")
}

pub(crate) fn is_titania_hot_source(root: &Path, file: &Path) -> bool {
    let rel = relative_path(root, file);
    !is_test_like_source_path(&rel)
        && (rel.starts_with("crates/titania-core/src/")
            || rel.starts_with("crates/titania-lanes/src/"))
}

pub(crate) fn tracked_set(root: &Path) -> HashSet<String> {
    match tracked_rust_files(root) {
        Some(files) => files.iter().map(|p| relative_path(root, p)).collect(),
        None => HashSet::new(),
    }
}

pub(crate) fn tracked_rust_files(root: &Path) -> Option<Vec<PathBuf>> {
    let crates_dir = root.join("crates");
    if !crates_dir.is_dir() {
        return None;
    }
    let mut out = Vec::new();
    walk_core_lanes(root, &crates_dir, &mut out);
    walk_vb_crates(root, &crates_dir, &mut out)?;
    Some(out)
}

fn walk_core_lanes(root: &Path, crates_dir: &Path, out: &mut Vec<PathBuf>) {
    walk_rs_files(&crates_dir.join("titania-core/src"), root, out);
    walk_rs_files(&crates_dir.join("titania-lanes/src"), root, out);
}

fn walk_vb_crates(root: &Path, crates_dir: &Path, out: &mut Vec<PathBuf>) -> Option<()> {
    std::fs::read_dir(crates_dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_scanned_crate(path))
        .map(|path| path.join("src"))
        .filter(|src| src.is_dir())
        .for_each(|src| walk_rs_files(&src, root, out));
    Some(())
}

fn is_scanned_crate(path: &Path) -> bool {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.starts_with("vb_") || name == "vb_cli",
        None => false,
    }
}

fn is_bad_prefix(file: &str) -> bool {
    [
        "target/",
        ".jj/",
        ".beads/",
        ".evidence/",
        ".cargo_temp/",
        "cargo-home/",
        "cargo_home/",
        ".cargo/registry/",
    ]
    .iter()
    .any(|prefix| file.starts_with(prefix))
}

fn is_bad_segment(file: &str) -> bool {
    [
        "/target/",
        "/.jj/",
        "/.beads/",
        "/.evidence/",
        "/.cargo_temp/",
        "/cargo-home/",
        "/cargo_home/",
        "/.cargo/registry/",
    ]
    .iter()
    .any(|segment| file.contains(segment))
}

fn is_bad_leaf(file: &str) -> bool {
    matches!(file, "target" | ".jj" | ".beads" | ".evidence" | ".cargo_temp")
}
