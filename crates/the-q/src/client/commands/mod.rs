mod explode;
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
        handler::{CommandResult, Error, Handler, Response},
        response::{Message, MessageBody, MessageOpts},
        visitor,
        visitor::Visitor,
    };
    pub use crate::prelude::*;
}

pub fn list() -> Vec<prelude::Arc<dyn prelude::Handler>> {
    use prelude::Arc;

    vec![
        Arc::new(explode::ExplodeCommand),
        Arc::new(say::SayCommand),
        Arc::new(test::TestCommand),
        Arc::new(vc::VcCommand),
    ]
}
