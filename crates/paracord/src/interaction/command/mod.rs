//! Types for constructing command descriptions to be registered or inspecting
//! already-registered command metadata

mod arg;
mod arg_builder;
mod info;
mod registered;
mod sim;
mod try_from_value;

pub use arg::*;
pub use arg_builder::*;
pub use info::*;
pub(super) use registered::*;
pub use sim::*;

/// Helper traits for working with command metadata
pub mod prelude {
    pub use super::{arg_builder::ArgBuilderExt as _, info::CommandInfoExt as _};
}

/// An error resulting from converting a value into a [`CommandInfo`] with
/// [`TryFrom`]
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Error converting command: {0}")]
pub struct TryFromError(pub &'static str);
