// !TODO: rename to interaction or something

pub mod handler;
mod registry;
pub mod visitor;

pub mod response {
    mod data;
    mod message;
    mod modal;
    mod responder;

    pub use data::*;
    pub use message::*;
    pub use modal::*;
    pub use responder::*;
}

pub use registry::Registry;
