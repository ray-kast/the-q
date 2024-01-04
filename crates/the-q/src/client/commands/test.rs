use super::prelude::*;

#[derive(Debug)]
pub struct TestCommand {
    name: String,
}

impl From<&CommandOpts> for TestCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Test", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for TestCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::user(&self.name) }

    async fn respond<'a>(
        &self,
        _: &Context,
        _: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        Ok(responder
            .modal(|s| {
                Modal::new(s, ModalPayload::Rename(modal::Rename {}), "Hi!").text_long(
                    ComponentPayload::Role(component::Role {}),
                    "foo",
                    id,
                )
            })
            .await
            .context("Error creating modal")?
            .into())
    }
}
