mod checks;
mod toml_scan;

use std::path::Path;

use titania_lanes::{Finding, LaneExit, LaneReport, exit};

const RULE_INVALID_INVOCATION: &str = "WS-INVOCATION-001";
const RULE_MEMBERS: &str = "WS-MEMBERS-001";
const RULE_CRATE_NAME: &str = "WS-CRATE-NAME-001";
const RULE_FORBIDDEN_FEATURE: &str = "WS-FORBIDDEN-FEATURE-001";
const RULE_FORBIDDEN_DEP: &str = "WS-FORBIDDEN-DEP-001";
const RULE_GENERATED_BOUNDARY: &str = "WS-GENERATED-BOUNDARY-001";
const RULE_UNREADABLE: &str = "WS-UNREADABLE-001";

const FORBIDDEN_FEATURE_NAMES: &[&str] =
    &["json", "serde-json", "generated", "maxperf", "velvet-ballistics", "velvet_ballistics"];

pub(crate) fn main_exit() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        eprintln!(
            "usage: check-workspace-assertions\n\
             Validates the Cargo workspace shape (members, package names,\n\
             forbidden features, generated-boundary files). Run from the\n\
             repository root."
        );
        return exit(LaneExit::Usage);
    }

    let root = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("InvalidInvocation: cannot read current directory: {error}");
            return exit(LaneExit::Usage);
        }
    };
    exit(run(&root))
}

fn run(root: &Path) -> LaneExit {
    if !root.join("Cargo.toml").exists() || !root.join("crates").exists() {
        let mut report = LaneReport::new();
        report.push(Finding::new(
            RULE_INVALID_INVOCATION,
            "Cargo.toml",
            0,
            "InvalidInvocation: run from repository root",
        ));
        eprint!("{}", report.render());
        return LaneExit::Usage;
    }

    let mut report = LaneReport::new();
    let members = checks::discover_members(root);
    checks::check_workspace_members(root, &mut report);
    checks::check_crate_names(root, &members, &mut report);
    checks::check_forbidden_dependencies(root, &members, &mut report);
    checks::check_generated_boundaries(root, &mut report);

    eprint!("{}", report.render());
    if report.is_clean() {
        eprintln!("workspace assertions: PASS");
        LaneExit::Clean
    } else {
        LaneExit::Violations
    }
}

#[cfg(test)]
mod tests {
    use super::FORBIDDEN_FEATURE_NAMES;

    #[test]
    fn forbidden_features_does_not_contain_cargo_or_unrelated() {
        assert!(!FORBIDDEN_FEATURE_NAMES.contains(&"serde"));
        assert!(FORBIDDEN_FEATURE_NAMES.contains(&"json"));
    }
}
