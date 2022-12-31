use futures_util::stream::FuturesUnordered;

use crate::prelude::*;

#[derive(Debug, clap::Parser)]
struct Opts {
    /// The Discord API token to use
    #[arg(long, env)]
    discord_token: String,

    /// Hint for the number of threads to use
    #[arg(short = 'j', long, env)]
    threads: Option<usize>,
}

#[allow(clippy::inline_always)]
#[inline(always)]
pub fn main() {
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
        Err(e) => Err(e),
    })
    .expect("Failed to load .env files");

    tracing_log::LogTracer::init().expect("Failed to initialize LogTracer");
    tracing::subscriber::set_global_default(
        tracing_subscriber::Registry::default()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .or_else(|_| tracing_subscriber::EnvFilter::try_new("info"))
                    .unwrap(),
            )
            .with(tracing_subscriber::fmt::layer()),
    )
    .expect("Failed to set default tracing subscriber");

    let opts: Opts = clap::Parser::parse();

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
            .expect("Failed to initialize async runtime")
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
async fn run(opts: Opts) -> Result {
    let Opts {
        threads: _,
        discord_token,
    } = opts;

    let mut client = crate::client::build(discord_token).await?;

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
        client.shard_manager.lock_owned().await.shutdown_all().await;
    }

    ret
}
