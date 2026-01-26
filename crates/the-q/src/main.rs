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
mod entry;
mod proto;
mod rpc;
mod util;

mod prelude {
    #![expect(unused_imports, reason = "Some exports may not yet be used")]

    pub use std::{
        borrow::{
            Borrow, BorrowMut, Cow,
            Cow::{Borrowed, Owned},
        },
        collections::{BTreeMap, BTreeSet},
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
    pub use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt};
    pub use hashbrown::{HashMap, HashSet};
    pub use tracing::{
        debug, debug_span, error, error_span, info, info_span, instrument, trace, trace_span, warn,
        warn_span, Instrument,
    };
    pub use tracing_subscriber::prelude::*;
    pub use url::Url;

    pub type Result<T = (), E = Error> = std::result::Result<T, E>;
}

fn main() { entry::main(); }
