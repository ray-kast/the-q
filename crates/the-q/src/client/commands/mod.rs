mod explode;
mod jpeg;
mod liquid;
mod point;
mod re;
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

    let explode = Arc::new(explode::ExplodeCommand::from(opts));
    let jpeg = Arc::new(jpeg::JpegCommand::from(opts));
    let jpeg_message = Arc::new(jpeg::JpegMessageCommand::from(opts));
    let liquid = Arc::new(liquid::LiquidCommand::from(opts));
    let liquid_message = Arc::new(liquid::LiquidMessageCommand::from(opts));
    let point = Arc::new(point::PointCommand::from(opts));
    let re = Arc::new(re::ReCommand::from(opts));
    let re_message = Arc::new(re::ReMessageCommand::from(opts));
    let say = Arc::new(say::SayCommand::from(opts));
    let sound = Arc::new(sound::SoundCommand::from(opts));
    let test = Arc::new(test::TestCommand::from(opts));

    Handlers {
        commands: vec![
            explode,
            jpeg,
            jpeg_message,
            liquid,
            liquid_message,
            point,
            re,
            re_message,
            say,
            test,
            Arc::clone(&sound) as Arc<dyn CommandHandler<Schema>>,
        ],
        components: vec![sound],
        modals: vec![],
    }
}
