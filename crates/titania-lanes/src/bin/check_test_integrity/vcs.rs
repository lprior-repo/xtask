use std::{env, fs};

use titania_core::TargetProject;
use titania_lanes::CommandIn;

use super::{RootInfo, Vcs, scan::is_test_path};

fn tool_output(tool: &str, args: &[&str], target: &TargetProject) -> Result<String, String> {
    let joined_args = args.join(" ");
    let mut command = CommandIn::new(target, tool)
        .map_err(|error| format!("{tool} {joined_args} failed to start: {error}"))?;
    command.inherit_env();
    command.args(args);
    let output = command
        .run_capture_raw()
        .map_err(|error| format!("{tool} {joined_args} failed to start: {error}"))?;
    if output.status.success() {
        output
            .stdout_str()
            .map(str::to_owned)
            .map_err(|error| format!("{tool} {joined_args} returned non-UTF8 stdout: {error}"))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("{tool} {joined_args} failed: {stderr}"))
    }
}

fn command_output(args: &[&str], target: &TargetProject) -> Result<String, String> {
    tool_output("git", args, target)
}

fn jj_output(args: &[&str], target: &TargetProject) -> Result<String, String> {
    tool_output("jj", args, target)
}

fn command_output_allow_fail(args: &[&str], target: &TargetProject) -> Option<String> {
    let mut command = match CommandIn::new(target, "git") {
        Ok(command) => command,
        Err(_) => return None,
    };
    command.inherit_env();
    command.args(args);
    command
        .run_capture_raw()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| output.stdout_str().ok().map(str::to_owned))
}

pub(super) fn root_dir(target: &TargetProject) -> Result<RootInfo, String> {
    if command_output(&["rev-parse", "--show-toplevel"], target).is_ok() {
        return Ok(RootInfo { vcs: Vcs::Git });
    }
    jj_output(&["workspace", "root"], target).map(|_| RootInfo { vcs: Vcs::Jj })
}

pub(super) fn default_base(target: &TargetProject, vcs: Vcs) -> String {
    if let Ok(value) = env::var("TEST_INTEGRITY_BASE") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    if vcs == Vcs::Jj {
        return "@-".to_owned();
    }
    let dirty = command_output(&["status", "--porcelain"], target)
        .map_or(true, |text| !text.trim().is_empty());
    if dirty {
        return "HEAD".to_owned();
    }
    match command_output_allow_fail(&["merge-base", "origin/main", "HEAD"], target)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
    {
        Some(base) => base,
        None => "HEAD".to_owned(),
    }
}

pub(super) fn validate_base_revision(
    target: &TargetProject,
    base: &str,
    vcs: Vcs,
) -> Result<(), String> {
    if base.trim().is_empty() {
        return Err("empty base revision".to_owned());
    }
    let result = match vcs {
        Vcs::Git => {
            let commit = format!("{base}^{{commit}}");
            command_output(&["rev-parse", "--verify", &commit], target).map(|_| ())
        }
        Vcs::Jj => {
            jj_output(&["log", "--no-graph", "-r", base, "-T", "commit_id"], target).map(|_| ())
        }
    };
    result.map_err(|error| format!("invalid base revision {base:?}: {error}"))
}

pub(super) fn changed_files(
    target: &TargetProject,
    base: &str,
    vcs: Vcs,
) -> Result<Vec<(String, String)>, String> {
    match vcs {
        Vcs::Git => git_changed_files(target, base),
        Vcs::Jj => jj_changed_files(target, base),
    }
}

fn git_changed_files(target: &TargetProject, base: &str) -> Result<Vec<(String, String)>, String> {
    let mut entries = parse_git_name_status(&command_output(
        &["diff", "--name-status", "--find-renames", base, "--"],
        target,
    )?);
    entries
        .extend(untracked_files(target, Vcs::Git)?.into_iter().map(|path| ("??".to_owned(), path)));
    Ok(entries)
}

fn parse_git_name_status(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let parts = line.split('\t').collect::<Vec<_>>();
            (parts.len() >= 2).then(|| {
                let status = parts.first().copied().map_or("", |value| value).to_owned();
                let path = parts.last().copied().map_or("", |value| value).to_owned();
                (status, path)
            })
        })
        .collect()
}

fn jj_changed_files(target: &TargetProject, base: &str) -> Result<Vec<(String, String)>, String> {
    jj_output(&["diff", "--summary", "--from", base, "--to", "@"], target).map(|text| {
        text.lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                let mut chars = trimmed.chars();
                let status = chars.next()?.to_string();
                let path = trimmed.get(1..)?.trim().to_owned();
                (!path.is_empty()).then_some((status, path))
            })
            .collect()
    })
}

pub(super) fn diff_text(target: &TargetProject, base: &str, vcs: Vcs) -> Result<String, String> {
    match vcs {
        Vcs::Git => git_diff_text(target, base),
        Vcs::Jj => jj_output(&["diff", "--git", "--from", base, "--to", "@"], target),
    }
}

fn git_diff_text(target: &TargetProject, base: &str) -> Result<String, String> {
    let mut text = command_output(&["diff", "--find-renames", "--unified=0", base, "--"], target)?;
    let untracked = untracked_files(target, Vcs::Git)?;
    let extra = untracked
        .iter()
        .filter(|path| is_test_path(path))
        .filter_map(|path| untracked_file_diff(target, path))
        .collect::<String>();
    text.push_str(&extra);
    Ok(text)
}

fn untracked_files(target: &TargetProject, vcs: Vcs) -> Result<Vec<String>, String> {
    match vcs {
        Vcs::Git => {
            command_output(&["ls-files", "--others", "--exclude-standard"], target).map(|text| {
                text.lines()
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
        }
        Vcs::Jj => Ok(Vec::new()),
    }
}

fn untracked_file_diff(target: &TargetProject, path: &str) -> Option<String> {
    let full_path = target.as_std_path().join(path);
    let content = fs::read_to_string(full_path).ok()?;
    let additions = content.lines().map(|line| format!("+{line}\n")).collect::<String>();
    Some(format!(
        "diff --git a/{path} b/{path}\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n{additions}",
        content.lines().count()
    ))
}
