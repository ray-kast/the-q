use super::prelude::*;

#[derive(Debug)]
pub struct PointCommand;

#[async_trait]
impl Handler for PointCommand {
    fn register(&self, _: &handler::Opts, cmd: &mut CreateApplicationCommand) -> Option<GuildId> {
        cmd.kind(CommandType::Message).name("Point and Laugh");
        None
    }

    async fn respond(&self, _: &Context, visitor: &mut Visitor) -> CommandResult {
        let target = visitor.target().message()?;

        Ok(Response::Message(
            Message::rich(|b| {
                b.mention(&target.author)
                    .push("Embed fail, laugh at this user,")
            })
            .ping_users(vec![target.author.id]),
        ))
    }
}
