use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

use crate::scan::scan;

pub(super) fn self_test() -> i32 {
    let root = fixture_root();
    if let Err(error) = reset_fixture(&root) {
        eprintln!("FixtureFailure: cleanup failed: {error}");
        return 1;
    }
    if let Err(error) = write_fixtures(&root) {
        eprintln!("FixtureFailure: write failed: {error}");
        return 1;
    }
    match missing_required_classes(&root) {
        Ok(missing) if missing.is_empty() => {
            println!("FixturePass: hot/cold forbidden API scanner");
            0
        }
        Ok(missing) => {
            eprintln!("FixtureFailure: missing classes {missing:?}");
            1
        }
        Err(error) => {
            eprintln!("FixtureFailure: scan failed: {error}");
            1
        }
    }
}

fn fixture_root() -> PathBuf {
    std::env::temp_dir().join(format!("hot-cold-scan-{}", std::process::id()))
}

fn reset_fixture(root: &Path) -> io::Result<()> {
    match fs::remove_dir_all(root) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn write_fixture(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

fn write_fixtures(root: &Path) -> io::Result<()> {
    let hot = root.join("crates/titania-core/src/engine.rs");
    let cold = root.join("crates/titania-core/src/diagnostic.rs");
    write_fixture(
        &hot,
        "pub fn bad() { println!(\"x\"); let _m: HashMap<String, u8> = HashMap::new(); let _c = std::sync::mpsc::channel(); }\n",
    )?;
    write_fixture(&cold, "pub fn ok() { println!(\"diagnostic only\"); }\n")
}

fn missing_required_classes(root: &Path) -> Result<Vec<&'static str>, String> {
    let (_classified, violations, _justified) = scan(root)?;
    let classes: BTreeSet<&'static str> =
        violations.iter().map(|finding| finding.class_id).collect();
    Ok(required_classes().iter().copied().filter(|class_id| !classes.contains(class_id)).collect())
}

const fn required_classes() -> [&'static str; 3] {
    ["FORMAT-PRINT-001", "MAP-STRING-001", "CHANNEL-UNBOUNDED-001"]
}
