use paracord::interaction;
use serenity::{
    model::{application::Interaction, gateway::Ready},
    prelude::*,
};

use super::commands;
use crate::prelude::*;

#[derive(Debug)]
pub struct HandlerCx {
    pub opts: commands::CommandOpts,
    pub redis: redis::Client,
}

pub struct Handler {
    registry: interaction::Registry<crate::rpc::Schema, HandlerCx>,
    cx: HandlerCx,
}

impl Handler {
    pub fn new_rc(cx: HandlerCx) -> Arc<Self> {
        Arc::new(Self {
            registry: interaction::Registry::new(commands::handlers()),
            cx,
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

#[async_trait::async_trait]
impl serenity::client::EventHandler for Handler {
    async fn interaction_create(&self, serenity_cx: Context, int: Interaction) {
        match int {
            Interaction::Ping(_) => (),

            Interaction::Command(c) => {
                self.registry
                    .handle_command(&serenity_cx, &self.cx, c)
                    .await;
            },
            Interaction::Component(c) => {
                self.registry
                    .handle_component(&serenity_cx, &self.cx, c)
                    .await;
            },
            Interaction::Autocomplete(a) => {
                self.registry
                    .handle_autocomplete(&serenity_cx, &self.cx, a)
                    .await;
            },
            Interaction::Modal(m) => {
                self.registry.handle_modal(&serenity_cx, &self.cx, m).await;
            },

            i => warn!(interaction = ?i, "Unknown interaction"),
        }
    }

    async fn ready(&self, ctx: Context, _: Ready) {
        handler("ready", async move {
            self.registry.init(&ctx, &self.cx).await?;
            Ok(())
        })
        .await;
    }
}
