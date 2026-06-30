//! Enforces per-function logical line cap + tracked source length limit with ledger.
//!
//! Rust re-implementation of the bash lane in
//! `velvet-ballistics/scripts/check-source-length.sh`. Run via
//! `cargo run --bin check-source-length --` from the repository root or via the
//! matching Moon task in `.moon/tasks/all.yml`.
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![forbid(unsafe_code)]

use std::path::Path;

use titania_lanes::{LaneExit, LaneReport, current_target_project, exit};

#[path = "check_source_length/compile_split.rs"]
mod compile_split;
#[path = "check_source_length/function_scan.rs"]
mod function_scan;
#[path = "check_source_length/ledger.rs"]
mod ledger;
#[path = "check_source_length/mutants.rs"]
mod mutants;
#[path = "check_source_length/paths.rs"]
mod paths;
#[path = "check_source_length/source_limit.rs"]
mod source_limit;

const FN_LINE_LIMIT: usize = 25;
const SOURCE_LINE_LIMIT: usize = 300;
const LEDGER_PATH: &str = ".config/source-length-exceptions.txt";

fn main() -> std::process::ExitCode {
    let target = match current_target_project() {
        Ok(target) => target,
        Err(error) => {
            eprintln!("[check-source-length] cannot resolve target project: {error}");
            return exit(LaneExit::Usage);
        }
    };
    let mut report = LaneReport::new();
    run(target.as_std_path(), &mut report);
    print_and_exit(&report)
}

fn run(root: &Path, report: &mut LaneReport) {
    mutants::check_mutants_residue(root, report);
    compile_split::check_compile_split_sources(root, report);
    let tracked = paths::tracked_rust_files(root);
    let exceptions = ledger::load_ledger(root, report);
    if let Some(files) = tracked.as_deref() {
        source_limit::check_source_line_limit(root, files, &exceptions, report);
        check_hot_functions(root, files, report);
    }
}

fn check_hot_functions(root: &Path, files: &[std::path::PathBuf], report: &mut LaneReport) {
    files
        .iter()
        .filter(|file| paths::is_titania_hot_source(root, file))
        .for_each(|file| function_scan::check_file(root, file, report));
}

fn print_and_exit(report: &LaneReport) -> std::process::ExitCode {
    let rendered = report.render();
    if !rendered.is_empty() {
        eprint!("{rendered}");
    }
    if report.is_clean() { exit(LaneExit::Clean) } else { exit(LaneExit::Violations) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_file(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("test path has parent"))
            .expect("create parent dirs");
        std::fs::write(path, text).expect("write test file");
    }

    fn long_source(lines: usize) -> String {
        (0..lines).map(|idx| format!("pub const L{idx}: usize = {idx};\n")).collect()
    }

    fn long_function() -> String {
        let body: String = (0..26).map(|idx| format!("    let _v{idx} = {idx};\n")).collect();
        format!("fn oversized() {{\n{body}}}\n")
    }

    #[test]
    fn missing_source_length_ledger_keeps_line_limit_active() {
        let temp = tempfile::tempdir().expect("tempdir");
        fixture_file(
            temp.path(),
            "crates/titania-lanes/src/lib.rs",
            &long_source(SOURCE_LINE_LIMIT + 1),
        );

        let mut report = LaneReport::new();
        run(temp.path(), &mut report);

        assert!(report.findings().iter().any(|finding| {
            finding.rule() == "SRC-LINE-LIMIT"
                && finding.path() == "crates/titania-lanes/src/lib.rs"
        }));
    }

    #[test]
    fn src_bin_production_functions_are_scanned() {
        let temp = tempfile::tempdir().expect("tempdir");
        fixture_file(temp.path(), "crates/titania-lanes/src/bin/oversized.rs", &long_function());

        let mut report = LaneReport::new();
        run(temp.path(), &mut report);

        assert!(report.findings().iter().any(|finding| {
            finding.rule() == "FN-LINE-LIMIT"
                && finding.path() == "crates/titania-lanes/src/bin/oversized.rs"
        }));
    }
}
