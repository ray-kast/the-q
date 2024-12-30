mod explode;
mod jpeg;
mod point;
mod re;
mod rpc;
mod say;
mod sound;
mod test;

mod prelude {
    #![expect(unused_imports, reason = "Some exports may not yet be used")]

    pub use paracord::interaction::{
        command::{prelude::*, Args, CommandInfo},
        completion::Completion,
        handler,
        handler::{
            CommandHandler, CommandVisitor, CompletionError, CompletionResult, CompletionVisitor,
            ComponentVisitor, HandlerError, IntoErr, ModalVisitor, RpcHandler,
        },
        response,
        response::{
            prelude::*, ButtonStyle, Embed, Message, MessageComponent, MessageOpts, Modal,
            ModalSource, TextInput,
        },
        rpc, visitor,
    };
    pub(super) use serenity::{
        client::Context,
        model::{channel::Attachment, id::GuildId, user::User},
    };

    pub use super::{CommandOpts, ComponentKey, ModalKey, Schema};
    pub use crate::{
        prelude::*,
        proto::{
            component, component::component::Payload as ComponentPayload, modal,
            modal::modal::Payload as ModalPayload,
        },
    };

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

    #[inline]
    pub fn id<T>(t: T) -> T { t }

    pub fn http_client(timeout: Option<std::time::Duration>) -> reqwest::Client {
        let timeout = timeout.unwrap_or(std::time::Duration::from_secs(10));
        let client = reqwest::Client::builder()
            .user_agent("the-q")
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .timeout(timeout)
            .connect_timeout(timeout);
        client.build().unwrap()
    }
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
    use prelude::*;

    let explode = Arc::new(explode::ExplodeCommand::from(opts));
    let jpeg = Arc::new(jpeg::JpegCommand::from(opts));
    let jpeg_message = Arc::new(jpeg::JpegMessageCommand::from(opts));
    let point = Arc::new(point::PointCommand::from(opts));
    let re = Arc::new(re::ReCommand::from(opts));
    let say = Arc::new(say::SayCommand::from(opts));
    let sound = Arc::new(sound::SoundCommand::from(opts));
    let test = Arc::new(test::TestCommand::from(opts));

    Handlers {
        commands: vec![
            explode,
            jpeg,
            jpeg_message,
            point,
            re,
            say,
            test,
            Arc::clone(&sound) as Arc<dyn CommandHandler<Schema>>,
        ],
        components: vec![sound],
        modals: vec![],
    }
}
