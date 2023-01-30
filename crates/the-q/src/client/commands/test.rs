use super::prelude::*;

#[derive(Debug, Default)]
pub struct TestCommand;

#[async_trait]
impl Handler for TestCommand {
    fn register_global(&self, _: &handler::Opts) -> CommandInfo { CommandInfo::user("Test") }

    async fn respond<'a>(
        &self,
        _: &Context,
        _: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        Ok(responder
            .modal(|s| {
                Modal::new(s, ModalPayload::Rename(modal::Rename {}), "Hi!").build_row(|r| {
                    r.build_text_long(ComponentPayload::Role(component::Role {}), "foo", id)
                })
            })
            .await
            .context("Error creating modal")?
            .into())
    }
}
