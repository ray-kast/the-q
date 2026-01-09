use paracord::interaction::{handler, response};

use crate::{proto::component, rpc::Schema};

pub type MessageBody<E = response::id::Error> = response::MessageBody<component::Component, E>;
pub type CommandError<'a> = handler::CommandError<'a, Schema>;
pub type CommandResult<'a> = handler::CommandResult<'a, Schema>;
pub type CommandResponder<'a, 'b> = handler::CommandResponder<'a, 'b, Schema>;
// pub type ComponentError<'a> = handler::ComponentError<'a, Schema>;
pub type ComponentResult<'a> = handler::ComponentResult<'a, Schema>;
pub type ComponentResponder<'a, 'b> = handler::ComponentResponder<'a, 'b, Schema>;
// pub type ModalError<'a> = handler::ModalError<'a, Schema>;
// pub type ModalResult<'a> = handler::ModalResult<'a, Schema>;
// pub type ModalResponder<'a, 'b> = handler::ModalResponder<'a, 'b, Schema>;
