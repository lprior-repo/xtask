//! Discover the target Rust project from a starting directory.
//!
//! The action layer walks ancestor directories and reads Cargo manifests.
//! The pure selector then chooses the nearest workspace root, falling back
//! to the nearest non-workspace package manifest.

use std::path::{Path, PathBuf};

use crate::{error::TargetProjectError, target_project::TargetProject};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestStatus {
    Workspace,
    Package,
    Other,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestObservation {
    root: PathBuf,
    manifest: PathBuf,
    status: ManifestStatus,
}

/// Discover the target project from a starting directory.
///
/// Workspace roots win over package roots. If no ancestor manifest has a
/// `[workspace]` table, the nearest ancestor with a `Cargo.toml` file is
/// used as a single-crate target.
///
/// # Errors
/// - [`TargetProjectError::NonAbsolute`] if `cwd` is relative.
/// - [`TargetProjectError::NoCargoToml`] if no ancestor has a Cargo.toml
///   file.
/// - [`TargetProjectError::CargoTomlNotFile`] if an ancestor path named
///   `Cargo.toml` exists but is not a file.
/// - [`TargetProjectError::MalformedCargoToml`] if the selected manifest is
///   malformed TOML.
/// - [`TargetProjectError::Io`] for non-NotFound filesystem failures.
pub fn discover_target(cwd: &Path) -> Result<TargetProject, TargetProjectError> {
    if !cwd.is_absolute() {
        return Err(TargetProjectError::NonAbsolute(cwd.display().to_string()));
    }

    let observations = cwd
        .ancestors()
        .map(read_manifest_observation)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let target = select_target_root(&observations)?;
    TargetProject::try_from_path(&target)
}

fn read_manifest_observation(
    root: &Path,
) -> Result<Option<ManifestObservation>, TargetProjectError> {
    let manifest = root.join("Cargo.toml");
    let metadata = match std::fs::metadata(&manifest) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(TargetProjectError::Io {
                path: manifest.display().to_string(),
                kind: e.kind(),
            });
        }
    };
    if !metadata.is_file() {
        return Err(TargetProjectError::CargoTomlNotFile);
    }
    let text = std::fs::read_to_string(&manifest).map_err(|e| TargetProjectError::Io {
        path: manifest.display().to_string(),
        kind: e.kind(),
    })?;
    Ok(Some(ManifestObservation {
        root: root.to_path_buf(),
        manifest,
        status: manifest_status(&text),
    }))
}

fn select_target_root(observations: &[ManifestObservation]) -> Result<PathBuf, TargetProjectError> {
    if let Some(workspace) = observations.iter().find(|o| o.status == ManifestStatus::Workspace) {
        return Ok(workspace.root.clone());
    }
    observations
        .iter()
        .find(|o| matches!(o.status, ManifestStatus::Package | ManifestStatus::Malformed))
        .map_or(Err(TargetProjectError::NoCargoToml), selected_target_root)
}

fn selected_target_root(observation: &ManifestObservation) -> Result<PathBuf, TargetProjectError> {
    match observation.status {
        ManifestStatus::Workspace | ManifestStatus::Package => Ok(observation.root.clone()),
        ManifestStatus::Malformed => Err(TargetProjectError::MalformedCargoToml {
            path: observation.manifest.display().to_string(),
        }),
        ManifestStatus::Other => Err(TargetProjectError::NoCargoToml),
    }
}

fn manifest_status(toml_text: &str) -> ManifestStatus {
    match toml_text.parse::<toml_edit::DocumentMut>() {
        Ok(doc) if has_explicit_table(&doc, "workspace") => ManifestStatus::Workspace,
        Ok(doc) if has_explicit_table(&doc, "package") => ManifestStatus::Package,
        Ok(_) => ManifestStatus::Other,
        Err(_) => ManifestStatus::Malformed,
    }
}

/// Returns `true` if the given Cargo.toml document has an explicit table.
/// TOML parsing prevents comments, strings, arrays, and implicit parent
/// tables such as `[workspace.metadata]` from being treated as roots.
fn has_explicit_table(doc: &toml_edit::DocumentMut, name: &str) -> bool {
    doc.get(name).and_then(toml_edit::Item::as_table).is_some_and(|table| !table.is_implicit())
}
