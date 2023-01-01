mod q;

pub(self) mod prelude {
    pub use serenity::{
        builder::CreateApplicationCommand,
        model::application::interaction::{
            application_command::ApplicationCommandInteraction, InteractionResponseType,
        },
        prelude::*,
    };

    pub use super::CommandHandler;
    pub use crate::prelude::*;
}

#[async_trait::async_trait]
pub trait CommandHandler: std::fmt::Debug + Send + Sync {
    fn register(&self, cmd: &mut serenity::builder::CreateApplicationCommand);

    async fn respond(
        &self,
        ctx: &prelude::Context,
        cmd: prelude::ApplicationCommandInteraction,
    ) -> prelude::Result;
}

pub fn list() -> [Box<dyn CommandHandler>; 1] { [Box::new(q::QCommand)] }
