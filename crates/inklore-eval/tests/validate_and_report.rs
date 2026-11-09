//! Tests of `validate` and of report generation, including through the real binary.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may unwrap (AGENTS.md)

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::{Fixture, records};
use inklore_eval::config::EvalConfig;
use inklore_eval::{render_report, run_batch, write_report};

fn eval_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_inklore-eval"))
}

#[test]
fn validate_reports_each_problem_separately_and_locates_it() {
    let fx = Fixture::new();
    let header = |id: &str| {
        format!(
            "id = \"{id}\"\nrepo = \"r\"\nbase_commit = \"{}\"\n",
            fx.base
        )
    };
    // 1. unknown field (line 4)
    fx.write(
        "chains/unknownfield/chain.toml",
        &format!(
            "{}bogus = 1\n[[tasks]]\nid = \"t1\"\nprompt = \"p.md\"\n",
            header("unknownfield")
        ),
    );
    fx.write("chains/unknownfield/p.md", "x");
    // 2. unknown check kind
    fx.write(
        "chains/badkind/chain.toml",
        &format!(
            "{}[[tasks]]\nid = \"t1\"\nprompt = \"p.md\"\n[[tasks.checks]]\nname = \"n\"\nkind = \"magic\"\n",
            header("badkind")
        ),
    );
    fx.write("chains/badkind/p.md", "x");
    // 3. missing prompt file
    fx.write(
        "chains/noprompt/chain.toml",
        &format!(
            "{}[[tasks]]\nid = \"t1\"\nprompt = \"missing.md\"\n",
            header("noprompt")
        ),
    );
    // and one valid chain, which must produce no issue
    fx.add_chain(
        "fine",
        "[[tasks]]\nid = \"t1\"\nprompt = \"p.md\"\n",
        &["p.md"],
    );

    let (config, issues) = EvalConfig::load_lenient(&fx.eval);
    assert_eq!(issues.len(), 3, "{issues:#?}");
    assert!(config.chains.contains_key("fine"));

    let find = |dir: &str| {
        issues
            .iter()
            .find(|i| i.file.ends_with(Path::new(dir).join("chain.toml")))
            .unwrap_or_else(|| panic!("no issue for {dir}: {issues:#?}"))
    };
    let unknown_field = find("unknownfield");
    assert!(
        unknown_field.message.contains("unknown field `bogus`"),
        "{unknown_field}"
    );
    assert_eq!(unknown_field.line, Some(4), "{unknown_field}");
    let bad_kind = find("badkind");
    assert!(bad_kind.message.contains("magic"), "{bad_kind}");
    assert!(bad_kind.line.is_some(), "{bad_kind}");
    let no_prompt = find("noprompt");
    assert!(
        no_prompt
            .message
            .contains("prompt file `missing.md` not found"),
        "{no_prompt}"
    );
    assert!(no_prompt.message.contains("task `t1`"), "{no_prompt}");
}

#[test]
fn the_validate_command_fails_on_errors_and_succeeds_on_a_clean_directory() {
    let fx = Fixture::new();
    fx.add_chain(
        "fine",
        "[[tasks]]\nid = \"t1\"\nprompt = \"p.md\"\n",
        &["p.md"],
    );
    let ok = eval_bin()
        .arg("--eval-dir")
        .arg(&fx.eval)
        .arg("validate")
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    assert!(String::from_utf8_lossy(&ok.stdout).contains("1 chain(s)"));

    fx.write("chains/broken/chain.toml", "this is = not [valid\n");
    let bad = eval_bin()
        .arg("--eval-dir")
        .arg(&fx.eval)
        .arg("validate")
        .output()
        .unwrap();
    assert!(!bad.status.success());
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert!(
        stderr.contains("broken") && stderr.contains("1 problem(s) found"),
        "{stderr}"
    );
}

#[test]
fn the_committed_eval_directory_validates() {
    let eval = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("eval");
    let (config, issues) = EvalConfig::load_lenient(&eval);
    assert!(issues.is_empty(), "{issues:#?}");
    assert!(config.arms.contains_key("baseline"));
    assert!(config.repos.is_empty() && config.chains.is_empty());
}

/// Two runs of a two-task chain with the scripted agent, for the report tests.
fn run_two_runs(fx: &Fixture) -> std::path::PathBuf {
    fx.add_chain(
        "two",
        "[[tasks]]\nid = \"t1\"\nprompt = \"t1.md\"\n  [[tasks.checks]]\n  name = \"a\"\n  kind = \"file_contains\"\n  path = \"a.txt\"\n  text = \"one\"\n\n[[tasks]]\nid = \"t2\"\nprompt = \"t2.md\"\n  [[tasks.checks]]\n  name = \"b is absent\"\n  kind = \"file_absent\"\n  path = \"b.txt\"\n",
        &["t1.md", "t2.md"],
    );
    fx.add_scripted_agent(
        "fake",
        "[[tasks.t1]]\naction = \"write_file\"\npath = \"a.txt\"\ncontent = \"one\\n\"\n[[tasks.t2]]\naction = \"write_file\"\npath = \"c.txt\"\ncontent = \"c\\n\"\n[[tasks.t2]]\naction = \"append_file\"\npath = \"c.txt\"\ncontent = \"more\\n\"\n",
    );
    let mut options = fx.options("two", "fake");
    options.runs = 2;
    run_batch(&fx.eval, &options, &|_| {}).unwrap().batch_dir
}

#[test]
fn the_report_is_deterministic_and_regenerates_from_runs_jsonl_alone() {
    let fx = Fixture::new();
    let batch = run_two_runs(&fx);
    let recs = records(&batch);
    assert_eq!(recs.len(), 4);

    let generated = fs::read(batch.join("report.md")).unwrap();
    let text = String::from_utf8(generated.clone()).unwrap();
    assert!(
        text.contains("chain `two` × arm `baseline` (agent `fake`)"),
        "{text}"
    );
    assert!(text.contains("| **Chain total** | 2/2 (100%) |"), "{text}");
    assert!(
        text.contains("–"),
        "cache write is never reported by the scripted agent: {text}"
    );

    // Same input -> byte-identical output, in-process ...
    assert_eq!(render_report(&recs), render_report(&recs));
    fs::remove_file(batch.join("report.md")).unwrap();
    write_report(&batch).unwrap();
    assert_eq!(fs::read(batch.join("report.md")).unwrap(), generated);

    // ... and through the CLI.
    fs::remove_file(batch.join("report.md")).unwrap();
    let out = eval_bin().arg("report").arg(&batch).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read(batch.join("report.md")).unwrap(), generated);
}

#[test]
fn a_corrupt_runs_file_names_the_bad_line() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("runs.jsonl"), "\n{not json}\n").unwrap();
    let error = write_report(dir.path()).unwrap_err().to_string();
    assert!(error.contains("runs.jsonl:2"), "{error}");
}

#[test]
fn the_run_command_works_end_to_end_through_the_binary() {
    let fx = Fixture::new();
    fx.add_chain(
        "one",
        "[[tasks]]\nid = \"t1\"\nprompt = \"t1.md\"\n",
        &["t1.md"],
    );
    fx.add_scripted_agent("fake", "");
    let out = eval_bin()
        .arg("--eval-dir")
        .arg(&fx.eval)
        .args([
            "run", "--chain", "one", "--arm", "baseline", "--agent", "fake", "--runs", "2",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("2 run(s), 2 task execution(s), 0 failed"),
        "{stdout}"
    );
}
