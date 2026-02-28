use serenity::model::guild::Member;

use super::prelude::*;

#[derive(Debug, Default)]
pub struct SayCommand;

// #[derive(DeserializeCommand)]
// #[deserialize(cx = HandlerCx)]
pub struct SayArgs<'a> {
    message: &'a str,

    member: Option<&'a Member>,
}

impl<'a> DeserializeCommand<'a, HandlerCx> for SayArgs<'a> {
    type Completion = NoCompletion;

    fn register_global(cx: &HandlerCx) -> CommandInfo {
        CommandInfo::build_slash(cx.opts.command_name("say"), "Say something!", |a| {
            a.string("message", "The message to send", true, ..)
        })
        .unwrap()
    }

    fn deserialize(visitor: &mut CommandVisitor<'a>) -> Result<Self, visitor::Error> {
        Ok(Self {
            message: visitor.visit_string("message")?.required()?,
            member: visitor.guild()?.optional().map(|(_, m)| m),
        })
    }
}

impl CommandHandler<Schema, HandlerCx> for SayCommand {
    type Data<'a> = SayArgs<'a>;

    async fn respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        _cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let SayArgs { message, member } = data;

        let color = member.and_then(|m| m.colour(&serenity_cx.cache));

        Ok(responder
            .create_message(Embed::default().desc_plain(message).color_opt(color).into())
            .await
            .context("Error speaking message")?
            .into())
    }
}
