use super::prelude::*;

#[derive(Debug, Default)]
pub struct SayCommand;

#[async_trait]
impl Handler<Schema> for SayCommand {
    fn register_global(&self, _: &handler::Opts) -> CommandInfo {
        CommandInfo::build_slash("qsay", "say something!", |a| {
            a.string("message", "The message to send", true, ..)
        })
        .unwrap()
    }

    async fn respond<'a>(
        &self,
        ctx: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let msg = visitor.visit_string("message")?.required()?;
        let guild = visitor.guild().optional();

        let color = guild.and_then(|(_, m)| m.colour(&ctx.cache));

        Ok(responder
            .create_message(Embed::default().desc_plain(msg).color_opt(color).into())
            .await
            .context("Error speaking message")?
            .into())
    }
}
