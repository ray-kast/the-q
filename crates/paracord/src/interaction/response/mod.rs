//! Types for responding to interactions according to the Discord webhook
//! protocol in a type-safe manner

mod component;
mod data;
mod embed;
pub mod id;
mod message;
mod modal;
mod responder;

pub use component::*;
pub use data::*;
pub use embed::*;
pub use message::*;
pub use modal::*;
pub use responder::*;

/// Helper traits for working with response data
pub mod prelude {
    pub use super::{
        component::{
            ComponentsExt as _, MessageActionRow as _, ModalActionRow as _, TextInputExt as _,
        },
        embed::EmbedExt as _,
        message::{MessageBodyExt as _, MessageExt as _, MessageOptsExt as _},
        responder::ResponderExt as _,
    };
}
