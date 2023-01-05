use futures_util::stream::FuturesUnordered;
use tracing_subscriber::EnvFilter;

use crate::prelude::*;

#[derive(Debug, clap::Parser)]
#[command(version, author, about)]
struct Opts {
    /// Log filter, using env_logger-like syntax
    #[arg(long, env = "RUST_LOG")]
    log_filter: Option<String>,

    /// Hint for the number of threads to use
    #[arg(short = 'j', long, env)]
    threads: Option<usize>,

    #[command(flatten)]
    client: crate::client::ClientOpts,
}

macro_rules! init_error {
    ($($args:tt)*) => ({
        ::tracing::error!($($args)*);
        ::std::process::exit(1);
    })
}

fn fmt_layer<S>() -> tracing_subscriber::fmt::Layer<S> {
    // configure log format here
    tracing_subscriber::fmt::layer()
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub fn main() {
    let tmp_logger =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(fmt_layer()));
    let span = error_span!("boot").entered();

    [
        ".env.local",
        if cfg!(debug_assertions) {
            ".env.dev"
        } else {
            ".env.prod"
        },
        ".env",
    ]
    .into_iter()
    .try_for_each(|p| match dotenv::from_filename(p) {
        Ok(p) => {
            trace!("Loaded env from {p:?}");
            Ok(())
        },
        Err(dotenv::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("Failed to load {p:?}")),
    })
    .unwrap_or_else(|e| init_error!("Loading .env files failed: {e:?}"));

    let opts: Opts = clap::Parser::parse();
    mem::drop(span);
    let span = error_span!("boot", ?opts).entered();

    let log_filter = opts.log_filter.as_deref().unwrap_or("info");

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_new(log_filter)
                .unwrap_or_else(|e| init_error!("Invalid log filter {log_filter:?}: {e}")),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .unwrap_or_else(|e| init_error!("Failed to initialize logger: {e}"));

    mem::drop((span, tmp_logger));

    let rt = {
        let mut builder = tokio::runtime::Builder::new_multi_thread();

        if let Some(threads) = opts.threads {
            builder
                .worker_threads(threads)
                .max_blocking_threads(threads * 2);
        }

        builder
            .enable_all()
            .build()
            .unwrap_or_else(|e| init_error!("Failed to initialize async runtime: {e}"))
    };

    std::process::exit(match rt.block_on(run(opts)) {
        Ok(()) => 0,
        Err(e) => {
            error!("{e:?}");
            1
        },
    });
}

enum StopType<S> {
    Signal(S),
    Closed(Result<(), serenity::Error>),
}

#[allow(clippy::inline_always)]
#[inline(always)]
#[instrument(level = "error", skip(opts))]
async fn run(opts: Opts) -> Result {
    let Opts {
        log_filter: _,
        threads: _,
        client,
    } = opts;

    let mut client = crate::client::build(client).await?;
    let signal;

    #[cfg(unix)]
    {
        use tokio::signal::unix::SignalKind;

        let mut stream = [
            SignalKind::hangup(),
            SignalKind::interrupt(),
            SignalKind::quit(),
            SignalKind::terminate(),
        ]
        .into_iter()
        .map(|k| {
            tokio::signal::unix::signal(k)
                .with_context(|| format!("Failed to hook signal {k:?}"))
                .map(|mut s| async move {
                    s.recv().await;
                    Result::<_>::Ok(k)
                })
        })
        .collect::<Result<FuturesUnordered<_>>>()?;

        signal = async move { stream.next().await.transpose() }
    }

    #[cfg(not(unix))]
    {
        use std::fmt;

        use futures_util::TryFutureExt;

        struct CtrlC;

        impl fmt::Debug for CtrlC {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("^C") }
        }

        signal = tokio::signal::ctrl_c().map_ok(|()| Some(CtrlC));
    }

    let ret = tokio::select! {
        s = signal => StopType::Signal(s),
        r = client.start() => StopType::Closed(r),
    };

    let shutdown = !matches!(ret, StopType::Closed(Err(_)));

    let ret = match ret {
        StopType::Signal(Ok(Some(s))) => {
            warn!("{s:?} received, shutting down...");
            Ok(())
        },
        StopType::Signal(Ok(None)) => Err(anyhow!("Unexpected error from signal handler")),
        StopType::Signal(Err(e)) => Err(e),
        StopType::Closed(Ok(())) => Err(anyhow!("Client hung up unexpectedly")),
        StopType::Closed(Err(e)) => Err(e).context("Fatal client error occurred"),
    };

    if shutdown {
        client.shard_manager.lock().await.shutdown_all().await;
    }

    ret
}
