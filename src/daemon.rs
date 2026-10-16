//! `start -d` runs SplitHello in a process of its own that outlives the terminal, and
//! `logs` reads what that process writes.
//!
//! ```text
//! start -d ── validate config ── spawn `start --log-file … --detached-child` ──┐
//!    │                                                                          │ child: new session (Unix) or
//!    │  poll ≤ 15s: run.toml names the child → print the ready banner, exit 0   │ detached process (Windows),
//!    │              the child exited        → print its last log lines, exit 1  │ logs to the file, runs on
//!    └──────────────────────────────────────────────────────────────────────────┘
//! ```

use std::fs::File;
use std::io::{ErrorKind, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use thiserror::Error;

use crate::app::{AppError, ReadyMetadata};
use crate::cli::LogsArgs;
use crate::instance::{self, InstanceError, Liveness, StatePaths};
use crate::lifecycle::CommandStatus;
use crate::load_config;

/// Startup binds sockets and reads one file; anything slower than this is stuck.
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const READY_POLL: Duration = Duration::from_millis(50);
/// Enough of the child's stderr for an error that came before its log file was open.
const MAX_CHILD_STDERR: usize = 16 * 1024;
/// Log lines shown when the background instance fails to start.
const FAILURE_LINES: usize = 20;
/// `logs` reads at most this much from the end of the file to find its lines.
const MAX_TAIL_BYTES: u64 = 8 * 1024 * 1024;
const FOLLOW_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Error)]
pub(crate) enum DaemonError {
    #[error("could not start SplitHello in the background")]
    Spawn(#[source] std::io::Error),
    #[error("the background SplitHello exited while starting ({0})")]
    ExitedEarly(ExitStatus),
    #[error("the background SplitHello was not ready within {}", humantime::format_duration(*.0))]
    NotReady(Duration),
    #[error("no log at {}", path.display())]
    NoLog { path: PathBuf },
    #[error("could not read {}", path.display())]
    ReadLog {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("SplitHello (pid {0}) logs to the terminal it was started in")]
    Foreground(u32),
}

impl DaemonError {
    pub(crate) const fn hint(&self) -> Option<&'static str> {
        match self {
            Self::ExitedEarly(_) | Self::NotReady(_) => {
                Some("its last log lines are above; `splithello logs` shows the whole log")
            }
            Self::NoLog { .. } => Some("`splithello start -d` writes one"),
            Self::Foreground(_) => Some("`splithello restart` moves it to the background"),
            Self::Spawn(_) | Self::ReadLog { .. } => None,
        }
    }
}

/// `start -d`: starts SplitHello as a background process and returns once it is ready.
/// `flags` are the global log flags the background instance should use.
pub(crate) async fn start_detached(
    config: &Path,
    log_file: Option<&Path>,
    flags: &[String],
) -> Result<CommandStatus, AppError> {
    let paths = StatePaths::from_env()?;
    refuse_if_running(&paths)?;
    // Config problems, including a new starter file, are reported here in the foreground.
    let Some(loaded) = load_config(config)? else {
        return Ok(CommandStatus::Failed);
    };
    let config = std::path::absolute(config).unwrap_or_else(|_| config.to_path_buf());
    let log = log_file.map_or_else(|| paths.log(), Path::to_path_buf);
    let log = std::path::absolute(&log).unwrap_or(log);
    instance::create_private_dir(paths.dir())?;

    let executable = std::env::current_exe().map_err(DaemonError::Spawn)?;
    let mut command = Command::new(executable);
    command
        .args(flags)
        .arg("start")
        .arg("--config")
        .arg(&config)
        .arg("--log-file")
        .arg(&log)
        .arg("--detached-child")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // Not the caller's directory: it may be removed, or unmounted, while SplitHello runs.
        .current_dir(paths.dir());
    let mut child = spawn_detached(&mut command).map_err(DaemonError::Spawn)?;
    let stderr = capture_stderr(&mut child);

    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    loop {
        if let Liveness::Running(record) = instance::probe(&paths)?
            && record.pid == child.id()
        {
            ReadyMetadata::new(
                record.proxy,
                record.pac.clone(),
                loaded.targets().len(),
                loaded.dns().upstream().host_str().unwrap_or("unknown"),
            )
            .emit();
            println!("running in the background · pid {}", record.pid);
            println!("  logs   splithello logs -f   ({})", log.display());
            println!("  stop   splithello stop");
            return Ok(CommandStatus::Success);
        }
        if let Some(status) = child.try_wait().map_err(DaemonError::Spawn)? {
            // Lost a race with another `start`: say who won rather than how this one failed.
            if let Liveness::Running(record) = instance::probe(&paths)? {
                return Err(InstanceError::AlreadyRunning {
                    pid: Some(record.pid),
                }
                .into());
            }
            report_failure(&log, &stderr);
            return Err(DaemonError::ExitedEarly(status).into());
        }
        if tokio::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            report_failure(&log, &stderr);
            return Err(DaemonError::NotReady(READY_TIMEOUT).into());
        }
        tokio::time::sleep(READY_POLL).await;
    }
}

pub(crate) fn refuse_if_running(paths: &StatePaths) -> Result<(), InstanceError> {
    match instance::probe(paths)? {
        Liveness::NotRunning { .. } => Ok(()),
        Liveness::Starting => Err(InstanceError::AlreadyRunning { pid: None }),
        Liveness::Running(record) => Err(InstanceError::AlreadyRunning {
            pid: Some(record.pid),
        }),
    }
}

/// On Unix the child leaves the terminal's session itself (`setsid` on `--detached-child`),
/// so a closed terminal sends it no SIGHUP.
#[cfg(unix)]
fn spawn_detached(command: &mut Command) -> std::io::Result<Child> {
    command.spawn()
}

/// No console (so closing the terminal does not end it) and its own process group (so the
/// terminal's Ctrl-C does not reach it). Some terminals put their processes in a job that
/// is closed with the terminal; breaking away from it is refused when the job forbids it.
#[cfg(windows)]
fn spawn_detached(command: &mut Command) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt as _;

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    const ERROR_ACCESS_DENIED: i32 = 5;

    keep_std_handles_from_children();
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
    match command.spawn() {
        Err(error) if error.raw_os_error() == Some(ERROR_ACCESS_DENIED) => {
            command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
            let child = command.spawn()?;
            tracing::warn!(
                "this terminal keeps its processes in a job; SplitHello may stop when the \
                 terminal closes"
            );
            Ok(child)
        }
        result => result,
    }
}

/// `CreateProcess` hands a child every inheritable handle, not only its stdio. Our own
/// stdout and stderr are inheritable when the caller gave us pipes, so the instance would
/// hold them open and `splithello start -d | …` would never see EOF. Unix closes them on
/// exec; here they have to be marked. `Stdio::inherit` duplicates the handle it passes, so
/// later spawns are unaffected. `tests/background.rs` reads `start -d` through pipes.
#[cfg(windows)]
fn keep_std_handles_from_children() {
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};

    let handles = [
        std::io::stdin().as_raw_handle(),
        std::io::stdout().as_raw_handle(),
        std::io::stderr().as_raw_handle(),
    ];
    for handle in handles.into_iter().filter(|handle| !handle.is_null()) {
        // SAFETY: a standard handle of this process stays open for its lifetime, and
        // clearing HANDLE_FLAG_INHERIT only changes what `CreateProcess` passes on. Failure
        // (a console pseudo-handle on old Windows) leaves the handle as it was.
        unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
    }
}

/// Collects the child's stderr in the background: anything it says before its log file is
/// open (or if opening it fails) lands here.
fn capture_stderr(child: &mut Child) -> Arc<Mutex<Vec<u8>>> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut stderr) = child.stderr.take() {
        let captured = Arc::clone(&captured);
        std::thread::spawn(move || {
            let mut chunk = [0_u8; 1024];
            while let Ok(read) = stderr.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                let mut captured = captured.lock().unwrap_or_else(|poison| poison.into_inner());
                let room = MAX_CHILD_STDERR.saturating_sub(captured.len());
                captured.extend_from_slice(&chunk[..read.min(room)]);
            }
        });
    }
    captured
}

fn report_failure(log: &Path, stderr: &Mutex<Vec<u8>>) {
    // The reader thread ends at EOF, right after the exit; give it that moment.
    std::thread::sleep(READY_POLL);
    let stderr = stderr.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut out = std::io::stderr().lock();
    if !stderr.is_empty() {
        let _ = out.write_all(&stderr);
    }
    if let Ok(lines) = tail(log, FAILURE_LINES)
        && !lines.is_empty()
    {
        let _ = writeln!(out, "--- last lines of {} ---", log.display());
        let _ = out.write_all(&lines);
    }
}

/// `logs`: the running instance's log, else the last background run's.
pub(crate) async fn logs(args: &LogsArgs) -> Result<CommandStatus, AppError> {
    let paths = StatePaths::from_env()?;
    let path = match instance::probe(&paths)? {
        Liveness::Running(record) => record.log.ok_or(DaemonError::Foreground(record.pid))?,
        Liveness::NotRunning { .. } | Liveness::Starting => paths.log(),
    };
    let lines = tail(&path, usize::from(args.lines)).map_err(|source| match source.kind() {
        ErrorKind::NotFound => DaemonError::NoLog { path: path.clone() },
        _ => DaemonError::ReadLog {
            path: path.clone(),
            source,
        },
    })?;
    let mut position = file_len(&path);
    if !write_out(&lines) {
        return Ok(CommandStatus::Success);
    }
    if args.follow {
        let interrupted = tokio::signal::ctrl_c();
        tokio::pin!(interrupted);
        loop {
            tokio::select! {
                _ = &mut interrupted => break,
                () = tokio::time::sleep(FOLLOW_POLL) => {}
            }
            let length = file_len(&path);
            if length < position {
                // Truncated, or replaced by a new run's log after `restart`.
                if !write_out(b"-- log restarted --\n") {
                    break;
                }
                position = 0;
            }
            if length > position {
                let fresh = read_range(&path, position, length).unwrap_or_default();
                position += fresh.len() as u64;
                if !write_out(&fresh) {
                    break;
                }
            }
        }
    }
    Ok(CommandStatus::Success)
}

/// Writes to stdout; `false` once the reader is gone (`splithello logs | head`).
fn write_out(bytes: &[u8]) -> bool {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(bytes)
        .and_then(|()| stdout.flush())
        .is_ok()
}

fn file_len(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |metadata| metadata.len())
}

fn read_range(path: &Path, start: u64, end: u64) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(end.saturating_sub(start))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The last `count` lines of `path`, read from at most its last [`MAX_TAIL_BYTES`].
fn tail(path: &Path, count: usize) -> std::io::Result<Vec<u8>> {
    let length = std::fs::metadata(path)?.len();
    let start = length.saturating_sub(MAX_TAIL_BYTES);
    let mut bytes = read_range(path, start, length)?;
    if start > 0 {
        // Starts mid-line; drop the fragment.
        let first = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(0, |end| end + 1);
        bytes.drain(..first);
    }
    Ok(last_lines(&bytes, count).to_vec())
}

fn last_lines(bytes: &[u8], count: usize) -> &[u8] {
    // A final line without a newline still counts as a line.
    let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let start = body
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, byte)| **byte == b'\n')
        .nth(count.saturating_sub(1))
        .map_or(0, |(index, _)| index + 1);
    if count == 0 {
        &bytes[bytes.len()..]
    } else {
        &bytes[start..]
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{last_lines, tail};

    #[test]
    fn last_lines_counts_from_the_end() {
        let text = b"one\ntwo\nthree\n";
        assert_eq!(last_lines(text, 2), b"two\nthree\n");
        assert_eq!(last_lines(text, 3), text);
        assert_eq!(last_lines(text, 10), text);
        assert_eq!(last_lines(b"one\ntwo", 1), b"two");
        assert_eq!(last_lines(b"", 5), b"");
        assert_eq!(last_lines(text, 0), b"");
    }

    #[test]
    fn tail_reads_a_file_and_drops_a_partial_first_line() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("splithello.log");
        fs::write(&path, "a\nb\nc\n").unwrap();
        assert_eq!(tail(&path, 2).unwrap(), b"b\nc\n");
        assert!(tail(&directory.path().join("missing.log"), 2).is_err());
    }
}
