use super::prelude::*;

#[derive(Debug)]
pub struct ExplodeCommand;

#[async_trait]
impl Handler for ExplodeCommand {
    fn register(&self, _: &handler::Opts, cmd: &mut CreateApplicationCommand) -> Option<GuildId> {
        cmd.kind(CommandType::User).name("Blender Explode");
        None
    }

    async fn respond(&self, _: &Context, visitor: &mut Visitor) -> CommandResult {
        let target = visitor.target().user()?;

        Ok(Response::Message(
            Message::rich(|b| b.mention(target).push(" ").push_bold("explode"))
                .ping_users(vec![target.id]),
        ))
    }
}
