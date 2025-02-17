use std::{process::Stdio, str::Chars};

use serenity::builder::CreateAttachment;
use tokio::{fs::File, io::AsyncWriteExt, process};

use super::prelude::*;

type Regex<'a> = shrec::re::kleene::Regex<Chars<'a>>;

// TODO: miserably hacky, but it should work until shrec gets proper regex parsing support
mod parse {
    use std::mem;

    use super::{super::prelude::*, Regex};

    type ParseResult<'a> = Result<Vec<Regex<'a>>, Vec<Error>>;

    #[derive(Debug)]
    struct Slice(std::ops::RangeInclusive<usize>);

    impl Slice {
        fn get<'a>(&self, s: &'a str) -> &'a str { &s[self.0.clone()] }

        fn push_end(self, end: usize) -> Self {
            let (start, _) = self.0.into_inner();
            Self(start..=end)
        }

        fn zip(lhs: Option<Self>, rhs: Option<Self>) -> Option<Self> {
            Some(match (lhs, rhs) {
                (None, None) => return None,
                (Some(l), None) => l,
                (None, Some(r)) => r,
                (Some(l), Some(r)) => {
                    let (ls, le) = l.0.into_inner();
                    let (rs, re) = r.0.into_inner();
                    Self(ls.min(rs)..=le.max(re))
                },
            })
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum Delim {
        Slash,
        Tick,
        TickSlash,
    }

    #[derive(Debug, Clone, Copy)]
    enum ReOp {
        Pipe,
        Star,
        LPar,
        RPar,
        Eof,
    }

    #[derive(Debug)]
    enum ReStackSym<'a> {
        Segment(usize),
        Paren,
        Cat(Regex<'a>, Option<Slice>),
        Alt(Regex<'a>, Option<Slice>),
    }

    #[derive(Debug)]
    struct ReStack<'a>(Vec<ReStackSym<'a>>);

    impl<'a> ReStack<'a> {
        const EMPTY: Self = Self(vec![]);

        fn shift_idx(mut self, i: usize) -> Self {
            if !matches!(self.0.last(), Some(ReStackSym::Segment(_))) {
                self.0.push(ReStackSym::Segment(i));
            }

            self
        }

        fn try_shift_op(&mut self, s: &'a str, end: Option<usize>, op: ReOp) -> Result {
            let (re, slice) = self.reduce(s, end, op)?;
            match op {
                ReOp::Pipe => self.0.push(ReStackSym::Alt(re, slice)),
                // TODO: make this associative to just one char, not the whole slice
                ReOp::Star => self.0.push(ReStackSym::Cat(Regex::Star(re.into()), slice)),
                ReOp::LPar => self
                    .0
                    .extend([ReStackSym::Cat(re, slice), ReStackSym::Paren]),
                ReOp::RPar => self.0.push(ReStackSym::Cat(re, slice)),
                ReOp::Eof => unreachable!("Eof cannot be used with shift_op"),
            }

            Ok(())
        }

        fn shift_op(
            mut self,
            s: &'a str,
            end: Option<usize>,
            op: ReOp,
            res: &mut ParseResult,
        ) -> Self {
            match self.try_shift_op(s, end, op) {
                Ok(()) => (),
                Err(e) => push_err(res, e),
            }

            self
        }

        fn reduce(
            &mut self,
            s: &'a str,
            end: Option<usize>,
            op: ReOp,
        ) -> Result<(Regex<'a>, Option<Slice>)> {
            let mut slice = None::<Slice>;
            let mut re = None::<Regex>;

            while let Some(sym) = self.0.pop() {
                let (r, s) = match (sym, op) {
                    (ReStackSym::Segment(t), _) => {
                        if let Some(re) = re {
                            unreachable!(
                                "Invalid Segment in parser stack: regex {re:?} already reduced"
                            );
                        }

                        let Some(end) = end else {
                            unreachable!("Invalid Segment in parser stack: no segment end given");
                        };

                        (Regex::Lit(s[t..=end].chars()), Some(Slice(t..=end)))
                    },
                    (ReStackSym::Paren, ReOp::RPar) => break,
                    (ReStackSym::Paren, ReOp::Eof) => {
                        bail!(
                            "Unclosed parentheses at {:?}",
                            slice.map_or("", |t| t.get(s))
                        )
                    },
                    (ReStackSym::Cat(r, t), ReOp::Pipe | ReOp::LPar | ReOp::RPar | ReOp::Eof) => (
                        if let Some(re) = re {
                            if let Regex::Cat(mut v) = r {
                                v.push(re);
                                Regex::Cat(v)
                            } else {
                                Regex::Cat(vec![r, re])
                            }
                        } else {
                            r
                        },
                        Slice::zip(t, slice),
                    ),
                    (ReStackSym::Alt(r, t), ReOp::Pipe | ReOp::RPar | ReOp::Eof) => (
                        if let Some(re) = re {
                            if let Regex::Alt(mut v) = r {
                                v.push(re);
                                Regex::Alt(v)
                            } else {
                                Regex::Alt(vec![r, re])
                            }
                        } else {
                            r
                        },
                        Slice::zip(t, slice),
                    ),
                    (o, _) => {
                        self.0.push(o);
                        break;
                    },
                };
                re = Some(r);
                slice = s;
            }

            Ok((re.unwrap_or(Regex::EMPTY), slice))
        }

        fn finish(mut self, s: &'a str, slice: Option<&Slice>) -> Result<Regex<'a>> {
            let (re, _) = self.reduce(s, slice.map(|s| *s.0.end()), ReOp::Eof)?;
            let Self(stack) = self;

            if !stack.is_empty() {
                unreachable!(
                    "Trailing symbols in parser stack for {:?}: {stack:?}",
                    slice.map_or("", |t| t.get(s))
                );
            }

            Ok(re)
        }
    }

    #[derive(Debug)]
    struct ReState<'a>(Option<Slice>, ReStack<'a>);

    impl<'a> ReState<'a> {
        const EMPTY: Self = Self(None, ReStack::EMPTY);

        fn shift_idx(self, i: usize) -> Self {
            let Self(slice, stack) = self;
            Self(
                Some(slice.map_or(Slice(i..=i), |s| s.push_end(i))),
                stack.shift_idx(i),
            )
        }

        fn shift_op(self, s: &'a str, i: usize, op: ReOp, res: &mut ParseResult) -> Self {
            let Self(slice, stack) = self;
            let seg_end = slice.as_ref().map(|s| *s.0.end());
            Self(
                Some(slice.map_or(Slice(i..=i), |s| s.push_end(i))),
                stack.shift_op(s, seg_end, op, res),
            )
        }

        fn shift(self, s: &'a str, i: usize, c: char, res: &mut ParseResult) -> Self {
            match c {
                '|' => self.shift_op(s, i, ReOp::Pipe, res),
                '*' => self.shift_op(s, i, ReOp::Star, res),
                '(' => self.shift_op(s, i, ReOp::LPar, res),
                ')' => self.shift_op(s, i, ReOp::RPar, res),
                _ => self.shift_idx(i),
            }
        }

        fn finish(self, s: &'a str) -> Result<Regex<'a>, Error> {
            let Self(slice, stack) = self;
            stack.finish(s, slice.as_ref())
        }
    }

    #[derive(Debug)]
    enum State<'a> {
        Poison,
        Message,
        Start(Delim),
        Re(Delim, ReState<'a>),
        TickFinish(ReState<'a>),
        TickFinishBlank,
        TickTrail(Slice),
    }

    fn push_err(res: &mut ParseResult, err: Error) {
        match res {
            Ok(_) => *res = Err(vec![err]),
            Err(e) => e.push(err),
        }
    }

    fn tick_trail_err(s: &str) -> Error { anyhow!("Trailing characters inside backticks: {s:?}") }

    pub fn scan_one(s: &str) -> ParseResult {
        let mut res = Ok(vec![]);
        let mut state = ReState::EMPTY;

        for (i, c) in s.char_indices() {
            state = state.shift(s, i, c, &mut res);
        }

        match (state.finish(s), &mut res) {
            (Ok(r), Ok(v)) => v.push(r),
            (Ok(_), Err(_)) => (),
            (Err(e), r) => push_err(r, e),
        }

        res
    }

    pub fn scan_any(s: &str) -> ParseResult {
        let mut res = Ok(vec![]);
        let mut state = State::Message;

        for (i, c) in s.char_indices() {
            state = match (mem::replace(&mut state, State::Poison), c) {
                (State::Message, '/') => State::Start(Delim::Slash),
                (State::Message, '`') => State::Start(Delim::Tick),
                (State::Message, _) => State::Message,
                (State::Start(Delim::Slash), '/') | (State::Start(Delim::Tick), '`') => {
                    State::Message
                },
                (State::Start(Delim::Tick), '/') => State::Start(Delim::TickSlash),
                (State::Start(Delim::TickSlash), '/') => State::TickFinishBlank,
                (State::Start(delim), c) => {
                    State::Re(delim, ReState::EMPTY.shift(s, i, c, &mut res))
                },
                (State::Re(Delim::TickSlash, s), '/') => State::TickFinish(s),
                (State::Re(Delim::Slash, state), '/')
                | (State::Re(Delim::Tick, state) | State::TickFinish(state), '`') => {
                    match (state.finish(s), &mut res) {
                        (Ok(r), Ok(v)) => v.push(r),
                        (Ok(_), Err(_)) => (),
                        (Err(e), r) => push_err(r, e),
                    }
                    State::Message
                },
                (State::Re(d, t), c) => State::Re(d, t.shift(s, i, c, &mut res)),
                (State::TickFinish(_) | State::TickFinishBlank, _) => {
                    State::TickTrail(Slice(i..=i))
                },
                (State::TickTrail(t), '`') => {
                    push_err(&mut res, tick_trail_err(t.get(s)));
                    State::Message
                },
                (State::TickTrail(t), _) => State::TickTrail(t.push_end(i)),
                (State::Poison, _) => unreachable!("Lexer was poisoned!"),
            }
        }

        match state {
            State::Message | State::Start(_) | State::TickFinishBlank => (),
            State::Re(_, t) | State::TickFinish(t) => push_err(
                &mut res,
                anyhow!(
                    "Unclosed regular expression at {:?}",
                    t.0.map_or("", |t| t.get(s))
                ),
            ),
            State::TickTrail(t) => push_err(&mut res, tick_trail_err(t.get(s))),
            State::Poison => unreachable!("Lexer finished in poisoned state!"),
        }

        res
    }
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

    let nfa = re.compile_atomic();
    let dfa = nfa.compile();
    let (dfa, _) = dfa.atomize_nodes::<u32>();
    let (dfa, ..) = dfa.optimize();

    let path = dir.path().join(format!("graph{i}.png"));

    let mut cmd = process::Command::new("dot");
    cmd.current_dir(dir.path())
        .args(["-Grankdir=LR", "-Gdpi=288", "-Tpng", "-o"])
        .arg(&path)
        .stdin(Stdio::piped());

    trace!("Running GraphViz: {cmd:?}");

    let mut child = cmd.spawn().context("Error starting GraphViz")?;
    let graph = format!(
        "{}",
        dfa.dot(
            |i| format!("{i:?}").into(),
            |_: &usize| "".into(),
            |t| {
                let () = t.iter().copied().collect();
                None
            }
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

        let msg = {
            match parse::scan_one(regex) {
                Ok(r) if r.is_empty() => {
                    Message::plain("No regular expressions detected.").ephemeral(true)
                },
                Ok(r) => {
                    Message::plain("Compiled regular expressions:").attach(graph_res(r).await?)
                },
                Err(e) => Message::rich(|b| {
                    b.push_line("Errors encountered while parsing regexes:");

                    for e in e {
                        b.push("- ")
                            .push_bold("ERROR:")
                            .push(" ")
                            .push_line_safe(e.to_string());
                    }

                    b
                }),
            }
        };

        let responder = responder
            .create_message(msg)
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

        let msg = {
            match parse::scan_any(&target.content) {
                Ok(r) if r.is_empty() => {
                    Message::plain("No regular expressions detected.").ephemeral(true)
                },
                Ok(r) => {
                    Message::plain("Compiled regular expressions:").attach(graph_res(r).await?)
                },
                Err(e) => Message::rich(|b| {
                    b.push_line("Errors encountered while parsing regexes:");

                    for e in e {
                        b.push("- ")
                            .push_bold("ERROR:")
                            .push(" ")
                            .push_line_safe(e.to_string());
                    }

                    b
                }),
            }
        };

        let responder = responder
            .create_message(msg)
            .await
            .context("Error sending DFA message")?;

        Ok(responder.into())
    }
}
