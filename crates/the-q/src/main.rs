//! Entry point for the-q

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(dead_code)] // TODO

pub(crate) mod client;
mod entry;
pub(crate) mod util;

pub(crate) mod prelude {
    pub use std::{
        borrow::{
            Borrow, Cow,
            Cow::{Borrowed, Owned},
        },
        collections::{BTreeMap, BTreeSet, HashMap, HashSet},
        fmt,
        future::Future,
        hash::Hash,
        marker::PhantomData,
        mem,
        str::FromStr,
        sync::Arc,
    };

    pub use anyhow::{anyhow, bail, ensure, Context as _, Error};
    pub use async_trait::async_trait;
    pub use futures_util::{FutureExt, StreamExt};
    pub use tracing::{
        debug, debug_span, error, error_span, info, info_span, instrument, trace, trace_span, warn,
        warn_span, Instrument,
    };
    pub use tracing_subscriber::prelude::*;
    pub use url::Url;

    pub type Result<T = (), E = Error> = std::result::Result<T, E>;
}

fn main() { entry::main(); }
