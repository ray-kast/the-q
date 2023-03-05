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
            let re = Regex::Cat(vec![
                Regex::Alt(vec![
                    Regex::Cat(vec![
                        Regex::Lit("k".chars()),
                        Regex::Alt(vec![Regex::Lit("i".chars()), Regex::Lit("a".chars())]),
                        Regex::Alt(vec![Regex::Lit("m".chars()), Regex::Lit("t".chars())]),
                    ]),
                    Regex::Lit("ban".chars()),
                ]),
                Regex::Alt(vec![
                    Regex::Cat(vec![
                        Regex::Lit("o".chars()),
                        Regex::Star(Regex::Lit("no".chars()).into()),
                    ]),
                    Regex::Cat(vec![
                        Regex::Lit("a".chars()),
                        Regex::Star(Regex::Lit("na".chars()).into()),
                    ]),
                ]),
            ]);
            debug!("{re:#?}");
            let nfa = re.compile();
            debug!("{nfa:#?}");
            let compiled_dfa = nfa.compile();
            debug!("{compiled_dfa:#?}");
            let atomized_dfa = compiled_dfa.atomize_nodes::<u32>();
            debug!("{atomized_dfa:#?}");

            Message::rich(|m| m.push_codeblock_safe(format!("{atomized_dfa:?}"), None))
        };

        let responder = responder
            .create_message(msg)
            .await
            .context("Error sending DFA message")?;

        Ok(responder.into())
    }
}
