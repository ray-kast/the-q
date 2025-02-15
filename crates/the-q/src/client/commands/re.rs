use std::process::Stdio;

use serenity::builder::CreateAttachment;
use shrec::re::kleene::Regex;
use tokio::{fs::File, io::AsyncWriteExt, process};

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
        _visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        // let target = visitor.target().message()?;

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
            trace!("{re:?}");
            let nfa = re.compile_atomic();
            let compiled_dfa = nfa.compile();
            let (atomized_dfa, _) = compiled_dfa.atomize_nodes::<u32>();
            let (dfa, _) = atomized_dfa.optimize();

            let dir = tokio::task::spawn_blocking(tempfile::tempdir)
                .await
                .context("Panicked creating temporary graph dir")?
                .context("Error creating temporary graph dir")?;
            let path = dir.path().join("graph.png");
            let mut cmd = process::Command::new("dot");
            cmd.current_dir(dir.path())
                .args(["-Grankdir=LR", "-Tpng", "-o"])
                .arg(&path)
                .stdin(Stdio::piped());
            trace!("Running GraphViz: {cmd:?}");
            let mut child = cmd.spawn().context("Error starting GraphViz")?;

            let graph = format!(
                "{}",
                dfa.dot(
                    |i| format!("{i:?}").into(),
                    |o| format!("{o:?}").into(),
                    |t| Some(format!("{t:?}").into())
                )
            );

            child
                .stdin
                .as_mut()
                .context("Error getting GraphViz stream")?
                .write_all(graph.as_bytes())
                .await
                .context("Error streaming dot to GraphViz")?;

            let out = child
                .wait_with_output()
                .await
                .context("Error invoking GraphViz")?;

            trace!("GraphViz exited with code {:?}", out.status);

            Message::plain("Compiled regular expression:").attach([CreateAttachment::file(
                &File::open(path).await.context("Error opening graph")?,
                "graph.png",
            )
            .await
            .context("Error attaching graph file")?])
        };

        let responder = responder
            .create_message(msg)
            .await
            .context("Error sending DFA message")?;

        Ok(responder.into())
    }
}
