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
#![allow(dead_code)] // TODO

pub(crate) mod client;
mod entry;
pub(crate) mod proto;
pub(crate) mod util;

pub(crate) mod prelude {
    pub use std::{
        borrow::{
            Borrow, BorrowMut, Cow,
            Cow::{Borrowed, Owned},
        },
        collections::{BTreeMap, BTreeSet, HashMap, HashSet},
        convert::Infallible,
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
