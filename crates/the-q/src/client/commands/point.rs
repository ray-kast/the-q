use super::prelude::*;

#[derive(Debug)]
pub struct PointCommand {
    name: String,
}

impl From<&CommandOpts> for PointCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Point and Laugh", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl Handler<Schema> for PointCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::message(&self.name) }

    async fn respond<'a>(
        &self,
        _: &Context,
        visitor: &mut CommandVisitor<'_>,
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
