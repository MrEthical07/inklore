//! Running the checks that decide whether a task passed.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::{CheckDef, safe_join};
use crate::proc::run_with_timeout;
use crate::results::CheckRecord;

/// How many lines of output a [`CheckRecord`] keeps.
pub const OUTPUT_TAIL_LINES: usize = 50;

/// Run one check in the working copy `workdir`.
pub(crate) fn run_check(check: &CheckDef, workdir: &Path) -> CheckRecord {
    let started = Instant::now();
    let (passed, lines) = match check {
        CheckDef::Command {
            command,
            timeout_secs,
            ..
        } => run_command(command, workdir, Duration::from_secs(*timeout_secs)),
        CheckDef::FileContains { path, text, .. } => file_contains(workdir, path, text),
        CheckDef::FileAbsent { path, .. } => file_absent(workdir, path),
    };
    CheckRecord {
        name: check.name().to_string(),
        passed,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        output_tail: tail(lines, OUTPUT_TAIL_LINES),
    }
}

fn run_command(argv: &[String], workdir: &Path, timeout: Duration) -> (bool, Vec<String>) {
    let Some((program, args)) = argv.split_first() else {
        return (false, vec!["empty command".to_string()]);
    };
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(workdir);
    match run_with_timeout(cmd, timeout) {
        Err(error) => (false, vec![format!("cannot start `{program}`: {error}")]),
        Ok(output) => {
            let mut lines = split_lines(&output.stdout);
            lines.extend(split_lines(&output.stderr));
            if output.timed_out {
                lines.push(format!("timed out after {}s", timeout.as_secs()));
            }
            let passed = !output.timed_out && output.status.is_some_and(|s| s.success());
            (passed, lines)
        }
    }
}

fn file_contains(workdir: &Path, path: &str, text: &str) -> (bool, Vec<String>) {
    let Ok(full) = safe_join(workdir, path) else {
        return (false, vec![format!("unsafe path `{path}`")]);
    };
    match fs::read(&full) {
        Err(error) => (false, vec![format!("cannot read `{path}`: {error}")]),
        Ok(bytes) => {
            let found = String::from_utf8_lossy(&bytes).contains(text);
            let message = if found {
                format!("`{path}` contains the text")
            } else {
                format!("`{path}` does not contain `{text}`")
            };
            (found, vec![message])
        }
    }
}

fn file_absent(workdir: &Path, path: &str) -> (bool, Vec<String>) {
    let Ok(full) = safe_join(workdir, path) else {
        return (false, vec![format!("unsafe path `{path}`")]);
    };
    if full.exists() {
        (false, vec![format!("`{path}` exists but must be absent")])
    } else {
        (true, vec![format!("`{path}` is absent")])
    }
}

fn split_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::to_string)
        .collect()
}

fn tail(mut lines: Vec<String>, keep: usize) -> Vec<String> {
    let extra = lines.len().saturating_sub(keep);
    lines.drain(..extra);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_last_lines() {
        let lines: Vec<String> = (0..60).map(|i| i.to_string()).collect();
        let kept = tail(lines, 50);
        assert_eq!(kept.len(), 50);
        assert_eq!(kept.first().map(String::as_str), Some("10"));
        assert_eq!(kept.last().map(String::as_str), Some("59"));
        assert_eq!(tail(vec!["a".into()], 50), vec!["a".to_string()]);
    }

    #[test]
    fn file_checks_work_on_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello world").unwrap();
        assert!(file_contains(dir.path(), "a.txt", "world").0);
        assert!(!file_contains(dir.path(), "a.txt", "nope").0);
        assert!(!file_contains(dir.path(), "missing.txt", "x").0);
        assert!(!file_contains(dir.path(), "../a.txt", "x").0);
        assert!(file_absent(dir.path(), "b.txt").0);
        assert!(!file_absent(dir.path(), "a.txt").0);
    }
}
