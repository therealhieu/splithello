//! Log setup: a readable `human` format for terminals and log files, `logfmt` for pipes
//! and tests. Both render the same structured events.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::File;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::ValueEnum;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

use crate::diagnostics::Outcome;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum LogFormat {
    /// `human` when stdout is a terminal, otherwise `logfmt`.
    #[default]
    Auto,
    /// Aligned, colored lines with icons and units.
    Human,
    /// `key=value` lines with RFC 3339 timestamps.
    Logfmt,
}

pub(crate) struct LogOptions {
    pub(crate) format: LogFormat,
    pub(crate) verbose: u8,
    pub(crate) quiet: bool,
}

impl LogOptions {
    /// The command-line flags that produce these options, so a background instance (and
    /// `restart`) logs the way its `start` asked.
    pub(crate) fn flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if self.quiet {
            flags.push("-q".to_owned());
        }
        if self.verbose > 0 {
            flags.push(format!("-{}", "v".repeat(usize::from(self.verbose))));
        }
        if let Some(format) = self
            .format
            .to_possible_value()
            .filter(|_| self.format != LogFormat::Auto)
        {
            flags.push("--log-format".to_owned());
            flags.push(format.get_name().to_owned());
        }
        flags
    }

    /// `RUST_LOG` wins. Otherwise SplitHello's own events follow `-q` (warn), default (info),
    /// `-v` (debug), `-vv` (trace); dependencies stay at warn until `-vv` raises them to debug.
    fn filter(&self) -> EnvFilter {
        let directives = match (self.quiet, self.verbose) {
            (true, _) => "warn",
            (false, 0) => "warn,splithello=info",
            (false, 1) => "warn,splithello=debug",
            (false, _) => "debug,splithello=trace",
        };
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(directives))
    }
}

/// Where log lines go: the terminal (or pipe) by default, a file for a background instance.
pub(crate) enum LogTarget {
    Stdout,
    File(File),
}

pub(crate) fn init(options: &LogOptions, target: LogTarget) {
    let terminal = matches!(target, LogTarget::Stdout) && std::io::stdout().is_terminal();
    let file = matches!(target, LogTarget::File(_));
    // A log file is read by people (`splithello logs`), so `auto` means human there too.
    let human = match options.format {
        LogFormat::Auto => terminal || file,
        LogFormat::Human => true,
        LogFormat::Logfmt => false,
    };
    let writer = match target {
        LogTarget::Stdout => BoxMakeWriter::new(std::io::stdout),
        LogTarget::File(file) => BoxMakeWriter::new(Arc::new(file)),
    };
    let builder = tracing_subscriber::fmt()
        .with_env_filter(options.filter())
        .with_writer(writer);
    if human {
        let color = terminal && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty());
        builder
            .event_format(HumanFormat {
                color,
                // A background instance runs for days; a bare time of day would be ambiguous.
                clock: if file { dated_clock } else { local_clock },
            })
            .init();
    } else {
        builder.with_target(false).with_timer(LocalTime).init();
    }
}

/// Opens `path` for appending, after moving the previous run's log to `<path>.1`, so the
/// file holds one run and the one before it. Rotation is best effort: when the rename
/// fails (on Windows another program may hold `<path>.1` open), logging appends instead
/// and the error is returned for a warning once logging is up.
pub(crate) fn open_log(path: &Path) -> std::io::Result<(File, Option<std::io::Error>)> {
    let rotation = match std::fs::rename(path, rotated(path)) {
        Ok(()) => None,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(error),
    };
    let file = crate::instance::private_options()
        .append(true)
        .create(true)
        .open(path)?;
    Ok((file, rotation))
}

/// `<path>.1`, whatever the extension.
pub(crate) fn rotated(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

/// A background instance has no terminal for a panic message; this puts it in the log.
pub(crate) fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(outcome = %Outcome::InternalError, error = %info, "SplitHello panicked");
        previous(info);
    }));
}

/// RFC 3339 local time with milliseconds and offset, e.g. `2026-10-15T12:01:03.042+07:00`.
struct LocalTime;

impl FormatTime for LocalTime {
    fn format_time(&self, writer: &mut Writer<'_>) -> fmt::Result {
        write!(
            writer,
            "{}",
            jiff::Zoned::now().strftime("%Y-%m-%dT%H:%M:%S%.3f%:z")
        )
    }
}

fn local_clock() -> String {
    jiff::Zoned::now().strftime("%H:%M:%S").to_string()
}

fn dated_clock() -> String {
    jiff::Zoned::now().strftime("%Y-%m-%d %H:%M:%S").to_string()
}

/// Renders the events SplitHello emits as short aligned lines:
///
/// ```text
/// SplitHello 0.1.0 ready
///   proxy  127.0.0.1:8080
/// 12:01:03 ✓ split    www.reddit.com      48.2s  ↑5.2K ↓881K
/// 12:01:04 ✗ dns      old.reddit.com      DoH request timed out → check [dns].upstream …
/// ```
pub(crate) struct HumanFormat {
    pub(crate) color: bool,
    pub(crate) clock: fn() -> String,
}

#[derive(Default)]
struct Fields {
    values: BTreeMap<&'static str, String>,
    message: String,
}

impl Fields {
    fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    fn number(&self, name: &str) -> Option<u64> {
        self.get(name).and_then(|value| value.parse().ok())
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_owned();
        } else {
            self.values.insert(field.name(), value.to_owned());
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let value = format!("{value:?}");
        if field.name() == "message" {
            self.message = value;
        } else {
            self.values.insert(field.name(), value);
        }
    }
}

#[derive(Clone, Copy)]
enum Paint {
    Green,
    Red,
    Yellow,
    Dim,
    Bold,
}

impl HumanFormat {
    fn paint(&self, paint: Paint, text: &str) -> String {
        if !self.color {
            return text.to_owned();
        }
        let code = match paint {
            Paint::Green => "32",
            Paint::Red => "31",
            Paint::Yellow => "33",
            Paint::Dim => "2",
            Paint::Bold => "1",
        };
        format!("\x1b[{code}m{text}\x1b[0m")
    }

    fn render(&self, level: Level, fields: &Fields) -> String {
        let outcome = fields.get("outcome").and_then(Outcome::parse);
        if outcome == Some(Outcome::Ready) {
            return self.ready(fields);
        }
        let time = self.paint(Paint::Dim, &(self.clock)());
        if fields.get("tunnels").is_some() {
            return format!("{time} {}", self.summary(fields));
        }
        if let Some(latest) = fields.get("latest_version") {
            return format!("{time} {}", self.update_available(latest, fields));
        }
        if let Some(outcome) = outcome {
            return format!("{time} {}", self.outcome_line(outcome, fields));
        }
        let mut line = format!("{time} {}", self.level(level));
        if !fields.message.is_empty() {
            let _ = write!(line, " {}", fields.message);
        }
        for (name, value) in &fields.values {
            let _ = write!(
                line,
                " {}",
                self.paint(Paint::Dim, &format!("{name}={value}"))
            );
        }
        line
    }

    fn level(&self, level: Level) -> String {
        match level {
            Level::ERROR => self.paint(Paint::Red, "error"),
            Level::WARN => self.paint(Paint::Yellow, "warn "),
            Level::INFO => self.paint(Paint::Dim, "info "),
            Level::DEBUG => self.paint(Paint::Dim, "debug"),
            Level::TRACE => self.paint(Paint::Dim, "trace"),
        }
    }

    fn ready(&self, fields: &Fields) -> String {
        let mut out = format!(
            "{} {} {}",
            self.paint(Paint::Bold, "SplitHello"),
            env!("CARGO_PKG_VERSION"),
            self.paint(Paint::Green, "ready")
        );
        let row = |label: &str, value: &str, note: &str| {
            format!(
                "\n  {}  {value}{}",
                self.paint(Paint::Dim, &format!("{label:<5}")),
                if note.is_empty() {
                    String::new()
                } else {
                    self.paint(Paint::Dim, &format!("   {note}"))
                }
            )
        };
        if let Some(proxy) = fields.get("proxy_addr") {
            out.push_str(&row("proxy", proxy, "browser HTTPS proxy"));
        }
        if let Some(pac) = fields.get("pac_url").filter(|pac| *pac != "disabled") {
            out.push_str(&row("pac", pac, "or this PAC URL"));
        }
        if let Some(doh) = fields.get("doh_host") {
            let targets = fields.get("target_count").unwrap_or("?");
            out.push_str(&row(
                "dns",
                &format!("{doh} (DoH)"),
                &format!("{targets} target(s)"),
            ));
        }
        out
    }

    fn outcome_line(&self, outcome: Outcome, fields: &Fields) -> String {
        let (icon, paint) = match outcome {
            Outcome::TargetTransformed => ("✓", Paint::Green),
            Outcome::DirectRelay | Outcome::Idle | Outcome::Reset | Outcome::Cancelled => {
                ("·", Paint::Dim)
            }
            Outcome::Timeout => ("✗", Paint::Yellow),
            _ => ("✗", Paint::Red),
        };
        let mut line = format!(
            "{} {}",
            self.paint(paint, icon),
            self.paint(paint, &format!("{:<8}", outcome.label()))
        );
        if let Some(host) = fields.get("host") {
            let _ = write!(line, " {host:<28}");
        }
        if let Some(duration) = fields.number("duration_ms") {
            let _ = write!(line, " {:>7}", human_duration_ms(duration));
        }
        if let (Some(up), Some(down)) = (fields.number("bytes_up"), fields.number("bytes_down")) {
            let _ = write!(
                line,
                "  {}",
                self.paint(
                    Paint::Dim,
                    &format!("↑{} ↓{}", human_bytes(up), human_bytes(down))
                )
            );
        }
        if let Some(error) = fields.get("error") {
            let _ = write!(line, "  {error}");
        }
        if let Some(hint) = fields.get("hint") {
            let _ = write!(line, "{}", self.paint(Paint::Dim, &format!(" → {hint}")));
        }
        line
    }

    fn update_available(&self, latest: &str, fields: &Fields) -> String {
        let mut line = format!(
            "{} {} {latest} available",
            self.paint(Paint::Yellow, "↑"),
            self.paint(Paint::Yellow, &format!("{:<8}", "update"))
        );
        if let Some(current) = fields.get("current_version") {
            let _ = write!(line, " (you have {current})");
        }
        if let Some(hint) = fields.get("hint") {
            let _ = write!(line, "{}", self.paint(Paint::Dim, &format!(" → {hint}")));
        }
        line
    }

    fn summary(&self, fields: &Fields) -> String {
        let count = |name| fields.number(name).unwrap_or(0);
        let failed = count("failed");
        let failed = if failed > 0 {
            self.paint(Paint::Red, &format!("{failed} failed"))
        } else {
            "0 failed".to_owned()
        };
        self.paint(
            Paint::Dim,
            &format!(
                "· last {}: {} tunnels ({} split, {} direct, {failed}) · {} active",
                fields.get("window").unwrap_or("?"),
                count("tunnels"),
                count("split"),
                count("direct"),
                count("active"),
            ),
        )
    }
}

impl<S, N> FormatEvent<S, N> for HumanFormat
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _context: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut fields = Fields::default();
        event.record(&mut fields);
        writeln!(
            writer,
            "{}",
            self.render(*event.metadata().level(), &fields)
        )
    }
}

fn human_duration_ms(ms: u64) -> String {
    match ms {
        0..1_000 => format!("{ms}ms"),
        1_000..60_000 => format!("{:.1}s", ms as f64 / 1_000.0),
        _ => format!("{}m{:02}s", ms / 60_000, (ms % 60_000) / 1_000),
    }
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["K", "M", "G", "T"];
    if bytes < 1_024 {
        return format!("{bytes}B");
    }
    let mut value = bytes as f64 / 1_024.0;
    let mut unit = 0;
    while value >= 1_024.0 && unit < UNITS.len() - 1 {
        value /= 1_024.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{value:.0}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::{Arc, Mutex};

    use tracing_subscriber::fmt::MakeWriter;

    use super::{HumanFormat, human_bytes, human_duration_ms};
    use crate::diagnostics::Outcome;

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for Capture {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn capture(emit: impl FnOnce()) -> String {
        let output = Capture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(output.clone())
            .event_format(HumanFormat {
                color: false,
                clock: || "12:00:00".to_owned(),
            })
            .finish();
        tracing::subscriber::with_default(subscriber, emit);
        String::from_utf8(output.0.lock().unwrap().clone()).unwrap()
    }

    #[test]
    fn ready_event_renders_a_banner() {
        let output = capture(|| {
            tracing::info!(
                outcome = %Outcome::Ready,
                proxy_addr = "127.0.0.1:8080",
                pac_url = "http://127.0.0.1:8081/proxy.pac",
                target_count = 2,
                doh_host = "1.1.1.1",
                "SplitHello is ready"
            );
        });
        assert_eq!(
            output,
            format!(
                "SplitHello {} ready\n  proxy  127.0.0.1:8080   browser HTTPS proxy\n  pac    http://127.0.0.1:8081/proxy.pac   or this PAC URL\n  dns    1.1.1.1 (DoH)   2 target(s)\n",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn tunnel_and_failure_events_render_one_aligned_line() {
        let output = capture(|| {
            tracing::info!(
                host = "www.reddit.com",
                port = 443,
                target = true,
                outcome = %Outcome::TargetTransformed,
                duration_ms = 48_213_u64,
                bytes_up = 5_321_u64,
                bytes_down = 902_113_u64,
                "tunnel completed"
            );
            tracing::warn!(
                host = "old.reddit.com",
                outcome = %Outcome::DnsError,
                error = "DoH request timed out",
                hint = Outcome::DnsError.hint().unwrap(),
                "tunnel failed"
            );
        });
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(
            lines[0],
            "12:00:00 ✓ split    www.reddit.com                 48.2s  ↑5.2K ↓881K"
        );
        assert!(
            lines[1].starts_with(
                "12:00:00 ✗ dns      old.reddit.com                DoH request timed out → "
            ),
            "{}",
            lines[1]
        );
    }

    #[test]
    fn update_event_renders_versions_and_the_upgrade_command() {
        let output = capture(|| {
            tracing::info!(
                latest_version = "0.2.0",
                current_version = "0.1.0",
                hint = "brew upgrade splithello",
                "update available"
            );
        });
        assert_eq!(
            output,
            "12:00:00 ↑ update   0.2.0 available (you have 0.1.0) → brew upgrade splithello\n"
        );
    }

    #[test]
    fn summary_event_renders_counts() {
        let output = capture(|| {
            tracing::info!(
                window = "1m",
                tunnels = 37_u64,
                split = 12_u64,
                direct = 24_u64,
                failed = 1_u64,
                active = 6_u64,
                "summary"
            );
        });
        assert_eq!(
            output,
            "12:00:00 · last 1m: 37 tunnels (12 split, 24 direct, 1 failed) · 6 active\n"
        );
    }

    #[test]
    fn options_turn_back_into_the_flags_that_made_them() {
        use super::{LogFormat, LogOptions};
        let options = |format, verbose, quiet| LogOptions {
            format,
            verbose,
            quiet,
        };
        assert!(options(LogFormat::Auto, 0, false).flags().is_empty());
        assert_eq!(options(LogFormat::Auto, 2, false).flags(), ["-vv"]);
        assert_eq!(
            options(LogFormat::Logfmt, 0, true).flags(),
            ["-q", "--log-format", "logfmt"]
        );
    }

    #[test]
    fn a_log_file_keeps_one_previous_run() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("splithello.log");
        let write = |text: &str| {
            let (mut file, rotation) = super::open_log(&path).unwrap();
            assert!(rotation.is_none());
            io::Write::write_all(&mut file, text.as_bytes()).unwrap();
        };
        write("first\n");
        write("second\n");
        write("third\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "third\n");
        assert_eq!(
            std::fs::read_to_string(super::rotated(&path)).unwrap(),
            "second\n"
        );
        assert_eq!(
            super::rotated(std::path::Path::new("/var/log/splithello")),
            std::path::PathBuf::from("/var/log/splithello.1")
        );
    }

    #[test]
    fn units_are_compact() {
        assert_eq!(human_duration_ms(480), "480ms");
        assert_eq!(human_duration_ms(48_213), "48.2s");
        assert_eq!(human_duration_ms(302_000), "5m02s");
        assert_eq!(human_bytes(900), "900B");
        assert_eq!(human_bytes(5_321), "5.2K");
        assert_eq!(human_bytes(902_113), "881K");
        assert_eq!(human_bytes(3 * 1_024 * 1_024 * 1_024), "3.0G");
    }
}
