use super::prelude::*;

#[derive(Debug)]
pub struct SayCommand;

#[async_trait]
impl Handler for SayCommand {
    fn register_global(&self, _: &handler::Opts) -> CommandInfo {
        CommandInfo::build_slash("qsay", "say something!", |a| {
            a.string("message", "The message to send", true, ..)
        })
        .unwrap()
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
