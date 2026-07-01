//! bench-run: empirical microbench harness for titania lane binaries.
//!
//! Spawns the prebuilt lane binary against a synthetic workspace fixture,
//! records wall-clock timings, and reports p50/p90/p99 as one CSV row.
//!
//! Usage: bench-run <lane> <file-count> [--trials N] [--warmup N]
//!
//! Output: one CSV row to stdout, comment header to stderr.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::get_unwrap,
    clippy::arithmetic_side_effects,
    clippy::dbg_macro,
    clippy::as_conversions,
    clippy::let_underscore_must_use,
    clippy::needless_pass_by_value
)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use tempfile::TempDir;

/// Lanes known to walk the standard fixture shape (root + crates/proj + src/*.rs).
const KNOWN_LANES: &[&str] = &[
    "forbidden-scan",
    "check-panic-surface",
    "check-source-length",
    "check-ignored-fallible-results",
    "check-nightly-features",
];

#[derive(Debug)]
enum BenchError {
    LaneUnknown(String),
    BinNotFound { lane: String, searched: Vec<PathBuf> },
    Usage(String),
    InvalidFileCount(u32),
    Io(std::io::Error),
}

impl std::fmt::Display for BenchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LaneUnknown(s) => write!(f, "unknown lane: {s}"),
            Self::BinNotFound { lane, searched } => {
                write!(f, "binary for lane '{lane}' not found, searched: {searched:?}")
            }
            Self::Usage(s) => write!(f, "usage error: {s}"),
            Self::InvalidFileCount(n) => write!(f, "invalid file count: {n}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::process::Termination for BenchError {
    fn report(self) -> std::process::ExitCode {
        eprintln!("bench-run: {self}");
        std::process::ExitCode::from(2)
    }
}

#[derive(Debug, Clone)]
struct Args {
    lane: String,
    file_count: u32,
    trials: u32,
    warmup: u32,
}

fn parse_args() -> Result<Args, BenchError> {
    let raw: Vec<String> = env::args().collect();
    let mut iter = raw.iter().skip(1);
    let lane = iter.next().ok_or_else(|| BenchError::Usage("missing <lane>".into()))?.clone();
    let file_count: u32 = iter
        .next()
        .ok_or_else(|| BenchError::Usage("missing <file-count>".into()))?
        .parse()
        .map_err(|_| BenchError::Usage("file-count must be u32".into()))?;
    let mut trials: u32 = 5;
    let mut warmup: u32 = 2;
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--trials" => {
                trials = iter
                    .next()
                    .ok_or_else(|| BenchError::Usage("--trials requires value".into()))?
                    .parse()
                    .map_err(|_| BenchError::Usage("--trials must be u32".into()))?;
            }
            "--warmup" => {
                warmup = iter
                    .next()
                    .ok_or_else(|| BenchError::Usage("--warmup requires value".into()))?
                    .parse()
                    .map_err(|_| BenchError::Usage("--warmup must be u32".into()))?;
            }
            other => return Err(BenchError::Usage(format!("unknown flag: {other}"))),
        }
    }
    if !KNOWN_LANES.contains(&lane.as_str()) {
        return Err(BenchError::LaneUnknown(lane));
    }
    if file_count == 0 {
        return Err(BenchError::InvalidFileCount(file_count));
    }
    Ok(Args { lane, file_count, trials, warmup })
}

fn locate_bin(lane: &str) -> Result<PathBuf, BenchError> {
    let mut searched = Vec::new();
    // Canonicalize so the path is absolute; `Command::new` resolves
    // relative paths against the child's cwd (which we override via
    // `current_dir` per trial), not the bench runner's cwd.
    let resolve = |raw: PathBuf| -> Result<PathBuf, BenchError> {
        std::fs::canonicalize(&raw).map_err(BenchError::Io)
    };
    if let Ok(dir) = env::var("TITANIA_BIN_DIR") {
        let p = PathBuf::from(dir).join(lane);
        searched.push(p.clone());
        if p.is_file() {
            return resolve(p);
        }
    }
    let cwd_rel = PathBuf::from("./target/release").join(lane);
    searched.push(cwd_rel.clone());
    if cwd_rel.is_file() {
        return resolve(cwd_rel);
    }
    Err(BenchError::BinNotFound { lane: lane.to_string(), searched })
}

fn write_fixture(file_count: u32) -> Result<TempDir, BenchError> {
    let dir = tempfile::tempdir().map_err(BenchError::Io)?;
    let root = dir.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/proj\"]\nresolver = \"3\"\n",
    )
    .map_err(BenchError::Io)?;
    fs::create_dir_all(root.join("crates/proj/src")).map_err(BenchError::Io)?;
    fs::write(
        root.join("crates/proj/Cargo.toml"),
        "[package]\nname = \"proj\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(BenchError::Io)?;
    let src = root.join("crates/proj/src");
    for idx in 0..file_count {
        let body = fixture_file_body(idx);
        fs::write(src.join(format!("f_{idx:06}.rs")), body).map_err(BenchError::Io)?;
    }
    Ok(dir)
}

fn fixture_file_body(idx: u32) -> String {
    let mut s = String::with_capacity(2048);
    s.push_str(&format!("//! Synthetic fixture file {idx}.\n"));
    s.push_str(&format!("const IDX: u32 = {idx};\n\n"));
    s.push_str("#[derive(Debug, Clone, PartialEq, Eq)]\n");
    s.push_str("struct Item { id: u32, name: String }\n\n");
    s.push_str("impl Item {\n");
    s.push_str("    fn new(id: u32, name: &str) -> Self {\n");
    s.push_str("        Self { id, name: name.to_owned() }\n");
    s.push_str("    }\n");
    s.push_str("}\n\n");
    s.push_str("fn process(items: &[Item]) -> Vec<u32> {\n");
    s.push_str("    items.iter().map(|i| i.id.saturating_add(IDX)).collect()\n");
    s.push_str("}\n\n");
    s.push_str("#[cfg(test)]\n");
    s.push_str("mod tests {\n");
    s.push_str("    use super::*;\n");
    s.push_str("    #[test]\n");
    s.push_str("    fn basic() {\n");
    s.push_str("        let items = vec![Item::new(1, \"a\"), Item::new(2, \"b\")];\n");
    s.push_str("        assert_eq!(process(&items).len(), 2);\n");
    s.push_str("    }\n");
    s.push_str("}\n");
    s
}

fn run_trial(bin: &Path, root: &Path) -> (Duration, i32) {
    let started = Instant::now();
    let result = Command::new(bin).arg("--quiet").current_dir(root).status();
    let elapsed = started.elapsed();
    let code = match result {
        Ok(status) => status.code().map_or(-1, i32::from),
        Err(_) => -1,
    };
    (elapsed, code)
}

fn percentiles(samples: &mut [Duration]) -> (Duration, Duration, Duration) {
    if samples.is_empty() {
        return (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    }
    samples.sort_unstable();
    let len = samples.len();
    let p50_idx = len / 2;
    let p90_idx = len.saturating_mul(9).saturating_div(10);
    let p99_idx = len.saturating_sub(1);
    let p50 = samples.get(p50_idx).copied().unwrap_or(Duration::ZERO);
    let p90 = samples.get(p90_idx).copied().unwrap_or(Duration::ZERO);
    let p99 = samples.get(p99_idx).copied().unwrap_or(Duration::ZERO);
    (p50, p90, p99)
}

fn print_csv_row(args: &Args, p50: Duration, p90: Duration, p99: Duration, exit_code: i32) {
    println!(
        "{},{},{},{},{},{},{}",
        args.lane,
        args.file_count,
        args.trials,
        p50.as_micros(),
        p90.as_micros(),
        p99.as_micros(),
        exit_code
    );
}

fn main() -> Result<(), BenchError> {
    let args = parse_args()?;
    let bin = locate_bin(&args.lane)?;
    let bin_meta = fs::metadata(&bin).map_err(BenchError::Io)?;
    let bin_size = bin_meta.len();
    let bin_mtime = bin_meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0_u64, |d| d.as_secs());

    let warmup_dir = write_fixture(args.file_count)?;
    for _ in 0..args.warmup {
        let _ = run_trial(&bin, warmup_dir.path());
    }

    let mut samples: Vec<Duration> =
        Vec::with_capacity(usize::try_from(args.trials).unwrap_or(usize::MAX));
    let mut last_code: i32 = -1;
    let trial_dir = write_fixture(args.file_count)?;
    for _ in 0..args.trials {
        let (d, code) = run_trial(&bin, trial_dir.path());
        samples.push(d);
        last_code = code;
    }

    let (p50, p90, p99) = percentiles(&mut samples);

    eprintln!(
        "# bin={} size={} mtime={} fixture_files={}",
        bin.display(),
        bin_size,
        bin_mtime,
        args.file_count
    );
    print_csv_row(&args, p50, p90, p99, last_code);
    Ok(())
}
