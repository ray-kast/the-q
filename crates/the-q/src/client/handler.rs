use dashmap::DashMap;
use serenity::{
    model::{
        application::{command::Command, interaction::Interaction},
        gateway::Ready,
        id::CommandId,
    },
    prelude::*,
};

use super::commands;
use crate::prelude::*;

pub struct Handler {
    commands: DashMap<CommandId, Box<dyn commands::CommandHandler>>,
}

impl Handler {
    pub fn new_rc() -> Arc<Self> {
        Arc::new(Self {
            commands: DashMap::new(),
        })
    }
}

#[instrument(skip(f))]
async fn handler(method: &'static str, f: impl Future<Output = Result<()>>) {
    match f.await {
        Ok(()) => (),
        Err(e) => error!("Error in {method}: {e:?}"),
    }
}

#[async_trait]
impl serenity::client::EventHandler for Handler {
    async fn interaction_create(&self, ctx: Context, int: Interaction) {
        handler("interaction_create", async move {
            match int {
                Interaction::ApplicationCommand(c) => match self.commands.get(&c.data.id) {
                    Some(cmd) => cmd.respond(&ctx, c).await?,
                    None => {
                        bail!("Unrecognized command {c:?}")
                    },
                },
                i if cfg!(debug_assertions) => warn!("Unknown interaction {i:?}"),
                _ => (),
            }

            Ok(())
        })
        .await;
    }

    async fn ready(&self, ctx: Context, _: Ready) {
        handler("ready", async move {
            for cmd in commands::list() {
                let res = Command::create_global_application_command(&ctx.http, |c| {
                    cmd.register(c);
                    c
                })
                .await
                .with_context(|| format!("Failed to register command {cmd:?}"))?;

                debug!("Registered {cmd:?} under {res:?}");

                assert!(self.commands.insert(res.id, cmd).is_none());
            }

            Ok(())
        })
        .await;
    }
}
