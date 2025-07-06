use std::process::Stdio;

use serenity::{builder::CreateAttachment, utils::MessageBuilder};
use shrec::{
    autom::NoToken,
    re::kleene::syntax::{pretty, scan_any, scan_one, ParseError, Regex},
};
use tokio::{fs::File, io::AsyncWriteExt, process};

use super::prelude::*;

fn print_errs<'a>(errs: Vec<ParseError<'_>>, b: &'a mut MessageBuilder) -> &'a mut MessageBuilder {
    for e in errs {
        b.push("- ")
            .push_bold("ERROR:")
            .push(" ")
            .push_line_safe(e.to_string());
    }

    b
}

async fn graph_res(
    res: impl IntoIterator<Item = Regex<'_>, IntoIter: ExactSizeIterator>,
) -> Result<Vec<CreateAttachment>> {
    let dir = tokio::task::spawn_blocking(tempfile::tempdir)
        .await
        .context("Panicked creating temporary graph dir")?
        .context("Error creating temporary graph dir")?;

    let mut graphs = vec![];
    for (i, re) in res.into_iter().enumerate().take(10) {
        graphs.push(graph_re(&dir, i, re).await?);
    }

    Ok(graphs)
}

async fn graph_re(dir: &tempfile::TempDir, i: usize, re: Regex<'_>) -> Result<CreateAttachment> {
    trace!("{re:?}");

    let nfa = re.compile();
    let dfa = nfa.compile_moore();
    let (dfa, ..) = dfa.optimize();

    let path = dir.path().join(format!("graph{i}.png"));

    let graph = format!(
        "{}",
        dfa.dot(
            |_: usize| "".into(),
            |i| pretty(i.copied()),
            |_: &BTreeSet<()>| None,
            |t: &NoToken| match *t {},
        )
    );

    let mut cmd = process::Command::new("dot");
    cmd.current_dir(dir.path())
        .args(["-Grankdir=LR", "-Gdpi=288", "-Tpng", "-o"])
        .arg(&path)
        .stdin(Stdio::piped());

    trace!("Running GraphViz: {cmd:?}");

    let mut child = cmd.spawn().context("Error starting GraphViz")?;
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

    CreateAttachment::file(
        &File::open(path).await.context("Error opening graph")?,
        "graph.png",
    )
    .await
    .context("Error attaching graph file")
}

#[derive(Debug)]
pub struct ReCommand {
    name: String,
}

impl From<&CommandOpts> for ReCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}regex", opts.command_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for ReCommand {
    fn register_global(&self) -> CommandInfo {
        CommandInfo::build_slash(
            &self.name,
            "Compiles and visualizes a regular expression",
            |a| a.string("regex", "The regular expression to compile", true, ..),
        )
        .unwrap()
    }

    async fn respond<'a>(
        &self,
        _: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let regex = visitor.visit_string("regex")?.required()?;

        let responder = responder
            .defer_message(MessageOpts::default())
            .await
            .context("Error sending deferred message")?;

        let msg = {
            match scan_one(regex) {
                Ok(r) => {
                    assert!(r.len() == 1);
                    Message::plain("").attach(graph_res(r).await?)
                },
                Err(e) => Message::rich(|b| {
                    print_errs(e, b.push_line("Errors encountered while parsing regex:"))
                }),
            }
        };

        responder
            .create_followup(msg)
            .await
            .context("Error sending DFA message")?;

        Ok(responder.into())
    }
}

#[derive(Debug)]
pub struct ReMessageCommand {
    name: String,
}

impl From<&CommandOpts> for ReMessageCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Compile Regexes", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for ReMessageCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::message(&self.name) }

    async fn respond<'a>(
        &self,
        _: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let target = visitor.target().message()?;

        let responder = responder
            .defer_message(MessageOpts::default())
            .await
            .context("Error sending deferred message")?;

        let msg = {
            match scan_any(&target.content) {
                Ok(r) if r.is_empty() => {
                    Message::plain("No regular expressions detected.").ephemeral(true)
                },
                Ok(r) => {
                    Message::plain("Compiled regular expressions:").attach(graph_res(r).await?)
                },
                Err(e) => Message::rich(|b| {
                    print_errs(e, b.push_line("Errors encountered while parsing regexes:"))
                }),
            }
        };

        responder
            .create_followup(msg)
            .await
            .context("Error sending DFA message")?;

        Ok(responder.into())
    }
}
