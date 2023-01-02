use std::collections::HashMap;

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
    command_opts: commands::CommandOpts,
    commands: DashMap<CommandId, Box<dyn commands::CommandHandler>>,
}

impl Handler {
    pub fn new_rc(command_opts: commands::CommandOpts) -> Arc<Self> {
        Arc::new(Self {
            command_opts,
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
            let cmds = Command::get_global_application_commands(&ctx.http)
                .await
                .context("Failed to get initial command list")?;

            let mut existing: HashMap<_, _> = cmds
                .iter()
                .map(|c| ((Borrowed(c.name.as_str()), c.kind, c.guild_id), Borrowed(c)))
                .collect();

            for cmd in commands::list() {
                let mut builder = serenity::builder::CreateApplicationCommand::default();
                let scope = cmd.register(&self.command_opts, &mut builder);
                let builder = builder;

                let name = builder
                    .0
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .expect("Missing command name!")
                    .to_owned();
                let kind = builder.0.get("type").expect("Missing command type!");
                let kind = serde_json::from_value(kind.clone()).expect("Invalid command type!");

                let id = existing
                    .get(&(Borrowed(name.as_str()), kind, scope))
                    .map(|c| c.id);
                let map = serde_json::Value::from(serenity::json::hashmap_to_json_map(builder.0));

                // Shoutout to serenity for not having non-builder command methods
                let res = match (id, scope) {
                    (None, None) => {
                        debug!("Creating global command {name:?}");
                        ctx.http.create_global_application_command(&map).await
                    },
                    (None, Some(guild)) => {
                        debug!("Creating guild command {name:?} for {guild:?}");
                        ctx.http
                            .create_guild_application_command(guild.into(), &map)
                            .await
                    },
                    (Some(id), None) => {
                        debug!("Updating global command {name:?} (ID {id:?})");
                        ctx.http
                            .edit_global_application_command(id.into(), &map)
                            .await
                    },
                    (Some(id), Some(guild)) => {
                        debug!("Updating guild command {name:?} (ID {id:?}) for {guild:?}");
                        ctx.http
                            .edit_guild_application_command(guild.into(), id.into(), &map)
                            .await
                    },
                }
                .with_context(|| format!("Failed to upsert {cmd:?}"))?;

                let was_updated = id.is_some();
                let id = res.id;
                let prev = existing.insert(
                    (Owned(res.name.clone()), res.kind, res.guild_id),
                    Owned(res),
                );

                assert_eq!(prev.is_some(), was_updated);
                assert!(prev.map_or(true, |v| matches!(v, Borrowed(_))));

                assert!(self.commands.insert(id, cmd).is_none());
            }

            for cmd in cmds {
                if self.commands.contains_key(&cmd.id) {
                    continue;
                }

                debug!(
                    "Deleting unregistered command {:?} (ID {:?})",
                    cmd.name, cmd.id
                );

                Command::delete_global_application_command(&ctx.http, cmd.id)
                    .await
                    .with_context(|| format!("Failed to delete command {cmd:?}"))?;
            }

            Ok(())
        })
        .await;
    }
}
