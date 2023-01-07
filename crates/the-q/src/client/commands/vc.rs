use super::prelude::*;

#[derive(Debug)]
pub struct VcCommand;

#[async_trait]
impl Handler for VcCommand {
    fn register(
        &self,
        opts: &handler::Opts,
        cmd: &mut CreateApplicationCommand,
    ) -> Option<GuildId> {
        cmd.name(&opts.command_base)
            .description(";)")
            .kind(CommandType::ChatInput);
        None
    }

    async fn respond(&self, ctx: &Context, cmd: &ApplicationCommandInteraction) -> Result {
        let Some(gid) = cmd.guild_id else {
            if cfg!(debug_assertions) {
                warn!("Command interaction {cmd:?} with no guild!");
            }

            cmd.create_interaction_response(&ctx.http, |res| {
                res.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(|msg| {
                    msg.content("This command must be used in a server").ephemeral(true)
                })
            }).await.context("Failed to respond with error to interaction")?;

            return Ok(Response::Message);
        };

        let guild = gid.to_guild_cached(&ctx.cache).context("Missing guild")?;

        let Some(voice_chan) = guild
            .voice_states
            .get(&cmd.user.id)
            .and_then(|s| s.channel_id)
        else {
            cmd.create_interaction_response(&ctx.http, |res| {
                res.kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|msg| {
                        msg.content("Please connect to a voice channel first.").ephemeral(true)
                    })
            })
            .await
            .context("Failed to respond with error to interaction")?;

            return Ok(Response::Message);
        };

        let sb = songbird::get(ctx)
            .await
            .context("Missing songbird context")?;

        let (call, res) = sb.join(gid, voice_chan).await;

        res.context("Failed to join call")?;

        call.lock()
            .await
            .leave()
            .await
            .context("Failed to leave call")?;

        cmd.create_interaction_response(&ctx.http, |res| {
            res.kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|msg| msg.content(";)"))
        })
        .await
        .context("Failed to respond to interaction")?;

        Ok(Response::Message)
    }
}
