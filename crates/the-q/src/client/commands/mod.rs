mod explode;
mod point;
mod rpc;
mod say;
mod test;
mod vc;

pub(self) mod prelude {
    pub(super) use serenity::client::Context;

    pub use super::{
        super::interaction::{
            command::{prelude::*, Args, CommandInfo},
            completion::Completion,
            handler,
            handler::{
                CommandError, CommandHandler as Handler, CompletionResult, CompletionVisitor,
                ComponentResponder, ComponentResult, IntoErr, ModalResponder, ModalResult,
                RpcError, RpcHandler, Visitor,
            },
            response::{
                prelude::*, ButtonStyle, Embed, Message, MessageBody, MessageComponent,
                MessageOpts, Modal, ModalSource, ResponseData, TextInput,
            },
            rpc, visitor,
        },
        Schema,
    };
    pub use crate::{
        prelude::*,
        proto::{
            component, component::component::Payload as ComponentPayload, modal,
            modal::modal::Payload as ModalPayload,
        },
    };

    pub type CommandResponder<'a, 'b> = handler::CommandResponder<'a, 'b, Schema>;
    pub type CommandResult<'a> = handler::CommandResult<'a, Schema>;

    #[inline]
    pub fn id<T>(t: T) -> T { t }
}

pub use rpc::*;

pub fn list() -> Vec<prelude::Arc<dyn prelude::Handler<Schema>>> {
    use prelude::Arc;

    vec![
        Arc::new(explode::ExplodeCommand::default()),
        Arc::new(point::PointCommand::default()),
        Arc::new(say::SayCommand::default()),
        Arc::new(test::TestCommand::default()),
        Arc::new(vc::VcCommand::default()),
    ]
}

pub fn components()
-> Vec<prelude::Arc<dyn prelude::RpcHandler<Schema, prelude::component::Component>>> {
    use prelude::Arc;

    vec![]
}

pub fn modals() -> Vec<prelude::Arc<dyn prelude::RpcHandler<Schema, prelude::modal::Modal>>> {
    use prelude::Arc;

    vec![]
}
