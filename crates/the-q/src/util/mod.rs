use crate::prelude::*;

pub mod image;
pub mod interaction;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct DebugShim<T>(pub T);

impl<T> fmt::Debug for DebugShim<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(std::any::type_name::<T>())
            .finish_non_exhaustive()
    }
}

impl<T> From<T> for DebugShim<T> {
    fn from(val: T) -> Self { Self(val) }
}

pub fn http_client(timeout: Option<std::time::Duration>) -> reqwest::Client {
    let timeout = timeout.unwrap_or(std::time::Duration::from_secs(10));
    let client = reqwest::Client::builder()
        .user_agent("the-q")
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .timeout(timeout)
        .connect_timeout(timeout);
    client.build().unwrap()
}
