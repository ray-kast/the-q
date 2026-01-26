use redis::{aio::ConnectionLike, cmd, pipe, Pipeline, RedisResult, ToRedisArgs};

/// Hacked together from [`redis::transaction`] to make it async
pub async fn transaction_async<
    C: ConnectionLike,
    K: ToRedisArgs,
    T,
    F: AsyncFnMut(&mut C, &mut Pipeline) -> RedisResult<Option<T>>,
>(
    con: &mut C,
    keys: &[K],
    func: F,
) -> RedisResult<T> {
    let mut func = func;
    loop {
        cmd("WATCH").arg(keys).exec_async(con).await?;
        let mut p = pipe();
        let response: Option<T> = func(con, p.atomic()).await?;
        match response {
            None => {
                continue;
            },
            Some(response) => {
                // make sure no watch is left in the connection, even if
                // someone forgot to use the pipeline.
                cmd("UNWATCH").exec_async(con).await?;
                return Ok(response);
            },
        }
    }
}
