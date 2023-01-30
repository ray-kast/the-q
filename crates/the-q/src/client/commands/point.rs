use super::prelude::*;

#[derive(Debug, Default)]
pub struct PointCommand;

#[async_trait]
impl Handler for PointCommand {
    fn register_global(&self, _: &handler::Opts) -> CommandInfo {
        CommandInfo::message("Point and Laugh")
    }

    async fn respond<'a>(
        &self,
        _: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let target = visitor.target().message()?;

        Ok(responder
            .create_message(
                Message::rich(|b| {
                    b.mention(&target.author)
                        .push("Embed fail, laugh at this user,")
                })
                .ping_users(vec![target.author.id]),
            )
            .await
            .context("Embed fail, laugh at this user")?
            .into())
    }
}
