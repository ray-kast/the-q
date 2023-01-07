use serenity::{
    model::{
        application::interaction::{Interaction, InteractionResponseType},
        gateway::Ready,
    },
    prelude::*,
};

use super::{command, commands};
use crate::prelude::*;

pub struct Handler {
    registry: command::Registry,
}

impl Handler {
    pub fn new_rc(command_opts: command::handler::Opts) -> Arc<Self> {
        Arc::new(Self {
            registry: command::Registry::new(command_opts, commands::list()),
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
            Interaction::ApplicationCommand(aci) => self.registry.handle(&ctx, aci).await,
            Interaction::MessageComponent(m) => {
                handler(
                    "Interaction::MessageComponent",
                    m.create_interaction_response(&ctx.http, |r| {
                        r.kind(InteractionResponseType::UpdateMessage)
                            .interaction_response_data(|d| d)
                    })
                    .map(|r| r.context("Failed to respond to message component")),
                )
                .await;
            },
            Interaction::Autocomplete(a) => {
                handler(
                    "Interaction::Autocomplete",
                    a.create_autocomplete_response(&ctx.http, |r| {
                        r.add_string_choice("fucc", "fucc")
                    })
                    .map(|r| r.context("Failed to fulfill autocomplete")),
                )
                .await;
            },
            Interaction::ModalSubmit(m) => {
                handler(
                    "Interaction::ModalSubmit",
                    m.create_interaction_response(&ctx.http, |r| {
                        r.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|d| {
                                d.ephemeral(true).content("Success (probably)!")
                            })
                    })
                    .map(|r| r.context("Failed to respond to modal")),
                )
                .await;
            },
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
