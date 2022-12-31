#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, clippy::cargo, missing_docs)]

pub(crate) mod client;
mod entry;

pub mod prelude {
    pub use std::sync::Arc;

    pub use anyhow::{anyhow, bail, ensure, Context, Error};
    pub use async_trait::async_trait;
    pub use futures_util::{FutureExt, StreamExt};
    pub use tracing::{
        debug, debug_span, error, error_span, info, info_span, trace, trace_span, warn, warn_span,
    };
    pub use tracing_subscriber::prelude::*;

    pub type Result<T = (), E = Error> = std::result::Result<T, E>;
}

fn main() { entry::main(); }
