use serenity::{
    model::{
        application::{
            command::Command,
            interaction::{Interaction, InteractionResponseType},
        },
        gateway::Ready,
    },
    prelude::*,
};

use crate::prelude::*;

pub struct Handler;

impl Handler {
    pub fn new_rc() -> Arc<Self> { Arc::new(Self) }
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
                Interaction::ApplicationCommand(c) => {
                    let Some(gid) = c.guild_id else {
                        if cfg!(debug_assertions) {
                            warn!("Command interaction {c:?} with no guild!");
                        }

                        return Ok(());
                    };

                    let guild = gid.to_guild_cached(&ctx.cache).context("Missing guild")?;

                    let Some(voice_chan) = guild
                        .voice_states
                        .get(&c.user.id)
                        .and_then(|s| s.channel_id) else {
                        c.create_interaction_response(&ctx.http, |res| {
                            res.kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|msg| {
                                    msg.content("Please connect to a voice channel first.")
                                })
                        })
                        .await
                        .context("Failed to respond with error to interaction")?;

                        return Ok(());
                    };

                    let sb = songbird::get(&ctx)
                        .await
                        .context("Missing songbird context")?;

                    let (call, res) = sb.join(gid, voice_chan).await;

                    res.context("Failed to join call")?;

                    call.lock()
                        .await
                        .leave()
                        .await
                        .context("Failed to leave call")?;

                    c.create_interaction_response(&ctx.http, |res| {
                        res.kind(InteractionResponseType::ChannelMessageWithSource)
                            .interaction_response_data(|msg| msg.content(";)"))
                    })
                    .await
                    .context("Failed to respond to interaction")?;
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
            let cmd = Command::create_global_application_command(&ctx.http, |cmd| {
                cmd.name("q").description(";)")
            })
            .await
            .context("Failed to register global command")?;

            info!("Registered {cmd:?}");

            Ok(())
        })
        .await;
    }
}
