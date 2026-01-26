use std::num::NonZeroU8;

use redis::aio::MultiplexedConnection;

use crate::{prelude::*, util::redis::transaction_async};

#[derive(Debug, Clone, Copy)]
pub struct RateLimitParams {
    pub bucket_mins: NonZeroU8,
    pub window_buckets: NonZeroU8,
    pub window_limit: u16,
}

impl FromStr for RateLimitParams {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (window_limit, window) = s.split_once("/").context("Missing / delimiter")?;
        let window_limit = window_limit
            .trim()
            .parse()
            .with_context(|| format!("Invalid window limit {window_limit}"))?;

        let (window_buckets, bucket_mins) = window
            .split_once("x")
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
    pub async fn check<K: fmt::Display + ?Sized>(
        self,
        key: &K,
        mut conn: MultiplexedConnection,
    ) -> Result<bool> {
        let bucket_secs = 60 * i64::from(self.bucket_mins.get());
        let now = jiff::Timestamp::now();
        let curr_bucket = now.as_second() / bucket_secs;

        let keys: Vec<_> = (curr_bucket
            .checked_sub_unsigned(u64::from(self.window_buckets.get() - 1))
            .unwrap()..=curr_bucket)
            .map(|i| format!("rate_limit:{key}:{}min:{i:x}", self.bucket_mins))
            .collect();
        let curr_key = keys.len().checked_sub(1).unwrap_or_else(|| unreachable!());

        let (last, prev): (u32, Vec<u32>) =
            transaction_async(&mut conn, &keys, async |conn, pipe| {
                pipe.incr(&keys[curr_key], 1)
                    .expire_at(
                        &keys[curr_key],
                        curr_bucket
                            .saturating_add_unsigned(u64::from(self.window_buckets.get()))
                            .saturating_mul(bucket_secs),
                    )
                    .mget(&keys[..curr_key])
                    .query_async(conn)
                    .await
            })
            .await
            .context("Error running ratelimit transaction")?;

        Ok(last + prev.into_iter().sum::<u32>() <= u32::from(self.window_limit))
    }
}
