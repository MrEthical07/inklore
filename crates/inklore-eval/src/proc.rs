//! Running a child process with a time limit, capturing its output, and killing its whole process tree.
//!
//! Everything is an argv array; no shell is ever involved.
//!
//! * **Unix:** the child is started in its own process group
//!   (`CommandExt::process_group(0)`) and the group is killed with `killpg(SIGKILL)`.
//! * **Windows:** `taskkill /T /F /PID <pid>` is run (an executable, not a shell).

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

/// How long to wait for the output readers after the process is gone. A grandchild that outlives its
/// parent can keep the pipes open; we then keep what was read so far instead of hanging.
const READER_GRACE: Duration = Duration::from_secs(2);

/// What a finished (or killed) child produced.
#[derive(Debug)]
pub(crate) struct ProcOutput {
    /// Exit status; `None` when the process was killed because of the timeout.
    pub status: Option<ExitStatus>,
    /// `true` when the time limit was hit.
    pub timed_out: bool,
    /// Everything written to stdout.
    pub stdout: Vec<u8>,
    /// Everything written to stderr.
    pub stderr: Vec<u8>,
}

/// Run `cmd` to completion or until `timeout`, killing the process tree on timeout.
pub(crate) fn run_with_timeout(mut cmd: Command, timeout: Duration) -> io::Result<ProcOutput> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;
    let stdout = spawn_reader(child.stdout.take());
    let stderr = spawn_reader(child.stderr.take());

    let (status, timed_out) = match child.wait_timeout(timeout)? {
        Some(status) => (Some(status), false),
        None => {
            kill_tree(&mut child);
            child.wait()?;
            (None, true)
        }
    };
    // Not done: killing leftover group members after a *normal* exit. Once the leader is reaped its pid
    // could be reused, so a late `killpg` might hit an unrelated group. Leftovers that hold the output
    // pipes open only cost `READER_GRACE`.

    Ok(ProcOutput {
        status,
        timed_out,
        stdout: finish_reader(stdout),
        stderr: finish_reader(stderr),
    })
}

type Shared = Arc<Mutex<Vec<u8>>>;

fn spawn_reader<R: Read + Send + 'static>(source: Option<R>) -> (Shared, Option<JoinHandle<()>>) {
    let buffer: Shared = Arc::new(Mutex::new(Vec::new()));
    let Some(mut source) = source else {
        return (buffer, None);
    };
    let sink = Arc::clone(&buffer);
    let handle = thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match source.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let (Ok(mut guard), Some(data)) = (sink.lock(), chunk.get(..n)) {
                        guard.extend_from_slice(data);
                    }
                }
            }
        }
    });
    (buffer, Some(handle))
}

fn finish_reader((buffer, handle): (Shared, Option<JoinHandle<()>>)) -> Vec<u8> {
    if let Some(handle) = handle {
        let deadline = Instant::now() + READER_GRACE;
        while !handle.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        if handle.is_finished() {
            let _ = handle.join();
        }
    }
    buffer.lock().map(|guard| guard.clone()).unwrap_or_default()
}

#[cfg(unix)]
fn kill_tree(child: &mut Child) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;
    // The child leads its own process group, so the group id equals its pid.
    if let Ok(pid) = i32::try_from(child.id()) {
        let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
    }
    let _ = child.kill();
}

#[cfg(windows)]
fn kill_tree(child: &mut Child) {
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &child.id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

#[cfg(not(any(unix, windows)))]
fn kill_tree(child: &mut Child) {
    let _ = child.kill();
}
