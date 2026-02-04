//! Entry point for the-q

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    clippy::clone_on_ref_ptr,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod client;
mod proto;
mod rpc;
mod util;

mod prelude {
    pub use marten::prelude::*;
}

fn main() { marten::boot::main::<Opts>(); }

#[derive(Debug, clap::Parser)]
#[command(version, author, about)]
struct Opts {
    /// Log filter, using env_logger-like syntax
    #[arg(long, env = "RUST_LOG")]
    log_filter: tracing_subscriber::EnvFilter,

    #[command(flatten)]
    command: Command,
}

#[derive(Debug, clap::Args)]
struct Command {
    /// Grafana Loki endpoint to use
    #[arg(long, env)]
    loki_endpoint: Option<url::Url>,

    /// Hint for the number of threads to use
    #[arg(short = 'j', long, env)]
    threads: Option<usize>,

    #[command(flatten)]
    client: crate::client::ClientOpts,
}

impl marten::CliOpts for Opts {
    type Command = Command;

    const CRATE: &str = env!("CARGO_PKG_NAME");

    fn into_parts(self) -> (tracing_subscriber::EnvFilter, Self::Command) {
        let Self {
            log_filter,
            command,
        } = self;

        (log_filter, command)
    }
}

impl marten::CliCommand for Command {
    fn loki_endpoint(&self) -> Option<&url::Url> { self.loki_endpoint.as_ref() }

    fn build_runtime(&self) -> marten::Result<tokio::runtime::Runtime, tokio::io::Error> {
        let mut builder = tokio::runtime::Builder::new_multi_thread();

        if let Some(threads) = self.threads {
            builder
                .worker_threads(threads)
                .max_blocking_threads(threads * 2);
        }

        builder.enable_all().build()
    }

    async fn run(self) -> marten::Result {
        let Self {
            loki_endpoint: _,
            threads: _,
            client,
        } = self;

        marten::run_until_signal_async(crate::client::build(client).await?).await
    }
}
