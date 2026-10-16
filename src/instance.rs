//! The running instance's footprint in the state directory, so `status` and `stop` can find
//! it however it was started:
//!
//! ```text
//! splithello.lock  empty; the instance holds an exclusive lock on it for its whole life
//! run.toml         pid, addresses, and control token; only the lock holder writes or removes it
//! ```
//!
//! The OS releases the lock when the process dies, so a crash can never leave SplitHello
//! looking alive, and a leftover `run.toml` is recognizably stale. The lock file itself is
//! never deleted: removing it would let two processes each lock a different file.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write as _};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::control::ControlToken;

const LOCK_FILE: &str = "splithello.lock";
const RUN_FILE: &str = "run.toml";
const RUN_FORMAT: u32 = 1;
/// `status` and `stop` probe with a brief shared lock; a starting instance waits this long
/// for such a probe to let go (and, on Windows, for a dead holder's lock to be released).
const ACQUIRE_PATIENCE: Duration = Duration::from_secs(1);
const ACQUIRE_RETRY: Duration = Duration::from_millis(50);

#[derive(Debug, Error)]
pub(crate) enum InstanceError {
    #[error("could not determine a state directory; set HOME or XDG_STATE_HOME")]
    NoStateDir,
    #[error("could not {action} {}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{} does not support file locks", path.display())]
    LockUnsupported { path: PathBuf },
    #[error("SplitHello is already running{}", pid_suffix(*.pid))]
    AlreadyRunning { pid: Option<u32> },
    #[error("could not record the running instance in {}", path.display())]
    Encode {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
}

fn pid_suffix(pid: Option<u32>) -> String {
    pid.map(|pid| format!(" (pid {pid})")).unwrap_or_default()
}

impl InstanceError {
    fn io(action: &'static str, path: &Path) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io {
            action,
            path,
            source,
        }
    }

    pub(crate) const fn hint(&self) -> Option<&'static str> {
        match self {
            Self::AlreadyRunning { .. } => {
                Some("`splithello status` shows it; `splithello stop` stops it")
            }
            Self::LockUnsupported { .. } => {
                Some("set XDG_STATE_HOME to a directory on a local disk")
            }
            Self::NoStateDir | Self::Io { .. } | Self::Encode { .. } => None,
        }
    }
}

/// Where the running instance's files live.
#[derive(Clone, Debug)]
pub(crate) struct StatePaths {
    dir: PathBuf,
}

impl StatePaths {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// `$XDG_STATE_HOME/splithello`, else `~/.local/state/splithello`.
    pub(crate) fn from_env() -> Result<Self, InstanceError> {
        crate::cli::default_state_dir()
            .map(Self::new)
            .ok_or(InstanceError::NoStateDir)
    }

    fn lock(&self) -> PathBuf {
        self.dir.join(LOCK_FILE)
    }

    pub(crate) fn run(&self) -> PathBuf {
        self.dir.join(RUN_FILE)
    }
}

/// What `status` and `stop` need to find and address the running instance.
/// Not `deny_unknown_fields`: another SplitHello version may read a newer file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct RunRecord {
    pub(crate) format: u32,
    pub(crate) pid: u32,
    pub(crate) version: String,
    /// Unix seconds.
    pub(crate) started_at: u64,
    pub(crate) config: PathBuf,
    pub(crate) proxy: SocketAddr,
    pub(crate) pac: Option<String>,
    pub(crate) control: SocketAddr,
    pub(crate) token: ControlToken,
}

impl RunRecord {
    pub(crate) fn new(
        config: PathBuf,
        proxy: SocketAddr,
        pac: Option<String>,
        control: SocketAddr,
        token: ControlToken,
    ) -> Self {
        Self {
            format: RUN_FORMAT,
            pid: std::process::id(),
            version: crate::update::CURRENT_VERSION.to_owned(),
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
            config,
            proxy,
            pac,
            control,
            token,
        }
    }
}

/// Proof that this process is the one running SplitHello. Dropping it removes `run.toml`,
/// then releases the lock.
#[derive(Debug)]
pub(crate) struct Instance {
    paths: StatePaths,
    published: bool,
    /// Declared last so it closes, and unlocks, after `drop` removed `run.toml`.
    _lock: File,
}

impl Instance {
    pub(crate) async fn acquire(paths: StatePaths) -> Result<Self, InstanceError> {
        create_private_dir(&paths.dir)?;
        let lock_path = paths.lock();
        let lock = private_options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(InstanceError::io("open", &lock_path))?;
        let deadline = tokio::time::Instant::now() + ACQUIRE_PATIENCE;
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(TryLockError::WouldBlock) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(ACQUIRE_RETRY).await;
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(InstanceError::AlreadyRunning {
                        pid: read_record(&paths.run()).map(|record| record.pid),
                    });
                }
                Err(TryLockError::Error(error)) if error.kind() == ErrorKind::Unsupported => {
                    return Err(InstanceError::LockUnsupported { path: lock_path });
                }
                Err(TryLockError::Error(error)) => {
                    return Err(InstanceError::io("lock", &lock_path)(error));
                }
            }
        }
        // Whatever is there was left by an instance that did not shut down cleanly.
        remove_if_present(&paths.run())?;
        Ok(Self {
            paths,
            published: false,
            _lock: lock,
        })
    }

    /// Writes `run.toml`. It replaces the old file in one step, so a reader never sees half.
    pub(crate) fn publish(&mut self, record: &RunRecord) -> Result<(), InstanceError> {
        let path = self.paths.run();
        let text = toml::to_string(record).map_err(|source| InstanceError::Encode {
            path: path.clone(),
            source,
        })?;
        let temporary = self.paths.dir.join(format!("{RUN_FILE}.tmp"));
        // A leftover would keep its old permissions; a fresh file gets owner-only ones.
        remove_if_present(&temporary)?;
        private_options()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(InstanceError::io("write", &temporary))?;
        fs::rename(&temporary, &path).map_err(InstanceError::io("write", &path))?;
        self.published = true;
        Ok(())
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if self.published {
            let _ = fs::remove_file(self.paths.run());
        }
    }
}

/// What the state directory says about a running instance.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Liveness {
    /// Nothing holds the lock. `stale` is a `run.toml` left by a crashed instance.
    NotRunning {
        stale: Option<RunRecord>,
    },
    /// The lock is held but `run.toml` is not written yet.
    Starting,
    Running(RunRecord),
}

/// Checks for a running instance without disturbing it: a shared lock succeeds only while
/// no instance holds its exclusive one, and is released at once.
pub(crate) fn probe(paths: &StatePaths) -> Result<Liveness, InstanceError> {
    let lock_path = paths.lock();
    let lock = match File::open(&lock_path) {
        Ok(lock) => lock,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(Liveness::NotRunning { stale: None });
        }
        Err(error) => return Err(InstanceError::io("open", &lock_path)(error)),
    };
    match lock.try_lock_shared() {
        Ok(()) => Ok(Liveness::NotRunning {
            stale: read_record(&paths.run()),
        }),
        Err(TryLockError::WouldBlock) => {
            Ok(read_record(&paths.run()).map_or(Liveness::Starting, Liveness::Running))
        }
        Err(TryLockError::Error(error)) if error.kind() == ErrorKind::Unsupported => {
            Err(InstanceError::LockUnsupported { path: lock_path })
        }
        Err(TryLockError::Error(error)) => Err(InstanceError::io("lock", &lock_path)(error)),
    }
}

/// Removes a `run.toml` left by a crashed instance. Holding the shared lock while doing so
/// keeps a starting instance from publishing in between. Returns whether one was removed.
pub(crate) fn discard_stale(paths: &StatePaths) -> Result<bool, InstanceError> {
    let lock_path = paths.lock();
    let lock = match File::open(&lock_path) {
        Ok(lock) => lock,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(InstanceError::io("open", &lock_path)(error)),
    };
    match lock.try_lock_shared() {
        Ok(()) => remove_if_present(&paths.run()),
        Err(_) => Ok(false),
    }
}

/// A missing or unreadable file reads as absent: it cannot be acted on either way.
fn read_record(path: &Path) -> Option<RunRecord> {
    toml::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn remove_if_present(path: &Path) -> Result<bool, InstanceError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(InstanceError::io("remove", path)(error)),
    }
}

/// Owner-only on Unix: `run.toml` holds the control token. On Windows the files inherit the
/// user profile's owner-only access.
fn create_private_dir(dir: &Path) -> Result<(), InstanceError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(dir)
        .map_err(InstanceError::io("create", dir))
}

#[cfg(unix)]
pub(crate) fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options
}

#[cfg(not(unix))]
pub(crate) fn private_options() -> OpenOptions {
    OpenOptions::new()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::SocketAddr;

    use tempfile::tempdir;

    use crate::control::ControlToken;

    use super::{Instance, InstanceError, Liveness, RunRecord, StatePaths, discard_stale, probe};

    fn record(pid: u32) -> RunRecord {
        let address = SocketAddr::from(([127, 0, 0, 1], 8080));
        let mut record = RunRecord::new(
            "/config.toml".into(),
            address,
            Some("http://127.0.0.1:8081/proxy.pac".to_owned()),
            address,
            ControlToken::generate().unwrap(),
        );
        record.pid = pid;
        record
    }

    fn paths() -> (tempfile::TempDir, StatePaths) {
        let directory = tempdir().unwrap();
        let paths = StatePaths::new(directory.path().join("state").join("splithello"));
        (directory, paths)
    }

    #[tokio::test]
    async fn only_one_instance_holds_the_lock_and_dropping_releases_it() {
        let (_directory, paths) = paths();
        assert_eq!(probe(&paths).unwrap(), Liveness::NotRunning { stale: None });

        let mut first = Instance::acquire(paths.clone()).await.unwrap();
        assert_eq!(probe(&paths).unwrap(), Liveness::Starting);
        first.publish(&record(42)).unwrap();
        assert_eq!(
            probe(&paths).unwrap(),
            Liveness::Running(record_with_pid(&paths, 42))
        );

        let error = Instance::acquire(paths.clone()).await.unwrap_err();
        assert!(matches!(
            error,
            InstanceError::AlreadyRunning { pid: Some(42) }
        ));
        assert_eq!(error.to_string(), "SplitHello is already running (pid 42)");

        drop(first);
        assert!(!paths.run().exists(), "a clean exit removes run.toml");
        assert_eq!(probe(&paths).unwrap(), Liveness::NotRunning { stale: None });
        Instance::acquire(paths).await.unwrap();
    }

    fn record_with_pid(paths: &StatePaths, pid: u32) -> RunRecord {
        let record = super::read_record(&paths.run()).unwrap();
        assert_eq!(record.pid, pid);
        record
    }

    #[tokio::test]
    async fn a_stale_record_is_reported_then_replaced() {
        let (_directory, paths) = paths();
        let mut crashed = Instance::acquire(paths.clone()).await.unwrap();
        crashed.publish(&record(7)).unwrap();
        // A crash: the lock goes away with the process, run.toml stays.
        crashed.published = false;
        drop(crashed);

        let Liveness::NotRunning { stale: Some(stale) } = probe(&paths).unwrap() else {
            panic!("expected a stale record");
        };
        assert_eq!(stale.pid, 7);

        let next = Instance::acquire(paths.clone()).await.unwrap();
        assert!(!paths.run().exists(), "the next instance discards it");
        drop(next);

        fs::write(paths.run(), toml::to_string(&record(8)).unwrap()).unwrap();
        assert!(discard_stale(&paths).unwrap());
        assert!(!paths.run().exists());
        assert!(!discard_stale(&paths).unwrap());
    }

    #[tokio::test]
    async fn a_probe_in_progress_does_not_turn_away_a_starting_instance() {
        let (_directory, paths) = paths();
        drop(Instance::acquire(paths.clone()).await.unwrap());
        let probe_lock = fs::File::open(paths.dir.join(super::LOCK_FILE)).unwrap();
        probe_lock.try_lock_shared().unwrap();
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            drop(probe_lock);
        });
        Instance::acquire(paths).await.unwrap();
        releaser.join().unwrap();
    }

    #[tokio::test]
    async fn a_held_lock_without_a_record_cannot_be_discarded() {
        let (_directory, paths) = paths();
        let _running = Instance::acquire(paths.clone()).await.unwrap();
        fs::write(paths.run(), "not toml at all").unwrap();
        assert_eq!(
            probe(&paths).unwrap(),
            Liveness::Starting,
            "corrupt reads as absent"
        );
        assert!(!discard_stale(&paths).unwrap());
        assert!(paths.run().exists());
    }

    #[test]
    fn records_round_trip_and_tolerate_new_fields() {
        let record = record(9);
        let text = toml::to_string(&record).unwrap();
        assert_eq!(toml::from_str::<RunRecord>(&text).unwrap(), record);
        let newer = format!("{text}future_field = 1\n");
        assert_eq!(toml::from_str::<RunRecord>(&newer).unwrap(), record);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn state_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let (_directory, paths) = paths();
        let mut instance = Instance::acquire(paths.clone()).await.unwrap();
        instance.publish(&record(1)).unwrap();
        let mode =
            |path: &std::path::Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&paths.dir), 0o700);
        assert_eq!(mode(&paths.run()), 0o600);
        assert_eq!(mode(&paths.lock()), 0o600);
    }
}
