use shrec::re::Regex;

use super::prelude::*;

#[derive(Debug)]
pub struct ReCommand {
    name: String,
}

impl From<&CommandOpts> for ReCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Compile Regexes", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for ReCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::message(&self.name) }

    async fn respond<'a>(
        &self,
        _: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let target = visitor.target().message()?;

        let msg = {
            let re = Regex::Alt(vec![Regex::Lit("hi".chars()), Regex::Lit("there".chars())]);
            let nfa = re.compile_scanner();
            let dfa = nfa.compile();

            Message::rich(|m| m.push_codeblock_safe(format!("{dfa:?}"), None))
        };

        let responder = responder
            .create_message(msg)
            .await
            .context("Error sending DFA message")?;

        Ok(responder.into())
    }
}
