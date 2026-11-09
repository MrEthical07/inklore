//! End-to-end tests of the runner with the scripted agent on throwaway Git repositories.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests may unwrap (AGENTS.md)

mod common;

use std::fs;
use std::time::{Duration, Instant};

use common::{Fixture, records};
use inklore_eval::{AgentExit, EvalError, run_batch};

const TWO_TASKS: &str = r#"
[[tasks]]
id = "t1"
prompt = "t1.md"
timeout_secs = 60
  [[tasks.checks]]
  name = "a written"
  kind = "file_contains"
  path = "a.txt"
  text = "one"

[[tasks]]
id = "t2"
prompt = "t2.md"
timeout_secs = 60
  [[tasks.checks]]
  name = "b written"
  kind = "file_contains"
  path = "b.txt"
  text = "two"
  [[tasks.checks]]
  name = "a still there"
  kind = "file_contains"
  path = "a.txt"
  text = "one"
"#;

const WRITES_BOTH: &str = r#"
[[tasks.t1]]
action = "write_file"
path = "a.txt"
content = "one\n"

[[tasks.t2]]
action = "write_file"
path = "sub/b.txt"
content = "two\n"
"#;

#[test]
fn two_task_chain_passes_and_each_diff_has_only_its_own_changes() {
    let fx = Fixture::new();
    fx.add_chain(
        "two",
        &TWO_TASKS.replace("path = \"b.txt\"", "path = \"sub/b.txt\""),
        &["t1.md", "t2.md"],
    );
    fx.add_scripted_agent("fake", WRITES_BOTH);

    let summary = run_batch(&fx.eval, &fx.options("two", "fake"), &|_| {}).unwrap();
    let recs = records(&summary.batch_dir);

    assert_eq!(recs.len(), 2);
    assert!(recs.iter().all(|r| r.passed), "{recs:#?}");
    assert_eq!(recs[0].changed_paths, vec!["a.txt"]);
    assert_eq!(
        recs[1].changed_paths,
        vec!["sub/b.txt"],
        "task 2's diff must not repeat task 1's"
    );
    assert!(recs[1].diff_stat.contains("b.txt"));
    assert!(!recs[1].diff_stat.contains("a.txt"));
    assert_eq!(recs[0].base_commit, fx.base);
    assert!(recs[0].metrics.turns.is_some());
    assert_eq!(recs[0].metrics.cache_write_tokens, None);
    assert!(summary.batch_dir.join("report.md").is_file());
    assert_eq!(
        (summary.runs, summary.task_executions, summary.failed_tasks),
        (1, 2, 0)
    );
}

#[test]
fn failing_check_fails_the_task_and_the_chain_continues_unless_stop_on_fail() {
    let fx = Fixture::new();
    fx.add_chain("two", TWO_TASKS, &["t1.md", "t2.md"]);
    // t1 writes the wrong content, so its check fails; t2 would pass.
    fx.add_scripted_agent(
        "fake",
        r#"
[[tasks.t1]]
action = "write_file"
path = "a.txt"
content = "wrong\n"
[[tasks.t2]]
action = "write_file"
path = "b.txt"
content = "two\n"
[[tasks.t2]]
action = "write_file"
path = "a.txt"
content = "one\n"
"#,
    );

    let mut options = fx.options("two", "fake");
    let recs = records(&run_batch(&fx.eval, &options, &|_| {}).unwrap().batch_dir);
    assert_eq!(recs.len(), 2, "the next task still runs after a failure");
    assert!(!recs[0].passed);
    assert!(!recs[0].checks[0].passed);
    assert!(recs[0].checks[0].output_tail[0].contains("does not contain"));
    assert!(recs[1].passed);

    options.stop_on_fail = true;
    let recs = records(&run_batch(&fx.eval, &options, &|_| {}).unwrap().batch_dir);
    assert_eq!(recs.len(), 1, "--stop-on-fail must not run task 2");
    assert!(!recs[0].passed);
}

#[test]
fn two_batches_back_to_back_get_distinct_run_ids_and_both_succeed() {
    let fx = Fixture::new();
    fx.add_chain(
        "two",
        &TWO_TASKS.replace("path = \"b.txt\"", "path = \"sub/b.txt\""),
        &["t1.md", "t2.md"],
    );
    fx.add_scripted_agent("fake", WRITES_BOTH);
    let options = fx.options("two", "fake");

    // No delay between the batches: they may well start within the same second.
    let first = run_batch(&fx.eval, &options, &|_| {}).unwrap();
    let second = run_batch(&fx.eval, &options, &|_| {}).unwrap();

    let first_recs = records(&first.batch_dir);
    let second_recs = records(&second.batch_dir);
    assert!(first_recs.iter().all(|r| r.passed), "{first_recs:#?}");
    assert!(second_recs.iter().all(|r| r.passed), "{second_recs:#?}");
    assert_ne!(first_recs[0].run_id, second_recs[0].run_id);
    assert!(
        first_recs[0]
            .run_id
            .ends_with(first.batch_dir.file_name().unwrap().to_str().unwrap())
    );
}

#[test]
fn a_task_over_its_timeout_is_recorded_as_timed_out_and_the_run_continues() {
    let fx = Fixture::new();
    fx.add_chain(
        "slow",
        r#"
[[tasks]]
id = "t1"
prompt = "t1.md"
timeout_secs = 1

[[tasks]]
id = "t2"
prompt = "t2.md"
timeout_secs = 60
  [[tasks.checks]]
  name = "b written"
  kind = "file_contains"
  path = "b.txt"
  text = "two"
"#,
        &["t1.md", "t2.md"],
    );
    fx.add_scripted_agent(
        "fake",
        r#"
[[tasks.t1]]
action = "sleep"
secs = 600
[[tasks.t2]]
action = "write_file"
path = "b.txt"
content = "two\n"
"#,
    );

    let started = Instant::now();
    let recs = records(
        &run_batch(&fx.eval, &fx.options("slow", "fake"), &|_| {})
            .unwrap()
            .batch_dir,
    );
    assert!(started.elapsed() < Duration::from_secs(120));
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0].agent_exit, AgentExit::TimedOut);
    assert!(!recs[0].passed);
    assert_eq!(recs[1].agent_exit, AgentExit::Completed);
    assert!(recs[1].passed);
}

#[test]
fn a_missing_base_commit_is_a_clear_error() {
    let fx = Fixture::new();
    fx.add_chain("two", TWO_TASKS, &["t1.md", "t2.md"]);
    let chain_file = fx.eval.join("chains/two/chain.toml");
    let text = fs::read_to_string(&chain_file)
        .unwrap()
        .replace(&fx.base, "deadbeefdeadbeef");
    fs::write(&chain_file, text).unwrap();
    fx.add_scripted_agent("fake", "");

    let error = run_batch(&fx.eval, &fx.options("two", "fake"), &|_| {}).unwrap_err();
    assert!(matches!(error, EvalError::MissingCommit { .. }), "{error}");
    assert!(error.to_string().contains("deadbeefdeadbeef"));
}

#[test]
fn an_arm_copies_files_in_and_they_do_not_count_as_task_changes() {
    let fx = Fixture::new();
    fx.write("arms/pushy.toml", "name = \"pushy\"\n[[copy]]\nfrom = \"files/settings.json\"\nto = \".claude/settings.json\"\n");
    fx.write("arms/files/settings.json", "{}\n");
    fx.add_chain("two", TWO_TASKS, &["t1.md", "t2.md"]);
    fx.add_scripted_agent("fake", WRITES_BOTH);

    let mut options = fx.options("two", "fake");
    options.arms = vec!["pushy".to_string()];
    let summary = run_batch(&fx.eval, &options, &|_| {}).unwrap();
    let recs = records(&summary.batch_dir);
    assert_eq!(recs[0].arm, "pushy");
    assert_eq!(
        recs[0].changed_paths,
        vec!["a.txt"],
        "the arm's files are committed before task 1"
    );
    let work = fs::read_dir(fx.eval.join(".work"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(work.join("repo/.claude/settings.json").is_file());
}

#[test]
fn unknown_chain_arm_or_agent_is_reported_by_name() {
    let fx = Fixture::new();
    fx.add_chain("two", TWO_TASKS, &["t1.md", "t2.md"]);
    fx.add_scripted_agent("fake", "");

    let mut options = fx.options("nope", "fake");
    assert!(
        run_batch(&fx.eval, &options, &|_| {})
            .unwrap_err()
            .to_string()
            .contains("chain `nope`")
    );
    options = fx.options("two", "ghost");
    assert!(
        run_batch(&fx.eval, &options, &|_| {})
            .unwrap_err()
            .to_string()
            .contains("agent `ghost`")
    );
    options = fx.options("two", "fake");
    options.arms = vec!["missing".to_string()];
    assert!(
        run_batch(&fx.eval, &options, &|_| {})
            .unwrap_err()
            .to_string()
            .contains("arm `missing`")
    );
}
