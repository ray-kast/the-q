use super::prelude::*;

#[derive(Debug)]
pub struct ExplodeCommand {
    name: String,
}

impl From<&CommandOpts> for ExplodeCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Blender Explode", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl Handler<Schema> for ExplodeCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::user(&self.name) }

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
