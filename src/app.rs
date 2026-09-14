use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::net::TcpListener;
use tokio::task::{JoinError, JoinSet};
use tokio::time::{Instant, timeout_at};
use tokio_util::sync::CancellationToken;

use crate::config::{Config, ConfigError};
use crate::diagnostics::Outcome;
use crate::dns::{DohResolver, ResolveError};
use crate::domain::DomainError;
use crate::pac::{PacServer, PacServiceError, render_pac};
use crate::proxy::{ProxyLimits, ProxyServer, ProxyServiceError, ProxyState};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadyMetadata {
    proxy_addr: SocketAddr,
    pac_url: Option<String>,
    target_count: usize,
    doh_host: String,
}

impl ReadyMetadata {
    #[cfg(test)]
    pub(crate) fn proxy_addr(&self) -> SocketAddr {
        self.proxy_addr
    }

    #[cfg(test)]
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
            setup = "configure the browser HTTPS proxy or PAC URL",
            scope = "explicit loopback proxy; not a VPN",
            "SplitHello is ready"
        );
    }
}

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("could not construct authenticated target resolver")]
    Resolver(#[source] ResolveError),
    #[error("could not compile target policy")]
    Domain(#[source] DomainError),
    #[error("could not bind proxy listener {address}")]
    ProxyBind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("could not bind PAC listener {address}")]
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
    #[error("could not install or receive shutdown signal")]
    Signal(#[source] std::io::Error),
}

impl AppError {
    pub(crate) const fn outcome(&self) -> Outcome {
        match self {
            Self::Config(_) => Outcome::ConfigError,
            Self::Resolver(_) => Outcome::DnsError,
            Self::Domain(_) => Outcome::ConfigError,
            Self::ProxyBind { .. }
            | Self::PacBind { .. }
            | Self::ListenerAddress(_)
            | Self::ProxyService(_)
            | Self::PacService(_) => Outcome::ConnectError,
            Self::ServiceStopped | Self::Task(_) => Outcome::InternalError,
            Self::Signal(_) => Outcome::InternalError,
        }
    }
}

pub(crate) struct Runtime {
    cancel: CancellationToken,
    tasks: JoinSet<Result<(), AppError>>,
    shutdown_timeout: Duration,
    ready: ReadyMetadata,
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
            target_resolver,
            config.proxy().allowed_ports().iter().copied(),
            limits,
            config.limits().max_connections(),
        ));
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

        Ok(Self {
            cancel,
            tasks,
            shutdown_timeout,
            ready,
        })
    }

    pub(crate) fn ready(&self) -> &ReadyMetadata {
        &self.ready
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
    }

    pub(crate) async fn run_until_signal(&mut self) -> Result<(), AppError> {
        tokio::select! {
            _ = self.cancel.cancelled() => Ok(()),
            signal = shutdown_signal() => {
                signal.map_err(AppError::Signal)?;
                self.cancel.cancel();
                Ok(())
            }
            joined = self.tasks.join_next(), if !self.tasks.is_empty() => {
                match joined {
                    Some(Ok(Ok(()))) => Err(AppError::ServiceStopped),
                    Some(Ok(Err(error))) => Err(error),
                    Some(Err(error)) => Err(AppError::Task(error)),
                    None => Err(AppError::ServiceStopped),
                }
            }
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<(), AppError> {
        self.cancel.cancel();
        drain_tasks(&mut self.tasks, self.shutdown_timeout).await
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
    use crate::diagnostics::Outcome;

    use super::{AppError, Runtime, drain_tasks};

    fn config_text(proxy: &str, pac: Option<&str>, shutdown_seconds: u64) -> String {
        let pac = pac
            .map(|address| format!("pac_listen = \"{address}\"\n"))
            .unwrap_or_default();
        format!(
            "[proxy]\nlisten = \"{proxy}\"\n{pac}\n[dns]\nupstream = \"https://1.1.1.1/dns-query\"\n\n[limits]\nshutdown_timeout_seconds = {shutdown_seconds}\n\n[[targets]]\nhost = \"reddit.com\"\ninclude_subdomains = true\nstrategy = \"tls-record-split\"\n"
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
