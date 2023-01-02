use super::prelude::*;

#[derive(Debug)]
pub struct TestCommand;

#[async_trait]
impl CommandHandler for TestCommand {
    fn register(&self, opts: &CommandOpts, cmd: &mut CreateApplicationCommand) -> Option<GuildId> {
        cmd.name("Test").kind(CommandType::User);
        None
    }

    async fn respond(&self, ctx: &Context, cmd: ApplicationCommandInteraction) -> Result {
        cmd.create_interaction_response(&ctx.http, |res| {
            res.kind(InteractionResponseType::Modal)
                .interaction_response_data(|mdl| mdl.title("hi"))
        })
        .await
        .context("Failed to respond to interaction")?;

        Ok(())
    }
}
