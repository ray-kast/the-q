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

    async fn respond(&self, ctx: &Context, visitor: &mut Visitor) -> CommandResult {
        let gid = visitor.guild().required()?;
        let user = visitor.user();

        let guild = gid.to_guild_cached(&ctx.cache).context("Missing guild")?;

        let voice_chan = guild
            .voice_states
            .get(&user.id)
            .and_then(|s| s.channel_id)
            .ok_or_else(|| {
                Message::plain("Please connect to a voice channel first.")
                    .ephemeral(true)
                    .into_err("Error getting user voice state")
            })?;

        let ctx = ctx.clone();
        let task = tokio::task::spawn(async move {
            let sb = songbird::get(&ctx)
                .await
                .context("Missing songbird context")?;

            let (call, res) = sb.join(gid, voice_chan).await;

            res.context("Failed to join call").map_err(|e| {
                warn!(?e, "Unable to join voice channel");
                MessageBody::plain("Couldn't join that channel, sorry.")
                    .into_err("Failed to join call (missing permissions?)")
            })?;

            call.lock()
                .await
                .leave()
                .await
                .context("Failed to leave call")?;

            Ok(MessageBody::plain(";)"))
        });

        Ok(Response::DeferMessage(
            MessageOpts::default().ephemeral(true),
            task,
        ))
    }
}
