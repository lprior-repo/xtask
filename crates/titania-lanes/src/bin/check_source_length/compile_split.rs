use std::path::Path;

use titania_lanes::{Finding, LaneReport, helpers::relative_path};

const SPLIT_MODULES: &[&str] = &[
    "mod_compile_core.rs",
    "mod_compile_errors.rs",
    "mod_compile_validation.rs",
    "mod_compile_lowering.rs",
];

pub(crate) fn check_compile_split_sources(root: &Path, report: &mut LaneReport) {
    let compile_dir = root.join("crates/vb_compile/src");
    if !compile_dir.is_dir() {
        eprintln!("NotApplicable: legacy compile split directory absent");
        return;
    }
    check_impl_body(root, &compile_dir, report);
    SPLIT_MODULES.iter().for_each(|name| check_split_module(root, &compile_dir, name, report));
}

fn check_impl_body(root: &Path, compile_dir: &Path, report: &mut LaneReport) {
    let impl_body = compile_dir.join("compile_core_impl.rs");
    if impl_body.is_file() {
        report.push(Finding::new(
            "COMPILE-SPLIT",
            relative_path(root, &impl_body),
            0,
            "hidden production include body must not remain",
        ));
    }
}

fn check_split_module(root: &Path, compile_dir: &Path, name: &str, report: &mut LaneReport) {
    let path = compile_dir.join(name);
    if !path.is_file() {
        push_missing_module(name, report);
        return;
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    check_module_text(root, &path, &text, report);
}

fn check_module_text(root: &Path, path: &Path, text: &str, report: &mut LaneReport) {
    if text.contains("include!(") {
        push_compile_split(root, path, "contains monolithic include body", report);
    }
    if is_doc_only_shell(text) {
        push_compile_split(
            root,
            path,
            "doc-only shell, not an owned implementation module",
            report,
        );
    }
}

fn is_doc_only_shell(text: &str) -> bool {
    let line_count = text.lines().count();
    let has_mod = text.lines().any(|line| line.trim_start().starts_with("mod "));
    line_count < 50 && !has_mod
}

fn push_compile_split(root: &Path, path: &Path, message: &str, report: &mut LaneReport) {
    report.push(Finding::new("COMPILE-SPLIT", relative_path(root, path), 0, message));
}

fn push_missing_module(name: &str, report: &mut LaneReport) {
    report.push(Finding::new(
        "COMPILE-SPLIT",
        format!("crates/vb_compile/src/{name}"),
        0,
        "missing from compile split",
    ));
}
