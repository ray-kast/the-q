use super::prelude::*;

#[derive(Debug)]
pub struct SayCommand;

#[async_trait]
impl Handler for SayCommand {
    fn register(&self, _: &handler::Opts, cmd: &mut CreateApplicationCommand) -> Option<GuildId> {
        cmd.kind(CommandType::ChatInput)
            .name("qsay")
            .description("say something!")
            .create_option(|opt| {
                opt.kind(CommandOptionType::String)
                    .name("message")
                    .description("The message to send")
                    .required(true)
            });
        None
    }

    async fn respond<'a>(
        &self,
        _: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let msg = visitor.visit_string("message")?.required()?;

        Ok(responder
            .create_message(Message::plain(msg))
            .await
            .context("Error speaking message")?
            .into())
    }
}
