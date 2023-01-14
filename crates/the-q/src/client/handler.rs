use serenity::{
    model::{application::interaction::Interaction, gateway::Ready},
    prelude::*,
};

use super::{commands, interaction};
use crate::prelude::*;

pub struct Handler {
    registry: interaction::Registry,
}

impl Handler {
    pub fn new_rc(command_opts: interaction::handler::Opts) -> Arc<Self> {
        Arc::new(Self {
            registry: interaction::Registry::new(command_opts, commands::list()),
        })
    }
}

#[instrument(skip(f))]
async fn handler(method: &'static str, f: impl Future<Output = Result>) {
    match f.await {
        Ok(()) => (),
        Err(e) => error!("Error in {method}: {e:?}"),
    }
}

#[async_trait]
impl serenity::client::EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, int: Interaction) {
        match int {
            Interaction::Ping(_) => (),

            Interaction::ApplicationCommand(aci) => self.registry.handle_command(&ctx, aci).await,
            Interaction::MessageComponent(mc) => self.registry.handle_component(&ctx, mc).await,
            Interaction::Autocomplete(ac) => self.registry.handle_autocomplete(&ctx, ac).await,
            Interaction::ModalSubmit(ms) => self.registry.handle_modal(&ctx, ms).await,
        }
    }

    async fn ready(&self, ctx: Context, _: Ready) {
        handler("ready", async move {
            self.registry.init(&ctx).await?;
            Ok(())
        })
        .await;
    }
}
