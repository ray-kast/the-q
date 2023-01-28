mod arg;
mod arg_builder;
mod info;
mod registered;
mod sim;
pub(self) mod try_from_value;

pub use arg::*;
pub use arg_builder::*;
pub use info::*;
pub(super) use registered::*;
pub use sim::*;

pub mod prelude {
    pub use super::{arg_builder::ArgBuilderExt as _, info::CommandInfoExt as _};
}

#[derive(Debug, thiserror::Error)]
#[error("Error converting command: {0}")]
pub struct TryFromError(pub &'static str);
