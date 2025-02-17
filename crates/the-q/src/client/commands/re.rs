use std::{process::Stdio, str::Chars};

use serenity::{builder::CreateAttachment, utils::MessageBuilder};
use tokio::{fs::File, io::AsyncWriteExt, process};

use super::prelude::*;

type Regex<'a> = shrec::re::kleene::Regex<Chars<'a>>;

// TODO: miserably hacky, but it should work until shrec gets proper regex parsing support
mod parse {
    use std::mem;

    use super::{super::prelude::*, Chars, Regex};

    type ParseResult<'a> = Result<Vec<Regex<'a>>, Vec<Error>>;

    #[derive(Debug)]
    struct Slice(std::ops::RangeInclusive<usize>);

    impl Slice {
        fn get<'a>(&self, s: &'a str) -> &'a str { &s[self.0.clone()] }

        fn push_end(self, end: usize) -> Self {
            let (start, _) = self.0.into_inner();
            Self(start..=end)
        }

        fn zip(self, rhs: Option<Self>) -> Self {
            match rhs {
                None => self,
                Some(r) => {
                    let (ls, le) = self.0.into_inner();
                    let (rs, re) = r.0.into_inner();
                    Self(ls.min(rs)..=le.max(re))
                },
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum Delim {
        Slash,
        Tick,
        TickSlash,
    }

    #[derive(Debug, Clone, Copy)]
    enum Unop {
        Star,
        Plus,
        Opt,
    }

    #[derive(Debug, Clone, Copy)]
    enum ReOp {
        Pipe,
        Unop(Unop),
        LPar,
        RPar,
        Eof,
    }

    #[derive(Debug)]
    enum Production<'a> {
        Segment(Chars<'a>),
        Re(Regex<'a>),
    }

    impl<'a> Production<'a> {
        fn re(self) -> Regex<'a> {
            match self {
                Self::Segment(s) => Regex::Lit(s),
                Self::Re(r) => r,
            }
        }
    }

    trait HasEmpty {
        const EMPTY: Self;
    }

    impl HasEmpty for Regex<'_> {
        const EMPTY: Self = Regex::EMPTY;
    }

    impl HasEmpty for Production<'_> {
        const EMPTY: Self = Production::Re(Regex::EMPTY);
    }

    impl HasEmpty for ReStack<'_> {
        const EMPTY: Self = Self(vec![]);
    }

    impl HasEmpty for ReState<'_> {
        const EMPTY: Self = Self(None, ReStack::EMPTY);
    }

    #[derive(Debug)]
    struct WithPos<T>(Option<(T, Slice)>);

    impl<T> From<(T, Slice)> for WithPos<T> {
        fn from(value: (T, Slice)) -> Self { Self(Some(value)) }
    }

    impl<T> WithPos<T> {
        fn map<U>(self, f: impl FnOnce(T) -> U) -> WithPos<U> {
            WithPos(self.0.map(|(t, s)| (f(t), s)))
        }
    }

    impl<T: HasEmpty> WithPos<T> {
        fn unzip(self) -> (T, Option<Slice>) {
            self.0.map_or((T::EMPTY, None), |(t, s)| (t, Some(s)))
        }
    }

    #[derive(Debug)]
    enum ReStackSym<'a> {
        SegmentStart(usize),
        LParen,
        Cat(WithPos<Regex<'a>>),
        Alt(WithPos<Regex<'a>>),
    }

    #[derive(Debug)]
    struct ReStack<'a>(Vec<ReStackSym<'a>>);

    impl<'a> ReStack<'a> {
        fn shift_idx(mut self, i: usize) -> Self {
            if !matches!(self.0.last(), Some(ReStackSym::SegmentStart(_))) {
                self.0.push(ReStackSym::SegmentStart(i));
            }

            self
        }

        #[instrument(level = "trace", ret)]
        fn try_shift_op(&mut self, s: &'a str, end: Option<usize>, op: ReOp) -> Result {
            let prod = self.reduce(s, end, op)?;
            match op {
                ReOp::Pipe => self.0.push(ReStackSym::Alt(prod.map(Production::re))),
                ReOp::Unop(u) => {
                    let Some((prod, slice)) = prod.0 else {
                        bail!(
                            "Missing operand while parsing {u:?} at {:?}",
                            end.map_or("", |e| &s[..=e])
                        );
                    };

                    let (prefix, re) = match prod {
                        Production::Segment(l) => {
                            if let Some((idx, _)) = l.as_str().char_indices().last() {
                                let (pre, suf) = l.as_str().split_at(idx);
                                (Some(Regex::Lit(pre.chars())), Regex::Lit(suf.chars()))
                            } else {
                                (None, Regex::Lit(l))
                            }
                        },
                        Production::Re(r) => (None, r),
                    };

                    let inner = match u {
                        Unop::Star => Regex::Star(re.into()),
                        Unop::Plus => Regex::Cat(vec![re.clone(), Regex::Star(re.into())]),
                        Unop::Opt => Regex::Alt(vec![Regex::EMPTY, re]),
                    };

                    self.0.push(ReStackSym::Cat(
                        (
                            if let Some(prefix) = prefix {
                                Regex::Cat(vec![prefix, inner])
                            } else {
                                inner
                            },
                            slice,
                        )
                            .into(),
                    ));
                },
                ReOp::LPar => self.0.extend([
                    ReStackSym::Cat(prod.map(Production::re)),
                    ReStackSym::LParen,
                ]),
                ReOp::RPar => {
                    if matches!(self.0.last(), Some(ReStackSym::LParen)) {
                        self.0.pop();
                    } else {
                        bail!(
                            "Found extraneous right parenthesis at {:?}",
                            prod.0.map_or("", |(_, t)| t.get(s))
                        );
                    }

                    self.0.push(ReStackSym::Cat(prod.map(Production::re)));
                },
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

        fn pop_until(
            &mut self,
            s: &'a str,
            end: Option<usize>,
            stop: impl Fn(&ReStackSym<'a>) -> bool,
        ) -> Result<WithPos<Production<'a>>> {
            let mut curr = None;

            loop {
                let Some(sym) = self.0.pop() else {
                    break Ok(WithPos(curr));
                };
                if matches!(sym, ReStackSym::LParen) || curr.is_some() && stop(&sym) {
                    self.0.push(sym);
                    break Ok(WithPos(curr));
                }

                curr = match (sym, curr) {
                    (ReStackSym::SegmentStart(t), None) => {
                        let Some(end) = end else {
                            unreachable!("Invalid Segment in parser stack: no segment end given");
                        };

                        Some((Production::Segment(s[t..=end].chars()), Slice(t..=end)))
                    },
                    (ReStackSym::SegmentStart(_), Some(_)) => {
                        unreachable!("Unexpanded SegmentStart in the middle of parser stack!")
                    },
                    (ReStackSym::LParen, _) => unreachable!(),
                    (ReStackSym::Cat(r), Some((p, s))) => {
                        let (r, t) = r.unzip();
                        Some((
                            Production::Re(if let Regex::Cat(mut v) = r {
                                v.push(p.re());
                                Regex::Cat(v)
                            } else {
                                Regex::Cat(vec![r, p.re()])
                            }),
                            s.zip(t),
                        ))
                    },
                    (ReStackSym::Alt(r), Some((p, s))) => {
                        let (r, t) = r.unzip();
                        Some((
                            Production::Re(if let Regex::Alt(mut v) = r {
                                v.push(p.re());
                                Regex::Alt(v)
                            } else {
                                Regex::Alt(vec![r, p.re()])
                            }),
                            s.zip(t),
                        ))
                    },
                    (ReStackSym::Cat(r) | ReStackSym::Alt(r), None) => r.map(Production::Re).0,
                };
            }
        }

        fn reduce(
            &mut self,
            s: &'a str,
            end: Option<usize>,
            op: ReOp,
        ) -> Result<WithPos<Production<'a>>> {
            match op {
                ReOp::Pipe | ReOp::RPar | ReOp::Eof => self.pop_until(s, end, |_| false),
                ReOp::Unop(_) => self.pop_until(s, end, |_| true),
                ReOp::LPar => self.pop_until(s, end, |s| matches!(s, ReStackSym::Alt(..))),
            }
        }

        fn finish(mut self, s: &'a str, slice: Option<&Slice>) -> Result<Regex<'a>> {
            let (prod, _) = self
                .reduce(s, slice.map(|s| *s.0.end()), ReOp::Eof)?
                .unzip();
            let Self(stack) = self;

            if !stack.is_empty() {
                unreachable!(
                    "Trailing symbols in parser stack for {:?}: {stack:?}",
                    slice.map_or("", |t| t.get(s))
                );
            }

            Ok(prod.re())
        }
    }

    #[derive(Debug)]
    struct ReState<'a>(Option<Slice>, ReStack<'a>);

    impl<'a> ReState<'a> {
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
                '*' => self.shift_op(s, i, ReOp::Unop(Unop::Star), res),
                '+' => self.shift_op(s, i, ReOp::Unop(Unop::Plus), res),
                '?' => self.shift_op(s, i, ReOp::Unop(Unop::Opt), res),
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

fn print_errs(errs: Vec<Error>, b: &mut MessageBuilder) -> &mut MessageBuilder {
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
                Ok(r) => {
                    assert!(r.len() == 1);
                    Message::plain("").attach(graph_res(r).await?)
                },
                Err(e) => Message::rich(|b| {
                    print_errs(e, b.push_line("Errors encountered while parsing regex:"))
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
                    print_errs(e, b.push_line("Errors encountered while parsing regexes:"))
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
