use std::env;
use std::io;

use anyhow::Result;
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

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let env_filter = EnvFilter::from_default_env();
    let is_dev = env::var("ENVIRONMENT")
        .map(|v| v.to_lowercase() == "development")
        .unwrap_or(false);

    if is_dev {
        tracing_subscriber::registry()
            .with(fmt::layer().with_writer(StdoutMaker))
            .with(env_filter)
            .init();
    } else {
        // Production: stdout + daily-rotating file under logs/.
        let file_appender = tracing_appender::rolling::daily("logs", "poprako-b.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        // Leak the guard so the non-blocking writer thread lives for the
        // entire process lifetime.
        std::mem::forget(guard);

        tracing_subscriber::registry()
            .with(fmt::layer().with_writer(StdoutMaker))
            .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
            .with(env_filter)
            .init();
    }

    poprako_b_preview::bot::run_server().await
}
