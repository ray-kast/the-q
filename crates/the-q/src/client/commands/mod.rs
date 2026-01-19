mod jpeg;
mod liquid;
mod re;
mod saturate;
mod say;
mod sound;

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

    pub use super::CommandOpts;
    pub use crate::{
        prelude::*,
        proto::{
            component, component::component::Payload as ComponentPayload, modal,
            modal::modal::Payload as ModalPayload,
        },
        rpc::{ComponentKey, ModalKey, Schema},
        util::{http_client, interaction::*},
    };

    #[inline]
    pub fn id<T>(t: T) -> T { t }
}

pub type Handlers = prelude::handler::Handlers<prelude::Schema>;

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

    let jpeg = Arc::new(jpeg::JpegCommand::from(opts));
    let jpeg_message = Arc::new(jpeg::JpegMessageCommand::from(opts));
    let jpeg_user = Arc::new(jpeg::JpegUserCommand::from(opts));
    let liquid = Arc::new(liquid::LiquidCommand::from(opts));
    let liquid_message = Arc::new(liquid::LiquidMessageCommand::from(opts));
    let liquid_user = Arc::new(liquid::LiquidUserCommand::from(opts));
    let re = Arc::new(re::ReCommand::from(opts));
    let re_message = Arc::new(re::ReMessageCommand::from(opts));
    let saturate = Arc::new(saturate::SaturateCommand::from(opts));
    let saturate_message = Arc::new(saturate::SaturateMessageCommand::from(opts));
    let saturate_user = Arc::new(saturate::SaturateUserCommand::from(opts));
    let say = Arc::new(say::SayCommand::from(opts));
    let sound = Arc::new(sound::SoundCommand::from(opts));

    Handlers {
        commands: vec![
            Arc::clone(&sound) as Arc<dyn CommandHandler<Schema>>,
            jpeg,
            jpeg_message,
            jpeg_user,
            liquid,
            liquid_message,
            liquid_user,
            re,
            re_message,
            saturate,
            saturate_message,
            saturate_user,
            say,
        ],
        components: vec![sound],
        modals: vec![],
    }
}
