mod explode;
mod point;
mod say;
mod test;
mod vc;

pub(self) mod prelude {
    // TODO: minimize these
    pub(super) use serenity::{
        builder::CreateApplicationCommand,
        model::{
            application::command::{CommandOptionType, CommandType},
            id::GuildId,
        },
        prelude::*,
    };

    pub use super::super::command::{
        handler,
        handler::{
            CommandError, CommandHandler as Handler, CommandResponder, CommandResult, IntoErr,
        },
        response::{Message, MessageBody, MessageOpts, Modal, ResponseData},
        visitor,
        visitor::Visitor,
    };
    pub use crate::prelude::*;
}

pub fn list() -> Vec<prelude::Arc<dyn prelude::Handler>> {
    use prelude::Arc;

    vec![
        Arc::new(explode::ExplodeCommand),
        Arc::new(point::PointCommand),
        Arc::new(say::SayCommand),
        Arc::new(test::TestCommand),
        Arc::new(vc::VcCommand),
    ]
}
