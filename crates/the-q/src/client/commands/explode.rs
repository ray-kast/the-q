use super::prelude::*;

#[derive(Debug, Default)]
pub struct ExplodeCommand;

#[async_trait]
impl Handler for ExplodeCommand {
    fn register_global(&self, _: &handler::Opts) -> CommandInfo {
        CommandInfo::user("Blender Explode")
    }

    async fn respond<'a>(
        &self,
        _: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let (target, _memb) = visitor.target().user()?;

        Ok(responder
            .create_message(
                Message::rich(|b| b.mention(target).push(" ").push_bold("explode"))
                    .ping_users(vec![target.id]),
            )
            .await
            .context("Error casting blender explode")?
            .into())
    }
}
