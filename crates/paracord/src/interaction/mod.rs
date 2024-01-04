//! Types and support traits for responding to application interaction events

pub mod command;
pub mod completion;
pub mod handler;
mod registry;
pub mod response;
pub mod rpc;
pub mod visitor;

pub use registry::Registry;

// TODO: serenity dropped a lot of &mut's, we probably can too
