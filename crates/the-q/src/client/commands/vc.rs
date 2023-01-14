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

    async fn respond<'a>(
        &self,
        ctx: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let gid = visitor.guild().required()?;
        let user = visitor.user();

        let guild = gid.to_guild_cached(&ctx.cache).context("Missing guild")?;

        let Some(voice_chan) = guild.voice_states.get(&user.id).and_then(|s| s.channel_id)
        else {
            return Err(responder
                .create_message(
                    Message::plain("Please connect to a voice channel first.").ephemeral(true),
                )
                .await
                .context("Error sending voice channel error")?
                .into_err("Error getting user voice state"));
        };

        let responder = responder
            .defer_message(MessageOpts::default().ephemeral(true))
            .await
            .context("Error sending deferred message")?;

        let sb = songbird::get(ctx)
            .await
            .context("Missing songbird context")?;

        let (call, res) = sb.join(gid, voice_chan).await;

        if let Err(err) = res {
            warn!(?err, "Unable to join voice channel");
            responder
                .edit(MessageBody::plain("Couldn't join that channel, sorry."))
                .await
                .context("Error sending channel join error")?;

            return Err(responder.into_err("Couldn't join call (missing permissions?)"));
        }

        call.lock()
            .await
            .leave()
            .await
            .context("Failed to leave call")?;

        responder
            .edit(MessageBody::plain(";)"))
            .await
            .context("Error updating deferred response")?;

        Ok(responder.into())
    }
}
