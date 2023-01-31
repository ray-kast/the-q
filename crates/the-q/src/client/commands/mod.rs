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
                IntoErr, ModalResponder, ModalResult, RpcError, RpcHandler, Visitor,
            },
            response::{
                prelude::*, ButtonStyle, Embed, Message, MessageBody, MessageComponent,
                MessageOpts, Modal, ModalSource, ResponseData, TextInput,
            },
            rpc, visitor,
        },
        CommandOpts, ComponentKey, ModalKey, Schema,
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
    pub type ComponentResponder<'a, 'b> = handler::ComponentResponder<'a, 'b, Schema>;
    pub type ComponentResult<'a> = handler::ComponentResult<'a, Schema>;

    #[inline]
    pub fn id<T>(t: T) -> T { t }
}

pub use rpc::*;

pub type Handlers = prelude::handler::Handlers<Schema>;

// TODO: set up command names
#[derive(Debug, clap::Args)]
pub struct CommandOpts {
    #[arg(long, env, default_value = "q")]
    command_base: String,

    #[arg(long, env, default_value = "")]
    context_menu_base: String,
}

// TODO: can this be attribute-macro-ified?
pub fn handlers(opts: &CommandOpts) -> Handlers {
    use prelude::Arc;

    let explode = Arc::new(explode::ExplodeCommand::from(opts));
    let point = Arc::new(point::PointCommand::from(opts));
    let say = Arc::new(say::SayCommand::from(opts));
    let test = Arc::new(test::TestCommand::from(opts));
    let vc = Arc::new(vc::VcCommand::from(opts));

    Handlers {
        commands: vec![
            explode,
            point,
            say,
            test,
            Arc::clone(&vc) as Arc<dyn prelude::Handler<Schema>>,
        ],
        components: vec![vc],
        modals: vec![],
    }
}
