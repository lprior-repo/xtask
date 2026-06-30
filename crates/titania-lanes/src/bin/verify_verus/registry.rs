use std::{fs, io, path::Path};

use titania_core::TargetProject;

const FIXTURE_SMOKE_MARKER: &str = "titania-verus-binding: fixture-smoke";
const FIXTURE_SMOKE_FILE: &str = "formal_setup_smoke.rs";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProofTargetKind {
    Production,
    FixtureSmoke,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProofTarget {
    path: String,
    kind: ProofTargetKind,
}

impl ProofTarget {
    #[must_use]
    pub(crate) fn path(&self) -> &str {
        &self.path
    }
}

#[must_use]
pub(crate) fn registry_path_is_nonempty(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else { return false };
    meta.is_file() && meta.len() != 0
}

pub(crate) fn parse_registry_targets(
    path: &Path,
    target: &TargetProject,
) -> io::Result<Vec<ProofTarget>> {
    let text = fs::read_to_string(path)?;
    Ok(extract_yaml_target_paths(&text)
        .into_iter()
        .map(|proof_path| ProofTarget {
            kind: classify_proof_target(target, &proof_path),
            path: proof_path,
        })
        .collect())
}

#[must_use]
pub(crate) fn contains_only_fixture_smoke(targets: &[ProofTarget]) -> bool {
    !targets.is_empty() && targets.iter().all(|target| target.kind == ProofTargetKind::FixtureSmoke)
}

fn classify_proof_target(target: &TargetProject, proof_path: &str) -> ProofTargetKind {
    let path = target.as_std_path().join(proof_path);
    if proof_path.ends_with(FIXTURE_SMOKE_FILE) || file_has_fixture_marker(&path) {
        ProofTargetKind::FixtureSmoke
    } else {
        ProofTargetKind::Production
    }
}

fn file_has_fixture_marker(path: &Path) -> bool {
    fs::read_to_string(path)
        .is_ok_and(|text| text.lines().any(|line| line.contains(FIXTURE_SMOKE_MARKER)))
}

fn extract_yaml_target_paths(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_start)
        .filter_map(|line| {
            let rest = line.strip_prefix("- path:")?.trim();
            strip_yaml_quotes(rest).map(str::to_owned)
        })
        .collect()
}

fn strip_yaml_quotes(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let quote_char = trimmed.chars().next()?;
    if (quote_char == '"' || quote_char == '\'')
        && trimmed.ends_with(quote_char)
        && trimmed.len() >= 2
    {
        trimmed.get(1..trimmed.len().saturating_sub(1))
    } else {
        Some(trimmed)
    }
}
