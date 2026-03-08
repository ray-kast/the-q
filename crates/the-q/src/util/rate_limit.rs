use std::num::NonZeroU8;

use redis::{aio::MultiplexedConnection, AsyncCommands};

use crate::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct RateLimitParams {
    pub bucket_mins: NonZeroU8,
    pub window_buckets: NonZeroU8,
    pub window_limit: u16,
}

impl FromStr for RateLimitParams {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (window_limit, window) = s.split_once('/').context("Missing / delimiter")?;
        let window_limit = window_limit
            .trim()
            .parse()
            .with_context(|| format!("Invalid window limit {window_limit}"))?;

        let (window_buckets, bucket_mins) = window
            .split_once('x')
            .context("Window format must be of the form '{N}x{M}min'")?;

        let window_buckets = window_buckets
            .trim()
            .parse()
            .with_context(|| format!("Invalid window bucket count {window_buckets}"))?;

        let bucket_mins = bucket_mins
            .trim_end()
            .strip_suffix("min")
            .context("Bucket duration must be of the form '{M}min'")?
            .trim()
            .parse()
            .with_context(|| format!("Invalid bucket duration {bucket_mins}"))?;

        Ok(Self {
            bucket_mins,
            window_buckets,
            window_limit,
        })
    }
}

impl RateLimitParams {
    #[instrument(level = "debug", fields(%key), skip(conn))]
    pub async fn acquire<K: fmt::Display + ?Sized>(
        self,
        key: &K,
        mut conn: MultiplexedConnection,
    ) -> Result<Option<RateLimitLock>, redis::RedisError> {
        let bucket_secs = 60 * i64::from(self.bucket_mins.get());
        let now = jiff::Timestamp::now();
        let curr_bucket = now.as_second() / bucket_secs;

        let keys: Vec<_> = (curr_bucket
            .checked_sub_unsigned(u64::from(self.window_buckets.get() - 1))
            .unwrap()..=curr_bucket)
            .map(|i| format!("rate_limit:{key}:{}min:{i:x}", self.bucket_mins))
            .collect();

        let curr_key = keys.len().checked_sub(1).unwrap_or_else(|| unreachable!());

        // HACK: manually inlining transaction_async because of AsyncFnMut fuckery
        let (last, mut all): (u32, Vec<Option<u32>>) = loop {
            redis::cmd("WATCH").arg(&keys).exec_async(&mut conn).await?;
            let mut pipe = redis::pipe();

            let response: Option<_> = pipe
                .atomic()
                .incr(&keys[curr_key], 1)
                .expire_at(
                    &keys[curr_key],
                    curr_bucket
                        .saturating_add_unsigned(u64::from(self.window_buckets.get()))
                        .saturating_mul(bucket_secs),
                )
                .ignore()
                .mget(keys.as_slice())
                .query_async(&mut conn)
                .await?;

            if let Some(response) = response {
                // make sure no watch is left in the connection, even if
                // someone forgot to use the pipeline.
                redis::cmd("UNWATCH").exec_async(&mut conn).await?;
                break response;
            }
        };

        debug!(%last, ?all, "Summing rate limit check");

        let mut keys = keys;
        let key = keys.pop();
        Ok(
            if last + all.drain(..curr_key).flatten().sum::<u32>() <= u32::from(self.window_limit) {
                Some(RateLimitLock { conn, key })
            } else {
                if let Some(key) = key {
                    () = conn.decr(key, 1).await?;
                }
                None
            },
        )
    }
}

#[must_use = ".confirm() must be called to prevent decrementing the request count"]
pub struct RateLimitLock {
    conn: MultiplexedConnection,
    key: Option<String>,
}

impl Drop for RateLimitLock {
    fn drop(&mut self) {
        if let Some(key) = self.key.take() {
            std::thread::scope(|scope| {
                let res = std::thread::Builder::new()
                    .name("gay baby async drop thread".into())
                    .spawn_scoped(scope, || {
                        tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(self.conn.decr(key, 1))
                            .unwrap_or_else(|err| warn!(%err, "Error releasing RateLimitLock"));
                    });

                match res {
                    // Don't log error, custom panic handler does that
                    Ok(h) => h.join().unwrap_or(()),
                    Err(err) => warn!(%err, "Error spawning drop thread for RateLimitLock"),
                }
            });
        }
    }
}

impl RateLimitLock {
    #[inline]
    pub fn confirm(&mut self) { self.key.take(); }
}
