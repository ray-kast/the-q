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
        // TODO: modals
        // cmd.create_interaction_response(&ctx.http, |res| {
        //     res.kind(InteractionResponseType::Modal)
        //         .interaction_response_data(|mdl| {
        //             mdl.custom_id("boar shocked").title("hi").components(|c| {
        //                 c.create_action_row(|r| {
        //                     r.create_input_text(|t| {
        //                         t.custom_id("boar shocked 2")
        //                             .label("no way")
        //                             .style(InputTextStyle::Paragraph)
        //                     })
        //                 })
        //             })
        //         })
        // })
        // .await
        // .context("Failed to respond to interaction")?;

        Ok(responder
            .modal(Modal)
            .await
            .context("Error creating modal")?
            .into())
    }
}
