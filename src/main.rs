use std::io;

use anyhow::Result;
#[cfg(not(debug_assertions))]
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// A `MakeWriter` impl that always returns `io::stdout()`.
///
/// Necessary because `fmt::layer().with_writer(io::stdout)` panics at
/// runtime, and `tracing_appender::non_blocking` requires `io::Write`,
/// not `MakeWriter`.
struct StdoutMaker;

impl<'a> fmt::MakeWriter<'a> for StdoutMaker {
    type Writer = io::Stdout;

    fn make_writer(&self) -> Self::Writer {
        io::stdout()
    }
}

#[cfg(debug_assertions)]
fn init_tracing(env_filter: EnvFilter) {
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(StdoutMaker))
        .with(env_filter)
        .init();
}

#[cfg(not(debug_assertions))]
fn init_tracing(env_filter: EnvFilter) -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("logs", "poprako-b.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(StdoutMaker))
        .with(
            fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(non_blocking),
        )
        .with(env_filter)
        .init();

    guard
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    init_tracing(env_filter);

    poprako_b_preview::bot::run_server().await
}
