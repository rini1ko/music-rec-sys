use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Colored text on the console and JSON lines in `<log_dir>/server.log.<date>`.
/// The level comes from `RUST_LOG` (default `info`). The returned guard must
/// stay alive until the program ends, otherwise the last lines are lost.
pub fn init(log_dir: &str) -> WorkerGuard {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (file, guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::daily(log_dir, "server.log"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(fmt::layer().json().with_ansi(false).with_writer(file))
        .init();
    guard
}
