use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn walk_rs_lines<F: FnMut(&str, &str, u32)>(
    root: &Path,
    display_root: &Path,
    mut visit: F,
) {
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        entries.filter_map(Result::ok).for_each(|entry| {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                visit_file(&path, display_root, &mut visit);
            }
        });
    }
}

fn visit_file<F: FnMut(&str, &str, u32)>(path: &Path, display_root: &Path, visit: &mut F) {
    let Ok(text) = fs::read_to_string(path) else { return };
    let rel_path = match path.strip_prefix(display_root) {
        Ok(relative) => relative,
        Err(_) => path,
    };
    let rel = rel_path.to_string_lossy().into_owned();
    let mut line_no = 0_u32;
    text.lines().for_each(|line| {
        line_no = line_no.saturating_add(1);
        visit(line, &rel, line_no);
    });
}
