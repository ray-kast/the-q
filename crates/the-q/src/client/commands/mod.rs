// For debug printing every mod <x> should export <X>Command
#![allow(clippy::module_name_repetitions)]

mod say;
mod test;
mod vc;

pub(self) mod prelude {
    // TODO: minimize these
    pub use serenity::{
        builder::{CreateApplicationCommand, CreateApplicationCommandOption},
        model::{
            application::{
                command::{CommandOptionType, CommandType},
                component::InputTextStyle,
                interaction::{
                    application_command::ApplicationCommandInteraction, InteractionResponseType,
                },
            },
            id::GuildId,
        },
        prelude::*,
    };

    pub use super::super::command::{
        handler,
        handler::{Error, Handler, Message, Response, Result},
        visitor,
        visitor::Visitor,
    };
    pub use crate::prelude::*;
}

pub fn list() -> Vec<Box<dyn prelude::Handler>> {
    vec![
        Box::new(say::SayCommand),
        Box::new(test::TestCommand),
        Box::new(vc::VcCommand),
    ]
}
