use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

const PINNED_NIGHTLY: &str = "nightly-2026-04-27";
const REQUIRED_COMPONENTS: &[&str] = &["rustfmt", "clippy", "rust-src", "llvm-tools-preview"];
const REQUIRED_TARGET: &str = "x86_64-unknown-linux-gnu";

#[derive(Debug, Clone, PartialEq, Eq)]
struct RustToolchainConfig {
    channel: String,
    components: BTreeSet<String>,
    targets: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MoonRustConfig {
    version: String,
    sync_toolchain_config: bool,
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("cannot derive workspace root from {}", manifest_dir.display()))
}

fn quoted_scalar(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let uncommented = line.split('#').next()?.trim();
        let (candidate, value) = uncommented.split_once('=')?;
        if candidate.trim() == key { unquote(value.trim()).map(ToOwned::to_owned) } else { None }
    })
}

fn yaml_scalar(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let uncommented = line.split('#').next()?.trim();
        let (candidate, value) = uncommented.split_once(':')?;
        if candidate.trim() == key {
            let trimmed = value.trim();
            Some(unquote(trimmed).unwrap_or(trimmed).to_owned())
        } else {
            None
        }
    })
}

fn yaml_bool(text: &str, key: &str) -> Option<bool> {
    yaml_scalar(text, key).and_then(|value| match value.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    })
}

fn unquote(value: &str) -> Option<&str> {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|rest| rest.strip_suffix('\'')))
}

fn quoted_array(text: &str, key: &str) -> Option<BTreeSet<String>> {
    text.lines().find_map(|line| {
        let uncommented = line.split('#').next()?.trim();
        let (candidate, value) = uncommented.split_once('=')?;
        if candidate.trim() != key {
            return None;
        }
        let inner = value.trim().strip_prefix('[')?.strip_suffix(']')?;
        Some(inner.split(',').map(str::trim).filter_map(unquote).map(ToOwned::to_owned).collect())
    })
}

fn parse_rust_toolchain(text: &str) -> Result<RustToolchainConfig, String> {
    Ok(RustToolchainConfig {
        channel: quoted_scalar(text, "channel").ok_or_else(|| "missing channel".to_owned())?,
        components: quoted_array(text, "components")
            .ok_or_else(|| "missing components".to_owned())?,
        targets: quoted_array(text, "targets").ok_or_else(|| "missing targets".to_owned())?,
    })
}

fn parse_moon_rust(text: &str) -> Result<MoonRustConfig, String> {
    Ok(MoonRustConfig {
        version: yaml_scalar(text, "version")
            .ok_or_else(|| "missing moon rust version".to_owned())?,
        sync_toolchain_config: yaml_bool(text, "syncToolchainConfig")
            .ok_or_else(|| "missing syncToolchainConfig".to_owned())?,
    })
}

fn validate_toolchain_pair(
    rust: &RustToolchainConfig,
    moon: &MoonRustConfig,
) -> Result<(), String> {
    if rust.channel != PINNED_NIGHTLY {
        return Err(format!("rust-toolchain.toml channel {} != {PINNED_NIGHTLY}", rust.channel));
    }
    if moon.version != PINNED_NIGHTLY {
        return Err(format!(
            ".moon/toolchains.yml rust version {} != {PINNED_NIGHTLY}",
            moon.version
        ));
    }
    if rust.channel != moon.version {
        return Err(format!(
            "rust-toolchain.toml channel {} != .moon/toolchains.yml version {}",
            rust.channel, moon.version
        ));
    }
    if !moon.sync_toolchain_config {
        return Err(".moon/toolchains.yml syncToolchainConfig must be true".to_owned());
    }
    let missing_components: Vec<&str> = REQUIRED_COMPONENTS
        .iter()
        .copied()
        .filter(|component| !rust.components.contains(*component))
        .collect();
    if !missing_components.is_empty() {
        return Err(format!("rust-toolchain.toml missing components {missing_components:?}"));
    }
    if !rust.targets.contains(REQUIRED_TARGET) {
        return Err(format!("rust-toolchain.toml missing target {REQUIRED_TARGET}"));
    }
    Ok(())
}

#[test]
fn toolchain_configs_are_pinned_nightly_and_in_sync() -> Result<(), String> {
    let root = workspace_root()?;
    let rust_text = std::fs::read_to_string(root.join("rust-toolchain.toml"))
        .map_err(|e| format!("read rust-toolchain.toml: {e}"))?;
    let moon_text = std::fs::read_to_string(root.join(".moon").join("toolchains.yml"))
        .map_err(|e| format!("read .moon/toolchains.yml: {e}"))?;

    let rust = parse_rust_toolchain(&rust_text)?;
    let moon = parse_moon_rust(&moon_text)?;
    validate_toolchain_pair(&rust, &moon)
}

#[test]
fn toolchain_config_validator_reports_mismatch_fixture() {
    let rust = parse_rust_toolchain(
        "[toolchain]\nchannel = \"stable\"\ncomponents = [\"rustfmt\", \"clippy\"]\ntargets = [\"x86_64-unknown-linux-gnu\"]\n",
    )
    .unwrap();
    let moon =
        parse_moon_rust("rust:\n  version: 'nightly-2026-04-27'\n  syncToolchainConfig: true\n")
            .unwrap();

    let err = validate_toolchain_pair(&rust, &moon).unwrap_err();
    assert_eq!(err, "rust-toolchain.toml channel stable != nightly-2026-04-27");
}

#[test]
fn toolchain_config_validator_reports_missing_sync_fixture() {
    let rust = parse_rust_toolchain(
        "[toolchain]\nchannel = \"nightly-2026-04-27\"\ncomponents = [\"rustfmt\", \"clippy\", \"rust-src\", \"llvm-tools-preview\"]\ntargets = [\"x86_64-unknown-linux-gnu\"]\n",
    )
    .unwrap();
    let moon =
        parse_moon_rust("rust:\n  version: 'nightly-2026-04-27'\n  syncToolchainConfig: false\n")
            .unwrap();

    let err = validate_toolchain_pair(&rust, &moon).unwrap_err();
    assert_eq!(err, ".moon/toolchains.yml syncToolchainConfig must be true");
}
