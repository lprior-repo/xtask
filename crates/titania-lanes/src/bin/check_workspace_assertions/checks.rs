use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use titania_lanes::{Finding, LaneReport};

use super::{
    FORBIDDEN_FEATURE_NAMES, RULE_CRATE_NAME, RULE_FORBIDDEN_DEP, RULE_FORBIDDEN_FEATURE,
    RULE_GENERATED_BOUNDARY, RULE_MEMBERS, RULE_UNREADABLE,
    toml_scan::{binary_names, named_table_values, package_name, quoted_array_values},
};

fn expected_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(super) fn check_workspace_members(root: &Path, report: &mut LaneReport) {
    let cargo_path = root.join("Cargo.toml");
    let manifest = match fs::read_to_string(&cargo_path) {
        Ok(text) => text,
        Err(error) => {
            report.push(Finding::new(
                RULE_UNREADABLE,
                "Cargo.toml",
                0,
                format!("Cargo.toml: unreadable: {error}"),
            ));
            return;
        }
    };
    let actual = quoted_array_values(&manifest, "members");
    report.record_scan();
    if !actual.is_empty() {
        eprintln!("[check-workspace-assertions] workspace members: {actual:?}");
    } else {
        report.push(Finding::new(
            RULE_MEMBERS,
            "Cargo.toml",
            0,
            "Cargo.toml: workspace.members is empty or missing",
        ));
    }
}

pub(super) fn check_crate_names(root: &Path, members: &[String], report: &mut LaneReport) {
    members.iter().for_each(|member| check_crate_name(root, member, report));
}

fn check_crate_name(root: &Path, member: &str, report: &mut LaneReport) {
    let manifest_path = root.join(member).join("Cargo.toml");
    let manifest = match fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(error) => {
            report.push(Finding::new(
                RULE_UNREADABLE,
                format!("{member}/Cargo.toml"),
                0,
                format!("{member}/Cargo.toml: unreadable manifest: {error}"),
            ));
            return;
        }
    };
    report.record_scan();
    check_package_name(member, &manifest, report);
    report_lanes_bins(member, &manifest);
    check_forbidden_features(member, &manifest, report);
}

fn check_package_name(member: &str, manifest: &str, report: &mut LaneReport) {
    if package_name(manifest).is_none() {
        report.push(Finding::new(
            RULE_CRATE_NAME,
            format!("{member}/Cargo.toml"),
            0,
            format!("{member}/Cargo.toml: missing or malformed `name =`"),
        ));
    }
}

fn report_lanes_bins(member: &str, manifest: &str) {
    let bins = binary_names(manifest);
    if member.ends_with("titania-lanes") && !bins.is_empty() {
        eprintln!("[check-workspace-assertions] {member} bins: {bins:?}");
    }
}

fn check_forbidden_features(member: &str, manifest: &str, report: &mut LaneReport) {
    let features = named_table_values(manifest, "[features]");
    let forbidden: Vec<String> =
        features.intersection(&expected_set(FORBIDDEN_FEATURE_NAMES)).cloned().collect();
    if !forbidden.is_empty() {
        report.push(Finding::new(
            RULE_FORBIDDEN_FEATURE,
            format!("{member}/Cargo.toml"),
            0,
            format!("{member}/Cargo.toml: forbidden feature names {forbidden:?}"),
        ));
    }
}

pub(super) fn check_forbidden_dependencies(
    root: &Path,
    members: &[String],
    report: &mut LaneReport,
) {
    let forbidden = expected_set(FORBIDDEN_FEATURE_NAMES);
    members.iter().for_each(|member| {
        let manifest_path = root.join(member).join("Cargo.toml");
        let Ok(manifest) = fs::read_to_string(&manifest_path) else {
            return;
        };
        let deps = dependency_names(&manifest);
        let hits: Vec<String> = deps.intersection(&forbidden).cloned().collect();
        if !hits.is_empty() {
            report.push(Finding::new(
                RULE_FORBIDDEN_DEP,
                format!("{member}/Cargo.toml"),
                0,
                format!("{member}/Cargo.toml: forbidden dependency {hits:?}"),
            ));
        }
    });
}

fn dependency_names(manifest: &str) -> BTreeSet<String> {
    ["[dependencies]", "[dev-dependencies]", "[build-dependencies]"]
        .into_iter()
        .flat_map(|table| named_table_values(manifest, table).into_iter())
        .collect()
}

pub(super) fn check_generated_boundaries(root: &Path, report: &mut LaneReport) {
    collect_generated_dirs(root)
        .into_iter()
        .flat_map(|dir| rust_files(&dir).into_iter())
        .for_each(|source| check_generated_file(root, &source, report));
}

fn check_generated_file(root: &Path, source: &Path, report: &mut LaneReport) {
    let Ok(text) = fs::read_to_string(source) else {
        return;
    };
    report.record_scan();
    FORBIDDEN_FEATURE_NAMES.iter().filter(|forbidden| text.contains(**forbidden)).for_each(
        |forbidden| {
            let rel = source
                .strip_prefix(root)
                .map_or_else(|_| source.display().to_string(), |path| path.display().to_string());
            report.push(Finding::new(
                RULE_GENERATED_BOUNDARY,
                rel,
                0,
                format!("forbidden generated-boundary token: {forbidden}"),
            ));
        },
    );
}

fn collect_generated_dirs(root: &Path) -> Vec<PathBuf> {
    match fs::read_dir(root.join("crates")) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path().join("src").join("generated"))
            .filter(|path| path.exists())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn rust_files(root: &Path) -> Vec<PathBuf> {
    match fs::read_dir(root) {
        Ok(entries) => {
            entries.flatten().flat_map(|entry| rust_file_entry(entry.path()).into_iter()).collect()
        }
        Err(_) => Vec::new(),
    }
}

fn rust_file_entry(path: PathBuf) -> Vec<PathBuf> {
    if path.is_dir() {
        rust_files(&path)
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
        vec![path]
    } else {
        Vec::new()
    }
}

pub(super) fn discover_members(root: &Path) -> Vec<String> {
    match fs::read_to_string(root.join("Cargo.toml")) {
        Ok(manifest) => quoted_array_values(&manifest, "members").into_iter().collect(),
        Err(_) => Vec::new(),
    }
}
