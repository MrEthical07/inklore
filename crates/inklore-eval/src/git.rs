//! Thin wrappers around the `git` executable. Always an argv array, never a shell.

use std::path::Path;
use std::process::Command;

use crate::error::EvalError;

/// Fixed identity so commits work on machines without a Git identity.
const IDENTITY: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "inklore-eval"),
    ("GIT_AUTHOR_EMAIL", "eval@inklore.invalid"),
    ("GIT_COMMITTER_NAME", "inklore-eval"),
    ("GIT_COMMITTER_EMAIL", "eval@inklore.invalid"),
];

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    // Line endings, signing and quoting must not depend on the machine's Git configuration.
    cmd.args(["-c", "core.autocrlf=false"])
        .args(["-c", "commit.gpgsign=false"])
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(dir)
        .envs(IDENTITY)
        .env("GIT_TERMINAL_PROMPT", "0");
    cmd
}

fn describe(args: &[&str]) -> String {
    args.join(" ")
}

/// Run `git <args>` in `dir`; returns stdout on success.
pub(crate) fn git(dir: &Path, args: &[&str]) -> Result<String, EvalError> {
    let output = command(dir, args)
        .output()
        .map_err(|source| EvalError::Spawn {
            program: "git".to_string(),
            source,
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(EvalError::Git {
            args: describe(args),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// `true` when `git <args>` exits with status 0.
pub(crate) fn git_succeeds(dir: &Path, args: &[&str]) -> bool {
    command(dir, args)
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Clone `source` into `dest` (which must not exist); `parent` is where git runs.
pub(crate) fn clone(parent: &Path, source: &str, dest: &Path) -> Result<(), EvalError> {
    let dest = dest.to_string_lossy();
    git(parent, &["clone", "--quiet", "--", source, &dest]).map(drop)
}

/// `true` when `commit` names a commit in the repository at `repo`.
pub(crate) fn commit_exists(repo: &Path, commit: &str) -> bool {
    let spec = format!("{commit}^{{commit}}");
    git_succeeds(repo, &["cat-file", "-e", &spec])
}

/// Check out `commit` with a detached HEAD.
pub(crate) fn checkout_detached(repo: &Path, commit: &str) -> Result<(), EvalError> {
    git(repo, &["checkout", "--quiet", "--detach", commit]).map(drop)
}

/// Stage everything, including new and deleted files.
pub(crate) fn stage_all(repo: &Path) -> Result<(), EvalError> {
    git(repo, &["add", "-A"]).map(drop)
}

/// `git diff --stat` of the staged changes against HEAD.
pub(crate) fn staged_diff_stat(repo: &Path) -> Result<String, EvalError> {
    git(repo, &["diff", "--cached", "--stat"]).map(|s| s.trim_end().to_string())
}

/// Paths changed in the staged changes against HEAD, sorted, with `/` separators.
pub(crate) fn staged_changed_paths(repo: &Path) -> Result<Vec<String>, EvalError> {
    let raw = git(repo, &["diff", "--cached", "--name-only", "-z"])?;
    let mut paths: Vec<String> = raw
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    paths.sort();
    Ok(paths)
}

/// Commit whatever is staged (an empty commit is fine) so the next diff starts here.
pub(crate) fn commit(repo: &Path, message: &str) -> Result<(), EvalError> {
    git(
        repo,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "--no-verify",
            "-m",
            message,
        ],
    )
    .map(drop)
}
