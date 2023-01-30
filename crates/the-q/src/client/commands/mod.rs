mod explode;
mod point;
mod say;
mod test;
mod vc;

pub(self) mod prelude {
    pub(super) use serenity::client::Context;

    pub use super::super::interaction::{
        command::{prelude::*, Args, CommandInfo},
        completion::Completion,
        handler,
        handler::{
            CommandError, CommandHandler as Handler, CommandResponder, CommandResult,
            CompletionResult, CompletionVisitor, IntoErr, Visitor,
        },
        response::{
            prelude::*, ButtonStyle, Embed, Message, MessageBody, MessageComponent, MessageOpts,
            Modal, ResponseData, TextInput,
        },
        visitor,
    };
    pub use crate::{
        prelude::*,
        proto::{
            component, component::component::Payload as ComponentPayload, modal,
            modal::modal::Payload as ModalPayload,
        },
    };

    #[inline]
    pub fn id<T>(t: T) -> T { t }
}

pub fn list() -> Vec<prelude::Arc<dyn prelude::Handler>> {
    use prelude::Arc;

    vec![
        Arc::new(explode::ExplodeCommand::default()),
        Arc::new(point::PointCommand::default()),
        Arc::new(say::SayCommand::default()),
        Arc::new(test::TestCommand::default()),
        Arc::new(vc::VcCommand::default()),
    ]
}
