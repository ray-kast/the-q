use super::prelude::*;

#[derive(Debug)]
pub struct TestCommand;

#[async_trait]
impl Handler for TestCommand {
    fn register(&self, _: &handler::Opts, cmd: &mut CreateApplicationCommand) -> Option<GuildId> {
        cmd.name("Test").kind(CommandType::User);
        None
    }

    async fn respond<'a>(
        &self,
        _: &Context,
        _: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        Ok(responder
            .modal(|s| Modal::new(s, modal::modal::Payload::Rename(modal::Rename {}), "Hi!"))
            .await
            .context("Error creating modal")?
            .into())
    }
}
