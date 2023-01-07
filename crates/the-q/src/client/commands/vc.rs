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

    async fn respond<'a>(&self, _: &Context, _: Visitor<'a>) -> Result {
        // // TODO: GuildVisitor
        // let Some(gid) = cmd.guild_id else {
        //     return Ok(Response::Message(Message::plain("This command must be used in a server").ephemeral(true)));
        // };

        // let guild = gid.to_guild_cached(&ctx.cache).context("Missing guild")?;

        // let Some(voice_chan) = guild
        //     .voice_states
        //     .get(&cmd.user.id)
        //     .and_then(|s| s.channel_id)
        // else {
        //     return Ok(Response::Message(
        //         Message::plain("Please connect to a voice channel first.").ephemeral(true),
        //     ));
        // };

        // let sb = songbird::get(ctx)
        //     .await
        //     .context("Missing songbird context")?;

        // let (call, res) = sb.join(gid, voice_chan).await;

        // res.context("Failed to join call")?;

        // call.lock()
        //     .await
        //     .leave()
        //     .await
        //     .context("Failed to leave call")?;

        // Ok(Response::Message(Message::plain(";)")))
        Err(anyhow!("todo").into())
    }
}
