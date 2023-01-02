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

    pub(super) use super::{CommandHandler, CommandOpts};
    pub use crate::prelude::*;
}

#[derive(Debug, clap::Args)]
pub struct CommandOpts {
    /// Base command name to prefix all slash commands with
    #[arg(long, env, default_value = "q")]
    command_base: String,
}

#[async_trait::async_trait]
pub trait CommandHandler: std::fmt::Debug + Send + Sync {
    fn register(
        &self,
        opts: &CommandOpts,
        cmd: &mut prelude::CreateApplicationCommand,
    ) -> Option<prelude::GuildId>;

    async fn respond(
        &self,
        ctx: &prelude::Context,
        cmd: prelude::ApplicationCommandInteraction,
    ) -> prelude::Result;
}

pub fn list() -> [Box<dyn CommandHandler>; 2] {
    [Box::new(vc::VcCommand), Box::new(test::TestCommand)]
}
