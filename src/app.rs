use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::net::TcpListener;
use tokio::task::{JoinError, JoinSet};
use tokio::time::{Instant, timeout_at};
use tokio_util::sync::CancellationToken;

use crate::cli::default_cache_path;
use crate::config::{Config, ConfigError};
use crate::control::{ControlError, ControlServer, ControlToken};
use crate::daemon::DaemonError;
use crate::diagnostics::{Outcome, ProxyStats, log_summary};
use crate::dns::{DohResolver, ResolveError};
use crate::domain::{DomainError, DomainName};
use crate::instance::InstanceError;
use crate::pac::{PacServer, PacServiceError, render_pac};
use crate::proxy::{ProxyLimits, ProxyServer, ProxyServiceError, ProxyState};
use crate::update::{self, ReleaseSource, UpdateError};

const SUMMARY_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadyMetadata {
    proxy_addr: SocketAddr,
    pac_url: Option<String>,
    target_count: usize,
    doh_host: String,
}

impl ReadyMetadata {
    /// For `start -d`, which announces readiness on behalf of the background process.
    pub(crate) fn new(
        proxy_addr: SocketAddr,
        pac_url: Option<String>,
        target_count: usize,
        doh_host: &str,
    ) -> Self {
        Self {
            proxy_addr,
            pac_url,
            target_count,
            doh_host: doh_host.to_owned(),
        }
    }

    pub(crate) fn proxy_addr(&self) -> SocketAddr {
        self.proxy_addr
    }

    pub(crate) fn pac_url(&self) -> Option<&str> {
        self.pac_url.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn target_count(&self) -> usize {
        self.target_count
    }

    #[cfg(test)]
    pub(crate) fn doh_host(&self) -> &str {
        &self.doh_host
    }

    pub(crate) const fn transport(&self) -> &'static str {
        "IPv4 TCP only for selected targets"
    }

    pub(crate) const fn outcome(&self) -> Outcome {
        Outcome::Ready
    }

    pub(crate) fn emit(&self) {
        tracing::info!(
            outcome = %self.outcome(),
            proxy_addr = %self.proxy_addr,
            pac_url = self.pac_url.as_deref().unwrap_or("disabled"),
            target_count = self.target_count,
            doh_host = %self.doh_host,
            transport = self.transport(),
            "SplitHello is ready"
        );
    }
}

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("could not determine a configuration path; set HOME or pass --config <PATH>")]
    NoConfigPath,
    #[error("could not write starter configuration {}: {source}", path.display())]
    ConfigWrite {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not construct authenticated target resolver")]
    Resolver(#[source] ResolveError),
    #[error("could not compile target policy")]
    Domain(#[source] DomainError),
    #[error("could not bind proxy listener {address}: {source}")]
    ProxyBind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("could not bind PAC listener {address}: {source}")]
    PacBind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("could not inspect a bound listener")]
    ListenerAddress(#[source] std::io::Error),
    #[error(transparent)]
    ProxyService(#[from] ProxyServiceError),
    #[error(transparent)]
    PacService(#[from] PacServiceError),
    #[error("runtime service stopped unexpectedly")]
    ServiceStopped,
    #[error("runtime task failed")]
    Task(#[source] JoinError),
    #[error(transparent)]
    Instance(#[from] InstanceError),
    #[error(transparent)]
    Control(#[from] ControlError),
    #[error(transparent)]
    Daemon(#[from] DaemonError),
    #[error("SplitHello is starting but has not said how to reach it yet")]
    Starting,
    #[error("SplitHello (pid {pid}) is still running after {}", humantime::format_duration(*.waited))]
    StillRunning { pid: u32, waited: Duration },
    #[error(
        "SplitHello runs as a {} service, which would start it again",
        crate::instance::manager_name(.manager)
    )]
    Managed { manager: String, restart: bool },
    #[error("could not stop SplitHello (pid {pid})")]
    Kill {
        pid: u32,
        #[source]
        source: std::io::Error,
    },
    #[error("update failed: {0}")]
    Update(#[from] UpdateError),
    #[error("could not ask for confirmation")]
    Prompt(#[source] std::io::Error),
}

impl AppError {
    pub(crate) const fn outcome(&self) -> Outcome {
        match self {
            Self::Config(_) | Self::NoConfigPath => Outcome::ConfigError,
            Self::ConfigWrite { .. } => Outcome::InternalError,
            Self::Resolver(_) => Outcome::DnsError,
            Self::Domain(_) => Outcome::ConfigError,
            Self::ProxyBind { .. }
            | Self::PacBind { .. }
            | Self::ListenerAddress(_)
            | Self::ProxyService(_)
            | Self::PacService(_) => Outcome::ConnectError,
            Self::ServiceStopped | Self::Task(_) | Self::Prompt(_) => Outcome::InternalError,
            // Like a busy port: something else already holds what `start` needs.
            Self::Instance(InstanceError::AlreadyRunning { .. }) => Outcome::ConnectError,
            Self::Instance(_) => Outcome::InternalError,
            Self::Control(ControlError::Random(_) | ControlError::Bind(_)) => {
                Outcome::InternalError
            }
            Self::Control(_) => Outcome::ConnectError,
            Self::Daemon(_) => Outcome::InternalError,
            Self::Starting | Self::StillRunning { .. } | Self::Kill { .. } => {
                Outcome::InternalError
            }
            Self::Managed { .. } => Outcome::ConfigError,
            Self::Update(_) => Outcome::ConnectError,
        }
    }

    /// What the user can do about it; the outcome's hint unless the error knows better.
    pub(crate) fn hint(&self) -> Option<&'static str> {
        match self {
            Self::Managed { manager, restart } => Some(match (manager.as_str(), restart) {
                ("homebrew", false) => {
                    "`brew services stop splithello`; or `splithello stop --force` until it is \
                     started again"
                }
                ("homebrew", true) => "`brew services restart splithello`",
                (_, false) => "stop it through that service manager, or `splithello stop --force`",
                (_, true) => "restart it through that service manager",
            }),
            Self::Update(_) => Some(
                "check the connection, or download the release from \
                 https://github.com/therealhieu/splithello/releases",
            ),
            Self::Instance(error) => match error.hint() {
                Some(hint) => Some(hint),
                None => self.outcome().hint(),
            },
            Self::Daemon(error) => error.hint(),
            Self::ProxyBind { .. } | Self::PacBind { .. } => Some(
                "another program uses that port; stop it, or set [proxy].listen / [proxy].pac \
                 to a free one",
            ),
            Self::Control(ControlError::Rejected(_)) => Some(
                "run.toml does not match the running instance; \
                 `splithello stop --force` stops it anyway",
            ),
            Self::Control(ControlError::Connect { .. } | ControlError::Timeout(_)) => {
                Some("`splithello stop --force` stops it without draining")
            }
            Self::StillRunning { .. } => Some("`splithello stop --force` stops it at once"),
            Self::Starting => Some("try again in a moment"),
            _ => self.outcome().hint(),
        }
    }
}

pub(crate) struct Runtime {
    cancel: CancellationToken,
    tasks: JoinSet<Result<(), AppError>>,
    shutdown_timeout: Duration,
    ready: ReadyMetadata,
    resolver: Arc<DohResolver>,
    stats: Arc<ProxyStats>,
}

impl Runtime {
    pub(crate) async fn start(config: Config) -> Result<Self, AppError> {
        let matcher = crate::domain::TargetMatcher::new(config.targets().to_vec())
            .map_err(AppError::Domain)?;
        let target_resolver = Arc::new(DohResolver::new(config.dns()).map_err(AppError::Resolver)?);
        let limits = ProxyLimits {
            max_connect_header_bytes: config.limits().max_connect_header_bytes(),
            max_client_hello_bytes: config.limits().max_client_hello_bytes(),
            connect_timeout: config.limits().connect_timeout(),
            client_hello_timeout: config.limits().client_hello_timeout(),
            idle_timeout: config.limits().idle_timeout(),
        };
        let state = Arc::new(ProxyState::new(
            matcher,
            target_resolver.clone(),
            config.proxy().allowed_ports().iter().copied(),
            limits,
            config.limits().max_connections(),
        ));
        let stats = state.stats();
        let cancel = CancellationToken::new();
        let shutdown_timeout = config.limits().shutdown_timeout();

        let proxy_listener =
            TcpListener::bind(config.proxy().listen())
                .await
                .map_err(|source| AppError::ProxyBind {
                    address: config.proxy().listen(),
                    source,
                })?;
        let proxy_addr = proxy_listener
            .local_addr()
            .map_err(AppError::ListenerAddress)?;
        let pac_listener = match config.proxy().pac_listen() {
            Some(address) => Some(
                TcpListener::bind(address)
                    .await
                    .map_err(|source| AppError::PacBind { address, source })?,
            ),
            None => None,
        };
        let pac_addr = pac_listener
            .as_ref()
            .map(TcpListener::local_addr)
            .transpose()
            .map_err(AppError::ListenerAddress)?;

        let pac_body = pac_listener
            .as_ref()
            .map(|_| Arc::<str>::from(render_pac(proxy_addr, config.targets())));
        let ready = ReadyMetadata {
            proxy_addr,
            pac_url: pac_addr.map(|address| format!("http://{address}/proxy.pac")),
            target_count: config.targets().len(),
            doh_host: config
                .dns()
                .upstream()
                .host_str()
                .unwrap_or("unknown")
                .to_owned(),
        };

        let mut tasks = JoinSet::new();
        let proxy = ProxyServer::new(
            proxy_listener,
            state,
            cancel.child_token(),
            shutdown_timeout,
        );
        tasks.spawn(async move { proxy.run().await.map_err(AppError::from) });
        if let (Some(listener), Some(body)) = (pac_listener, pac_body) {
            let pac = PacServer::new(
                listener,
                body,
                cancel.child_token(),
                config.limits().max_connect_header_bytes(),
                config.limits().idle_timeout(),
                shutdown_timeout,
                config.limits().max_connections(),
            );
            tasks.spawn(async move { pac.run().await.map_err(AppError::from) });
        }
        tasks.spawn(report_summaries(Arc::clone(&stats), cancel.child_token()));
        // Detached: it finishes on its own, which `tasks` would read as a stopped service.
        tokio::spawn(prewarm_targets(
            target_resolver.clone(),
            config
                .targets()
                .iter()
                .map(|target| target.host().clone())
                .collect(),
            cancel.child_token(),
        ));

        Ok(Self {
            cancel,
            tasks,
            shutdown_timeout,
            ready,
            resolver: target_resolver,
            stats,
        })
    }

    pub(crate) fn ready(&self) -> &ReadyMetadata {
        &self.ready
    }

    /// Serves `status` and `stop` on `listener`. A `STOP` cancels the whole runtime, so
    /// shutdown drains exactly as on Ctrl-C.
    pub(crate) fn spawn_control(&mut self, listener: TcpListener, token: ControlToken) {
        let server = ControlServer::new(
            listener,
            token,
            Arc::clone(&self.stats),
            self.cancel.clone(),
            self.shutdown_timeout,
        );
        self.tasks.spawn(async move {
            server.run().await;
            Ok(())
        });
    }

    /// Starts the daily check for a newer release. Detached like the prewarm: it never
    /// stops the proxy, and a failure only logs at DEBUG.
    pub(crate) fn spawn_update_notifier(&self) {
        match ReleaseSource::github(self.resolver.clone()) {
            Ok(source) => {
                tokio::spawn(update::notify(
                    source,
                    default_cache_path(update::CACHE_FILE),
                    self.cancel.child_token(),
                ));
            }
            Err(error) => tracing::debug!(error = %error, "update check unavailable"),
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
    }

    pub(crate) async fn run_until_signal(&mut self) -> Result<(), AppError> {
        let signal = shutdown_signal();
        tokio::pin!(signal);
        let mut listening = true;
        loop {
            tokio::select! {
                // Cancellation first: a `STOP` makes services finish with `Ok`, which the
                // `join_next` branch would otherwise report as an unexpected stop.
                biased;
                _ = self.cancel.cancelled() => return Ok(()),
                signal = &mut signal, if listening => match signal {
                    Ok(()) => {
                        self.cancel.cancel();
                        return Ok(());
                    }
                    // A detached process has no console on Windows; the control channel
                    // still stops it, so this must not take the proxy down.
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "could not listen for Ctrl-C; `splithello stop` stops SplitHello"
                        );
                        listening = false;
                    }
                },
                joined = self.tasks.join_next(), if !self.tasks.is_empty() => {
                    return match joined {
                        Some(Ok(Ok(()))) => Err(AppError::ServiceStopped),
                        Some(Ok(Err(error))) => Err(error),
                        Some(Err(error)) => Err(AppError::Task(error)),
                        None => Err(AppError::ServiceStopped),
                    };
                }
            }
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<(), AppError> {
        self.cancel.cancel();
        drain_tasks(&mut self.tasks, self.shutdown_timeout).await
    }
}

/// Logs one summary line per window that saw traffic, so a quiet terminal still shows
/// the proxy is alive and how traffic splits between targets and direct relays.
async fn report_summaries(
    stats: Arc<ProxyStats>,
    cancel: CancellationToken,
) -> Result<(), AppError> {
    let mut interval = tokio::time::interval_at(Instant::now() + SUMMARY_WINDOW, SUMMARY_WINDOW);
    let mut previous = stats.snapshot();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            _ = interval.tick() => {
                let now = stats.snapshot();
                let window = now.since(previous);
                previous = now;
                if window.tunnels() > 0 {
                    log_summary(SUMMARY_WINDOW, window);
                }
            }
        }
    }
}

/// Resolves every target apex once in the background, so the first browser request
/// skips the DoH connection setup and lookup. Best effort: failures are logged at DEBUG.
async fn prewarm_targets(
    resolver: Arc<DohResolver>,
    hosts: Vec<DomainName>,
    cancel: CancellationToken,
) {
    let mut lookups = JoinSet::new();
    for host in hosts {
        let resolver = resolver.clone();
        lookups.spawn(async move { resolver.prewarm(&host).await });
    }
    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = lookups.join_all() => {}
    }
}

async fn drain_tasks(
    tasks: &mut JoinSet<Result<(), AppError>>,
    shutdown_timeout: Duration,
) -> Result<(), AppError> {
    let deadline = Instant::now() + shutdown_timeout;
    let mut first_error = None;
    while !tasks.is_empty() {
        match timeout_at(deadline, tasks.join_next()).await {
            Ok(Some(Ok(Ok(())))) => {}
            Ok(Some(Ok(Err(error)))) => {
                first_error.get_or_insert(error);
            }
            Ok(Some(Err(error))) => {
                first_error.get_or_insert_with(|| AppError::Task(error));
            }
            Ok(None) => break,
            Err(_) => {
                tasks.abort_all();
                while let Some(joined) = tasks.join_next().await {
                    match joined {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            first_error.get_or_insert(error);
                        }
                        Err(error) if error.is_cancelled() => {}
                        Err(error) => {
                            first_error.get_or_insert_with(|| AppError::Task(error));
                        }
                    }
                }
                break;
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(unix)]
async fn shutdown_signal() -> Result<(), std::io::Error> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result,
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() -> Result<(), std::io::Error> {
    tokio::signal::ctrl_c().await
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use tempfile::tempdir;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::JoinSet;
    use tokio::time::{Instant, timeout};

    use crate::config::Config;
    use crate::control::{ControlToken, Verb, request};
    use crate::diagnostics::Outcome;

    use super::{AppError, Runtime, drain_tasks};

    fn config_text(proxy: &str, pac: Option<&str>, shutdown_seconds: u64) -> String {
        let pac = pac.map_or_else(|| "false".to_owned(), |address| format!("\"{address}\""));
        format!(
            "targets = [\"reddit.com\"]\n\n[proxy]\nlisten = \"{proxy}\"\npac = {pac}\n\n[limits]\nshutdown_timeout = \"{shutdown_seconds}s\"\n"
        )
    }

    fn load_config(text: &str) -> Config {
        let directory = tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(&path, text).unwrap();
        Config::load(&path).unwrap()
    }

    #[tokio::test]
    async fn start_binds_all_listeners_before_ready_and_exposes_actual_metadata() {
        let proxy_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_reservation.local_addr().unwrap();
        let pac_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pac_addr = pac_reservation.local_addr().unwrap();
        drop((proxy_reservation, pac_reservation));
        let config = load_config(&config_text(
            &proxy_addr.to_string(),
            Some(&pac_addr.to_string()),
            1,
        ));
        let runtime = Runtime::start(config).await.unwrap();
        let ready = runtime.ready();
        assert_eq!(ready.outcome(), Outcome::Ready);
        assert_ne!(ready.proxy_addr().port(), 0);
        assert!(
            ready
                .pac_url()
                .is_some_and(|url| url.ends_with("/proxy.pac"))
        );
        assert_eq!(ready.target_count(), 1);
        assert_eq!(ready.doh_host(), "1.1.1.1");
        assert_eq!(ready.transport(), "IPv4 TCP only for selected targets");
        runtime.cancel();
        runtime.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn second_bind_failure_drops_the_first_listener_without_ready_runtime() {
        let proxy_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_addr = proxy_reservation.local_addr().unwrap();
        drop(proxy_reservation);
        let pac_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let pac_addr = pac_reservation.local_addr().unwrap();
        let config = load_config(&config_text(
            &proxy_addr.to_string(),
            Some(&pac_addr.to_string()),
            1,
        ));

        let error = match Runtime::start(config).await {
            Ok(_) => panic!("occupied PAC listener must fail startup"),
            Err(error) => error,
        };
        assert!(matches!(error, AppError::PacBind { address, .. } if address == pac_addr));
        let rebound = TcpListener::bind(proxy_addr).await.unwrap();
        drop(rebound);
    }

    #[tokio::test]
    async fn direct_cancellation_seam_stops_services_and_closes_listeners() {
        let config = load_config(&config_text("127.0.0.1:0", None, 1));
        let mut runtime = Runtime::start(config).await.unwrap();
        let address = runtime.ready().proxy_addr();
        TcpStream::connect(address).await.unwrap();
        runtime.cancel();
        timeout(Duration::from_secs(1), runtime.run_until_signal())
            .await
            .unwrap()
            .unwrap();
        runtime.shutdown().await.unwrap();
        assert!(TcpStream::connect(address).await.is_err());
    }

    /// A `STOP` finishes every service with `Ok`; the run must still count as a clean stop,
    /// not "service stopped unexpectedly". Repeated because the race only sometimes loses.
    #[tokio::test]
    async fn a_control_stop_ends_the_run_cleanly() {
        for _ in 0..20 {
            let config = load_config(&config_text("127.0.0.1:0", None, 1));
            let mut runtime = Runtime::start(config).await.unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let token = ControlToken::generate().unwrap();
            runtime.spawn_control(listener, token.clone());
            request(address, &token, Verb::Stop).await.unwrap();
            // Let the services finish first, so the cancellation and a finished task are
            // both ready when `run_until_signal` looks: the moment the race happens.
            tokio::time::sleep(Duration::from_millis(50)).await;
            timeout(Duration::from_secs(2), runtime.run_until_signal())
                .await
                .expect("STOP ends the run")
                .expect("a requested stop is not an error");
            runtime.shutdown().await.unwrap();
        }
    }

    #[tokio::test]
    async fn drain_aborts_and_joins_stuck_tasks_at_deadline() {
        let mut tasks = JoinSet::new();
        tasks.spawn(async {
            std::future::pending::<()>().await;
            Ok(())
        });
        let started = Instant::now();
        drain_tasks(&mut tasks, Duration::from_millis(20))
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn task_panic_maps_to_internal_error() {
        let mut tasks = JoinSet::new();
        tasks.spawn(async {
            panic!("test panic");
            #[allow(unreachable_code)]
            Ok(())
        });
        let error = drain_tasks(&mut tasks, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Task(_)));
        assert_eq!(error.outcome(), Outcome::InternalError);
    }
}
