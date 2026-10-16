//! `start`, `status`, `stop`, and `restart`: running SplitHello and finding it again
//! afterwards.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::net::TcpListener;

use crate::app::{AppError, Runtime};
use crate::cli::{ConfigArgs, StartArgs, StopArgs, default_cache_path};
use crate::control::{self, ControlError, ControlToken, Reply, Verb};
use crate::daemon;
use crate::instance::{self, Instance, InstanceError, Liveness, RunRecord, StatePaths};
use crate::logging::{self, LogOptions, LogTarget};
use crate::update::{self, CURRENT_VERSION, InstallMethod};
use crate::{config_path, load_config};

/// How long `stop` waits for an instance that holds its lock but has not written
/// `run.toml` yet; startup only binds sockets, so this is generous.
const STARTING_WAIT: Duration = Duration::from_secs(5);
/// On top of the drain the instance announced: closing listeners and exiting.
const STOP_GRACE: Duration = Duration::from_secs(5);
/// After SIGTERM without a reply (so without a known drain) or a forced kill.
const SIGNAL_WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(100);

/// A command's result as the process exit code. `NotRunning` (3) follows the LSB
/// convention for status queries, so scripts can tell "stopped" from "broken".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandStatus {
    Success,
    Failed,
    NotRunning,
}

impl From<bool> for CommandStatus {
    fn from(success: bool) -> Self {
        if success { Self::Success } else { Self::Failed }
    }
}

impl From<CommandStatus> for ExitCode {
    fn from(status: CommandStatus) -> Self {
        match status {
            CommandStatus::Success => Self::SUCCESS,
            CommandStatus::Failed => Self::from(1),
            CommandStatus::NotRunning => Self::from(3),
        }
    }
}

/// Runs the proxy in this process until Ctrl-C, SIGTERM, or `splithello stop`; with `-d`,
/// starts it as a background process instead.
///
/// ```text
/// lock → (log file) → config → bind listeners → bind control → write run.toml → ready → …
/// ```
///
/// `run.toml` is written before the ready line, so whoever sees `ready` can use `status`
/// and `stop` at once. A log file is opened only once the lock is held, so a second
/// `start` can never rotate the running instance's log away.
pub(crate) async fn start(
    args: &StartArgs,
    options: &LogOptions,
) -> Result<CommandStatus, AppError> {
    let path = config_path(&args.config)?;
    if args.detach {
        return daemon::start_detached(&path, args.log_file.as_deref(), &options.flags()).await;
    }
    // A new session has no controlling terminal, so closing the terminal sends no SIGHUP.
    #[cfg(unix)]
    let session = args.detached_child.then(rustix::process::setsid);
    let mut instance = Instance::acquire(StatePaths::from_env()?).await?;
    let log = match &args.log_file {
        Some(file) => Some(log_to_file(file, options)?),
        None => None,
    };
    #[cfg(unix)]
    if let Some(Err(error)) = session {
        tracing::warn!(
            error = %std::io::Error::from(error),
            "could not leave the terminal's session; closing the terminal may stop SplitHello"
        );
    }
    let Some(config) = load_config(&path)? else {
        return Ok(CommandStatus::Failed);
    };
    let check_updates = update::check_enabled(config.update().check());
    let config_file = std::path::absolute(&path).unwrap_or(path);

    let mut runtime = Runtime::start(config).await?;
    let mut record = match bind_control(&mut runtime, config_file).await {
        Ok(record) => record,
        Err(error) => {
            let _ = runtime.shutdown().await;
            return Err(error);
        }
    };
    record.log = log;
    record.flags = options.flags();
    record.managed_by = instance::managed_by(|key| std::env::var_os(key));
    if let Err(error) = instance.publish(&record) {
        let _ = runtime.shutdown().await;
        return Err(error.into());
    }
    runtime.ready().emit();
    if check_updates {
        runtime.spawn_update_notifier();
    }
    let run_result = runtime.run_until_signal().await;
    let shutdown_result = runtime.shutdown().await;
    drop(instance);
    run_result
        .and(shutdown_result)
        .map(|()| CommandStatus::Success)
}

/// Starts logging to `file` (moving the previous run's log aside) and returns its absolute
/// path for `run.toml`.
fn log_to_file(file: &Path, options: &LogOptions) -> Result<PathBuf, AppError> {
    let (handle, rotation) = logging::open_log(file).map_err(InstanceError::io("open", file))?;
    logging::init(options, LogTarget::File(handle));
    logging::log_panics();
    if let Some(error) = rotation {
        tracing::warn!(error = %error, "could not move the previous log aside; appending to it");
    }
    Ok(std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf()))
}

/// Starts the control channel and returns the record that tells others how to reach it.
async fn bind_control(runtime: &mut Runtime, config: PathBuf) -> Result<RunRecord, AppError> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(ControlError::Bind)?;
    let control = listener.local_addr().map_err(ControlError::Bind)?;
    let token = ControlToken::generate()?;
    let ready = runtime.ready();
    let record = RunRecord::new(
        config,
        ready.proxy_addr(),
        ready.pac_url().map(str::to_owned),
        control,
        token.clone(),
    );
    runtime.spawn_control(listener, token);
    Ok(record)
}

pub(crate) async fn status() -> Result<CommandStatus, AppError> {
    let record = match instance::probe(&StatePaths::from_env()?)? {
        Liveness::NotRunning { stale } => {
            println!("SplitHello is not running");
            if let Some(stale) = stale {
                println!("  last run  pid {} did not shut down cleanly", stale.pid);
            }
            println!("  start     splithello start");
            return Ok(CommandStatus::NotRunning);
        }
        Liveness::Starting => {
            println!("SplitHello is starting");
            return Ok(CommandStatus::Success);
        }
        Liveness::Running(record) => record,
    };
    let live = control::request(record.control, &record.token, Verb::Status).await;
    let latest = update::check_enabled(true)
        .then(|| default_cache_path(update::CACHE_FILE))
        .flatten()
        .and_then(|cache| update::cached_latest(&cache))
        .filter(|latest| update::is_newer(latest));
    print!(
        "{}",
        render_status(&record, live.as_ref(), unix_now(), latest.as_deref())
    );
    Ok(CommandStatus::Success)
}

fn render_status(
    record: &RunRecord,
    live: Result<&Reply, &ControlError>,
    now: u64,
    latest: Option<&str>,
) -> String {
    let uptime =
        humantime::format_duration(Duration::from_secs(now.saturating_sub(record.started_at)));
    let mut out = format!(
        "SplitHello {} running · pid {} · up {uptime}\n",
        record.version, record.pid
    );
    let mut row = |label: &str, value: &str| out.push_str(&format!("  {label:<8} {value}\n"));
    row("proxy", &record.proxy.to_string());
    row("pac", record.pac.as_deref().unwrap_or("disabled"));
    row("config", &record.config.display().to_string());
    if let Some(manager) = &record.managed_by {
        row("managed", instance::manager_name(manager));
    }
    match &record.log {
        Some(log) => row("log", &format!("{}  (splithello logs -f)", log.display())),
        None => row("log", "the terminal it was started in"),
    }
    match live {
        Ok(reply) => {
            let count = |key| reply.number(key).unwrap_or(0);
            row(
                "tunnels",
                &format!(
                    "{} open · {} split, {} direct, {} failed since start",
                    count("active"),
                    count("split"),
                    count("direct"),
                    count("failed"),
                ),
            );
        }
        Err(error) => row(
            "control",
            &format!("not answering ({})", crate::diagnostics::error_chain(error)),
        ),
    }
    if record.version != CURRENT_VERSION {
        row(
            "binary",
            &format!("{CURRENT_VERSION} is installed; `splithello restart` to use it"),
        );
    }
    if let Some(latest) = latest {
        row(
            "update",
            &format!(
                "{latest} available → {}",
                InstallMethod::current().upgrade_command()
            ),
        );
    }
    out
}

pub(crate) async fn stop(args: &StopArgs) -> Result<CommandStatus, AppError> {
    let paths = StatePaths::from_env()?;
    let Some(record) = running_record(&paths).await? else {
        if instance::discard_stale(&paths)? {
            println!("SplitHello is not running (cleared the record of one that crashed)");
        } else {
            println!("SplitHello is not running");
        }
        return Ok(CommandStatus::Success);
    };
    if let Some(manager) = &record.managed_by {
        // Stopping it here would only make the service manager start it again.
        if !args.force {
            return Err(AppError::Managed {
                manager: manager.clone(),
                restart: false,
            });
        }
        println!(
            "warning: {} may start SplitHello again",
            instance::manager_name(manager)
        );
    }
    stop_instance(&paths, &record, args.force).await?;
    println!("stopped");
    let proxy = match &record.pac {
        Some(pac) => format!("{} or {pac}", record.proxy),
        None => record.proxy.to_string(),
    };
    println!(
        "  a browser still set to {proxy} can't open HTTPS sites until SplitHello runs again; \
         remove that proxy setting to browse without it"
    );
    Ok(CommandStatus::Success)
}

/// Stops the instance `record` describes and waits until it is gone.
async fn stop_instance(
    paths: &StatePaths,
    record: &RunRecord,
    force: bool,
) -> Result<(), AppError> {
    let pid = record.pid;
    let wait = if force {
        kill(pid).map_err(|source| AppError::Kill { pid, source })?;
        println!("killed SplitHello (pid {pid})");
        SIGNAL_WAIT
    } else {
        match control::request(record.control, &record.token, Verb::Stop).await {
            Ok(reply) => {
                let drain = Duration::from_millis(reply.number("drain_ms").unwrap_or(0));
                println!(
                    "stopping SplitHello (pid {pid}); open tunnels get up to {} to finish",
                    humantime::format_duration(drain)
                );
                drain + STOP_GRACE
            }
            Err(error) if error.is_unreachable() => terminate(pid, error)?,
            Err(error) => return Err(error.into()),
        }
    };
    wait_until_stopped(paths, pid, wait).await?;
    if force {
        // A killed instance had no chance to remove it.
        instance::discard_stale(paths)?;
    }
    Ok(())
}

/// Stops the running instance and starts it again in the background, with the same config,
/// log file, and log flags unless given new ones. A config that would not start is
/// reported before anything is stopped.
pub(crate) async fn restart(
    args: &ConfigArgs,
    options: &LogOptions,
) -> Result<CommandStatus, AppError> {
    let paths = StatePaths::from_env()?;
    let previous = running_record(&paths).await?;
    if let Some(manager) = previous
        .as_ref()
        .and_then(|record| record.managed_by.clone())
    {
        return Err(AppError::Managed {
            manager,
            restart: true,
        });
    }
    let explicit_flags = options.flags();
    let (config, flags, log) = match &previous {
        Some(record) => (
            args.config.clone().unwrap_or_else(|| record.config.clone()),
            if explicit_flags.is_empty() {
                record.flags.clone()
            } else {
                explicit_flags
            },
            record.log.clone(),
        ),
        None => (config_path(args)?, explicit_flags, None),
    };
    if load_config(&config)?.is_none() {
        return Ok(CommandStatus::Failed);
    }
    match &previous {
        Some(record) => {
            stop_instance(&paths, record, false).await?;
            println!("stopped");
            if record.log.is_none() {
                println!("  it ran in a terminal; it now runs in the background");
            }
        }
        None => println!("SplitHello was not running; starting it"),
    }
    daemon::start_detached(&config, log.as_deref(), &flags).await
}

/// The running instance's record; waits briefly for one that is still starting.
async fn running_record(paths: &StatePaths) -> Result<Option<RunRecord>, AppError> {
    let deadline = tokio::time::Instant::now() + STARTING_WAIT;
    loop {
        match instance::probe(paths)? {
            Liveness::NotRunning { .. } => return Ok(None),
            Liveness::Running(record) => return Ok(Some(record)),
            Liveness::Starting if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(POLL).await;
            }
            Liveness::Starting => return Err(AppError::Starting),
        }
    }
}

/// The control channel did not answer. On Unix SIGTERM still drains like Ctrl-C; Windows has
/// no such signal for a process without a console, so only `--force` is left.
#[cfg(unix)]
fn terminate(pid: u32, error: ControlError) -> Result<Duration, AppError> {
    send_signal(pid, rustix::process::Signal::TERM)
        .map_err(|source| AppError::Kill { pid, source })?;
    println!(
        "stopping SplitHello (pid {pid}) with SIGTERM; it did not answer ({})",
        crate::diagnostics::error_chain(&error)
    );
    Ok(SIGNAL_WAIT)
}

#[cfg(not(unix))]
fn terminate(_pid: u32, error: ControlError) -> Result<Duration, AppError> {
    Err(error.into())
}

async fn wait_until_stopped(paths: &StatePaths, pid: u32, wait: Duration) -> Result<(), AppError> {
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        if matches!(instance::probe(paths)?, Liveness::NotRunning { .. }) {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::StillRunning { pid, waited: wait });
        }
        tokio::time::sleep(POLL).await;
    }
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: rustix::process::Signal) -> std::io::Result<()> {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a process id"))?;
    rustix::process::kill_process(pid, signal).map_err(std::io::Error::from)
}

#[cfg(unix)]
fn kill(pid: u32) -> std::io::Result<()> {
    send_signal(pid, rustix::process::Signal::KILL)
}

/// `taskkill /F` by absolute path, so no `PATH` entry can stand in for it. Only the exit code
/// is read: its output is localized.
#[cfg(not(unix))]
fn kill(pid: u32) -> std::io::Result<()> {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let taskkill = std::path::Path::new(&root)
        .join("System32")
        .join("taskkill.exe");
    let status = std::process::Command::new(taskkill)
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "taskkill exited with {status}"
        )))
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use crate::control::{ControlError, ControlToken, Reply};
    use crate::instance::RunRecord;
    use crate::update::CURRENT_VERSION;

    use super::render_status;

    fn record() -> RunRecord {
        let mut record = RunRecord::new(
            "/home/me/.config/splithello/config.toml".into(),
            SocketAddr::from(([127, 0, 0, 1], 8080)),
            Some("http://127.0.0.1:8081/proxy.pac".to_owned()),
            SocketAddr::from(([127, 0, 0, 1], 50_000)),
            ControlToken::generate().unwrap(),
        );
        record.pid = 4242;
        record.started_at = 1_000;
        record
    }

    #[test]
    fn status_shows_where_it_runs_and_its_traffic() {
        let reply = Reply::parse("OK pid=4242 active=2 split=120 direct=340 failed=3").unwrap();
        let mut record = record();
        record.log = Some("/home/me/.local/state/splithello/splithello.log".into());
        let text = render_status(&record, Ok(&reply), 1_000 + 7_384, None);
        assert_eq!(
            text,
            format!(
                "SplitHello {CURRENT_VERSION} running · pid 4242 · up 2h 3m 4s\n\
                 \x20 proxy    127.0.0.1:8080\n\
                 \x20 pac      http://127.0.0.1:8081/proxy.pac\n\
                 \x20 config   /home/me/.config/splithello/config.toml\n\
                 \x20 log      /home/me/.local/state/splithello/splithello.log  (splithello logs -f)\n\
                 \x20 tunnels  2 open · 120 split, 340 direct, 3 failed since start\n"
            )
        );
        record.log = None;
        let text = render_status(&record, Ok(&reply), 1_000, None);
        assert!(
            text.contains("  log      the terminal it was started in\n"),
            "{text}"
        );
    }

    #[test]
    fn status_flags_an_unreachable_instance_a_new_binary_and_an_update() {
        let mut record = record();
        record.version = "0.0.1".to_owned();
        record.pac = None;
        let error = ControlError::Timeout(std::time::Duration::from_secs(4));
        let text = render_status(&record, Err(&error), 1_000, Some("9.9.9"));
        assert!(text.contains("SplitHello 0.0.1 running"), "{text}");
        assert!(text.contains("  pac      disabled\n"), "{text}");
        assert!(
            text.contains(
                "  control  not answering (the running instance did not answer within 4s)"
            ),
            "{text}"
        );
        assert!(
            text.contains(&format!("  binary   {CURRENT_VERSION} is installed")),
            "{text}"
        );
        assert!(text.contains("  update   9.9.9 available → "), "{text}");
        assert!(
            !text.contains(record.token.expose()),
            "never print the token"
        );
    }
}
