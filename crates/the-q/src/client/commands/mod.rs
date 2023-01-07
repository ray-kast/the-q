// For debug printing every mod <x> should export <X>Command
#![allow(clippy::module_name_repetitions)]

mod test;
mod vc;

pub(self) mod prelude {
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

    pub(super) use super::super::command::{
        handler,
        handler::{Error, Handler, Response, Result},
    };
    pub use crate::prelude::*;
}

pub fn list() -> Vec<Box<dyn prelude::Handler>> {
    vec![Box::new(vc::VcCommand), Box::new(test::TestCommand)]
}
