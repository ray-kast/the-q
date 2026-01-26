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
            ComponentVisitor, DeserializeCommand, DeserializeRpc, HandlerError, IntoErr,
            ModalVisitor, RpcHandler,
        },
        response,
        response::{
            prelude::*, ButtonStyle, Embed, Message, MessageComponent, MessageOpts, Modal,
            ModalSource, TextInput,
        },
        rpc, visitor,
    };
    pub use qcore::{DeserializeCommand, DeserializeRpc};
    pub(super) use serenity::{
        client::Context,
        model::{
            channel::{Attachment, Message as MessageBase},
            id::GuildId,
            user::User,
        },
    };

    pub use super::{super::handler::HandlerCx, CommandOpts};
    pub use crate::{
        prelude::*,
        proto::{
            component, component::component::Payload as ComponentPayload, modal,
            modal::modal::Payload as ModalPayload,
        },
        rpc::{ComponentKey, ModalKey, Schema},
        util::{http_client, interaction::*},
    };
}

pub type Handlers = prelude::handler::Handlers<prelude::Schema, prelude::HandlerCx>;

mod opts {
    use crate::{prelude::*, util::rate_limit::RateLimitParams};

    // TODO: set up command names
    #[derive(Debug, clap::Args)]
    pub struct CommandOpts {
        #[arg(long, env, default_value = "q")]
        command_base: String,

        #[arg(long, env, default_value = "")]
        context_menu_base: String,

        #[arg(long, env)]
        pub image_rate_limit: RateLimitParams,
    }

    impl CommandOpts {
        pub fn command_name<N: fmt::Display + ?Sized>(&self, name: &N) -> String {
            format!("{}{name}", self.command_base)
        }

        pub fn menu_name<N: fmt::Display + ?Sized>(&self, name: &N) -> String {
            format!("{}{name}", self.context_menu_base)
        }
    }
}

pub use opts::CommandOpts;

macro_rules! command {
    ($ty:ty) => {
        Arc::new(<$ty as Default>::default())
            as Arc<dyn handler::DynCommandHandler<Schema, HandlerCx>>
    };

    (rpc $ty:ty) => {{
        let handler = Arc::new(<$ty as Default>::default());
        (
            Arc::clone(&handler) as Arc<dyn handler::DynCommandHandler<Schema, HandlerCx>>,
            handler as Arc<dyn handler::DynRpcHandler<Schema, ComponentKey, HandlerCx>>,
        )
    }};
}

// TODO: can this be attribute-macro-ified?
pub fn handlers() -> Handlers {
    use prelude::*;

    let (sound, sound_rpc) = command!(rpc sound::SoundCommand);

    Handlers {
        commands: vec![
            command!(jpeg::JpegCommand),
            command!(jpeg::JpegMessageCommand),
            command!(jpeg::JpegUserCommand),
            command!(liquid::LiquidCommand),
            command!(liquid::LiquidMessageCommand),
            command!(liquid::LiquidUserCommand),
            command!(re::ReCommand),
            command!(re::ReMessageCommand),
            command!(saturate::SaturateCommand),
            command!(saturate::SaturateMessageCommand),
            command!(saturate::SaturateUserCommand),
            command!(say::SayCommand),
            sound,
        ],
        components: vec![sound_rpc],
        modals: vec![],
    }
}
