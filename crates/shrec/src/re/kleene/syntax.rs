// TODO: this is provisional

use std::{borrow::Cow, mem, str::Chars};

use crate::{free::Succ, partition_map::Partition, range_set::RangeSet, re::run::Run};

pub fn pretty(i: Partition<char>) -> Cow<'static, str> {
    fn escape(c: char) -> String {
        let s = c.escape_default().collect::<String>();
        if s.len() == s.trim().len() {
            s
        } else {
            format!("'{s}'")
        }
    }

    let (start, end) = i.bounds();

    if let Some(start) = start
        && let Some(end) = end
        && start == end
    {
        return "∅".into();
    }

    let end = end
        .map(|e| {
            // TODO: stolen from <char as Step> - wen step trait eta
            let e = e as u32;
            let mut i = e.checked_sub(1)?;

            if e >= 0xe000 && 0xe000 > i {
                i = i.checked_sub(0x800)?;
            }

            // SAFETY: res is a valid unicode scalar
            // (below 0x110000 and not in 0xD800..0xE000)
            Some(unsafe { char::from_u32_unchecked(i) })
        })
        .map(|e| e.unwrap());

    if let Some(start) = start
        && let Some(end) = end
        && start == end
    {
        escape(start).into()
    } else {
        let start = start.map(escape);
        let end = end.map(escape);

        match (start, end) {
            (None, None) => "…–…".into(),
            (None, Some(e)) => format!("…–{e}").into(),
            (Some(s), None) => format!("{s}–…").into(),
            (Some(s), Some(e)) => format!("{s}–{e}").into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError<'a> {
    #[error("Missing operant while parsing {0:?} at {1:?}")]
    MissingUnop(Unop, &'a str),
    #[error("Found extraneous right parenthesis at {0:?}")]
    TrailingParen(&'a str),
    #[error("Unclosed parenthesis at {0:?}")]
    UnclosedParen(&'a str),

    #[error("Too many dashes in character class at {0:?}")]
    ClassExtraDash(&'a str),
    #[error("Too many carets in character class at {0:?}")]
    ClassExtraCaret(&'a str),
    #[error("Unterminated character class")]
    ClassUnclosed(&'a str),

    #[error("Trailing characters inside backticks: {0:?}")]
    BacktickTrail(&'a str),
    #[error("Unclosed regular expression at {0:?}")]
    Unclosed(&'a str),
}

pub type Regex<'a> = super::Regex<Run<char, Chars<'a>>>;
pub type ParseResult<'a> = Result<Vec<Regex<'a>>, Vec<ParseError<'a>>>;

#[derive(Debug)]
struct Slice(std::ops::Range<usize>);

impl Slice {
    fn get<'a>(&self, s: &'a str) -> &'a str { &s[self.0.clone()] }

    fn push_end(self, end: usize) -> Self { Self(self.0.start..end) }

    fn zip(self, rhs: &Self) -> Self {
        if rhs.0.start == rhs.0.end {
            self
        } else {
            Self(self.0.start.min(rhs.0.start)..self.0.end.max(rhs.0.end))
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
pub enum Unop {
    Star,
    Plus,
    Opt,
}

#[derive(Debug, Clone)]
enum ReOp {
    Pipe,
    Unop(Unop),
    LPar,
    RPar,
    // HACK to force reducing early and terminate segments for char classes
    ClassHint,
    Class(RangeSet<char>),
    Eof,
}

#[derive(Debug)]
enum Production<'a> {
    Blank,
    Segment(&'a str),
    Re(Regex<'a>),
}

impl<'a> Production<'a> {
    fn re(self) -> Regex<'a> {
        match self {
            Self::Blank => Regex::EMPTY,
            Self::Segment(s) => Regex::Lit(s.into()),
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
    const EMPTY: Self = Production::Blank;
}

impl HasEmpty for ReStack<'_> {
    const EMPTY: Self = Self(vec![]);
}

#[derive(Debug)]
struct WithPos<T>(T, Slice);

impl<T> WithPos<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> WithPos<U> { WithPos(f(self.0), self.1) }
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

fn join_cat<'a>(r: Regex<'a>, s: Regex<'a>) -> Regex<'a> {
    match (r, s) {
        (Regex::Cat(mut v), Regex::Cat(w)) => {
            v.extend(w);
            Regex::Cat(v)
        },
        (Regex::Cat(mut v), p) => {
            v.push(p);
            Regex::Cat(v)
        },
        (r, Regex::Cat(mut v)) => {
            v.insert(0, r);
            Regex::Cat(v)
        },
        (r, p) => Regex::Cat(vec![r, p]),
    }
}

fn join_alt<'a>(r: Regex<'a>, s: Regex<'a>) -> Regex<'a> {
    match (r, s) {
        (Regex::Alt(mut v), Regex::Alt(w)) => {
            v.extend(w);
            Regex::Alt(v)
        },
        (Regex::Alt(mut v), p) => {
            v.push(p);
            Regex::Alt(v)
        },
        (r, Regex::Alt(mut v)) => {
            v.insert(0, r);
            Regex::Alt(v)
        },
        (r, p) => Regex::Alt(vec![r, p]),
    }
}

impl<'a> ReStack<'a> {
    fn shift_idx(mut self, i: usize) -> Self {
        if !matches!(self.0.last(), Some(ReStackSym::SegmentStart(_))) {
            self.0.push(ReStackSym::SegmentStart(i));
        }

        self
    }

    fn try_shift_op(&mut self, s: &'a str, end: usize, op: ReOp) -> Result<(), ParseError<'a>> {
        let prod = self.reduce(s, end, &op)?;
        match op {
            ReOp::Pipe => self.0.push(ReStackSym::Alt(prod.map(Production::re))),
            ReOp::Unop(u) => {
                let (prefix, re) = match prod.0 {
                    Production::Blank => {
                        return Err(ParseError::MissingUnop(u, &s[..end]));
                    },
                    Production::Segment(l) => 'pre: {
                        if let Some((idx, _)) = l.char_indices().last()
                            && idx > 0
                        {
                            let (pre, suf) = l.split_at(idx);
                            break 'pre (Some(Regex::Lit(pre.into())), Regex::Lit(suf.into()));
                        }

                        (None, Regex::Lit(l.into()))
                    },
                    Production::Re(r) => (None, r),
                };

                let inner = match u {
                    Unop::Star => Regex::Star(re.into()),
                    Unop::Plus => join_cat(re.clone(), Regex::Star(re.into())),
                    Unop::Opt => join_alt(Regex::EMPTY, re),
                };

                self.0.push(ReStackSym::Cat(WithPos(
                    if let Some(prefix) = prefix {
                        debug_assert!(
                            !matches!(prefix, Regex::Lit( Run::Run(ref l)) if l.as_str().is_empty())
                        );
                        join_cat(prefix, inner)
                    } else {
                        inner
                    },
                    prod.1,
                )));
            },
            ReOp::LPar => self.0.extend([
                ReStackSym::Cat(prod.map(Production::re)),
                ReStackSym::LParen,
            ]),
            ReOp::RPar => {
                if matches!(self.0.last(), Some(ReStackSym::LParen)) {
                    self.0.pop();
                } else {
                    return Err(ParseError::TrailingParen(prod.1.get(s)));
                }

                self.0.push(ReStackSym::Cat(prod.map(Production::re)));
            },
            ReOp::ClassHint => self.0.push(ReStackSym::Cat(prod.map(Production::re))),
            ReOp::Class(c) => self.0.push(ReStackSym::Cat(
                prod.map(|p| join_cat(p.re(), Regex::Lit(Run::Set(c)))),
            )),
            ReOp::Eof => unreachable!("Eof cannot be used with shift_op"),
        }

        Ok(())
    }

    fn shift_op(mut self, s: &'a str, end: usize, op: ReOp, res: &mut ParseResult<'a>) -> Self {
        match self.try_shift_op(s, end, op) {
            Ok(()) => (),
            Err(e) => push_err(res, e),
        }

        self
    }

    fn pop_until(
        &mut self,
        s: &'a str,
        end: usize,
        opt: bool,
        stop: impl Fn(&ReStackSym<'a>) -> bool,
    ) -> Result<WithPos<Production<'a>>, ParseError<'a>> {
        let mut curr = None::<WithPos<Production>>;

        loop {
            let Some(sym) = self.0.pop() else {
                break Ok(curr.unwrap_or(WithPos(Production::EMPTY, Slice(end..end))));
            };
            if matches!(sym, ReStackSym::LParen) || (opt || curr.is_some()) && stop(&sym) {
                self.0.push(sym);
                break Ok(curr.unwrap_or(WithPos(Production::EMPTY, Slice(end..end))));
            }

            curr = Some(match (sym, curr) {
                (ReStackSym::SegmentStart(t), None) => {
                    WithPos(Production::Segment(&s[t..end]), Slice(t..end))
                },
                (ReStackSym::SegmentStart(_), _) => {
                    unreachable!("Unexpanded SegmentStart in the middle of parser stack!")
                },
                (ReStackSym::LParen, _) => unreachable!(),
                (ReStackSym::Cat(r), Some(WithPos(p, s))) => {
                    let WithPos(r, t) = r;
                    WithPos(Production::Re(join_cat(r, p.re())), s.zip(&t))
                },
                (ReStackSym::Cat(r), None) => r.map(Production::Re),
                (ReStackSym::Alt(r), Some(WithPos(p, s))) => {
                    let WithPos(r, t) = r;
                    WithPos(Production::Re(join_alt(r, p.re())), s.zip(&t))
                },
                (ReStackSym::Alt(r), None) => r.map(|r| Production::Re(join_alt(r, Regex::EMPTY))),
            });
        }
    }

    fn reduce(
        &mut self,
        s: &'a str,
        end: usize,
        op: &ReOp,
    ) -> Result<WithPos<Production<'a>>, ParseError<'a>> {
        match op {
            ReOp::Pipe | ReOp::RPar | ReOp::Eof => self.pop_until(s, end, true, |_| false),
            ReOp::Unop(_) => self.pop_until(s, end, false, |_| true),
            ReOp::LPar | ReOp::ClassHint | ReOp::Class(_) => {
                self.pop_until(s, end, true, |s| matches!(s, ReStackSym::Alt(..)))
            },
        }
    }

    fn finish(mut self, s: &'a str, slice: &Slice) -> Result<Regex<'a>, ParseError<'a>> {
        let WithPos(prod, _) = self.reduce(s, slice.0.end, &ReOp::Eof)?;
        let Self(stack) = self;

        if let Some(trail) = stack.last() {
            match trail {
                ReStackSym::LParen => return Err(ParseError::UnclosedParen(slice.get(s))),
                ReStackSym::SegmentStart(_) | ReStackSym::Cat(_) | ReStackSym::Alt(_) => {
                    unreachable!(
                        "Trailing symbols in parser stack for {:?}: {stack:?}",
                        slice.get(s)
                    );
                },
            }
        }

        Ok(prod.re())
    }
}

#[derive(Debug)]
struct ClassState {
    start: usize,
    init: bool,
    last: Option<char>,
    ranged: bool,
    invert: bool,
    set: RangeSet<char>,
}

impl ClassState {
    const fn new(start: usize) -> Self {
        Self {
            start,
            init: true,
            last: None,
            ranged: false,
            invert: false,
            set: RangeSet::EMPTY,
        }
    }

    fn shift_char(&mut self, next: Option<char>) {
        self.init = false;

        let part: Option<Partition<_>> = match (
            mem::replace(&mut self.ranged, false),
            self.last.take(),
            next,
        ) {
            (false, c, n) => {
                self.last = n;
                c.map(|c| (c..c.succ()).into())
            },
            (true, None, None) => Some((..).into()),
            (true, None, Some(e)) => Some((..e.succ()).into()),
            (true, Some(s), None) => Some((s..).into()),
            (true, Some(s), Some(e)) if e < s => Some((e..s.succ()).into()),
            (true, Some(s), Some(e)) => Some((s..e.succ()).into()),
        };

        if let Some(part) = part {
            self.set.set(part, !self.invert);
        }
    }

    fn shift_range<'a>(&mut self, s: &'a str, res: &mut ParseResult<'a>) {
        self.init = false;

        let r = !self.ranged;
        if mem::replace(&mut self.ranged, r) {
            push_err(res, ParseError::ClassExtraDash(&s[self.start..]));
        }
    }

    fn shift_inv<'a>(&mut self, s: &'a str, res: &mut ParseResult<'a>) {
        let init = mem::replace(&mut self.init, false);

        let r = !self.invert;
        if mem::replace(&mut self.invert, r) {
            push_err(res, ParseError::ClassExtraCaret(&s[self.start..]));
        }

        if init {
            self.set = RangeSet::FULL;
        }
    }

    #[inline]
    fn finish(&mut self) -> RangeSet<char> {
        self.shift_char(None);
        mem::replace(&mut self.set, RangeSet::EMPTY)
    }
}

#[derive(Debug)]
struct ReState<'a>(Slice, ReStack<'a>, Option<ClassState>);

impl<'a> ReState<'a> {
    const fn new(i: usize) -> Self { Self(Slice(i..i), ReStack::EMPTY, None) }

    fn shift_idx(self, i: usize) -> Self {
        let Self(slice, stack, class) = self;
        Self(slice.push_end(i), stack.shift_idx(i), class)
    }

    fn shift_op(self, s: &'a str, i: usize, op: ReOp, res: &mut ParseResult<'a>) -> Self {
        let Self(slice, stack, class) = self;
        Self(slice.push_end(i), stack.shift_op(s, i, op, res), class)
    }

    fn set_class(self, i: usize, class: Option<ClassState>) -> Self {
        let Self(_, stack, _) = self;
        Self(Slice(i..i), stack, class)
    }

    fn shift(mut self, s: &'a str, i: usize, c: char, res: &mut ParseResult<'a>) -> Self {
        match self.2 {
            None => match c {
                '|' => self.shift_op(s, i, ReOp::Pipe, res),
                '*' => self.shift_op(s, i, ReOp::Unop(Unop::Star), res),
                '+' => self.shift_op(s, i, ReOp::Unop(Unop::Plus), res),
                '?' => self.shift_op(s, i, ReOp::Unop(Unop::Opt), res),
                '(' => self.shift_op(s, i, ReOp::LPar, res),
                ')' => self.shift_op(s, i, ReOp::RPar, res),
                '.' => self.shift_op(s, i, ReOp::Class(RangeSet::FULL), res),
                '[' => self
                    .shift_op(s, i, ReOp::ClassHint, res)
                    .set_class(i, Some(ClassState::new(i))),
                _ => self.shift_idx(i),
            },
            Some(ref mut l) => 'state: {
                match c {
                    '-' => l.shift_range(s, res),
                    '^' => l.shift_inv(s, res),
                    ']' => {
                        let part = l.finish();
                        break 'state self.set_class(i, None).shift_op(
                            s,
                            i,
                            ReOp::Class(part),
                            res,
                        );
                    },
                    c => l.shift_char(Some(c)),
                }

                self
            },
        }
    }

    fn finish(self, s: &'a str, i: usize) -> Result<Regex<'a>, ParseError<'a>> {
        let Self(mut slice, stack, class) = self;
        if let Some(class) = class {
            return Err(ParseError::ClassUnclosed(&s[class.start..]));
        }
        slice = slice.push_end(i);
        stack.finish(s, &slice)
    }
}

#[derive(Debug)]
enum State<'a> {
    Poison,
    Message,
    Start(Delim),
    Re(Delim, ReState<'a>),
    TickFinish(ReState<'a>, usize),
    TickFinishBlank,
    TickTrail(Slice),
}

fn push_err<'a>(res: &mut ParseResult<'a>, err: ParseError<'a>) {
    match res {
        Ok(_) => *res = Err(vec![err]),
        Err(e) => e.push(err),
    }
}

fn tick_trail_err(s: &str) -> ParseError<'_> { ParseError::BacktickTrail(s) }

pub fn scan_one(s: &str) -> ParseResult<'_> {
    let mut res = Ok(vec![]);
    let mut state = ReState::new(0);

    for (i, c) in s.char_indices() {
        state = state.shift(s, i, c, &mut res);
    }

    match (state.finish(s, s.len()), &mut res) {
        (Ok(r), Ok(v)) => v.push(r),
        (Ok(_), Err(_)) => (),
        (Err(e), r) => push_err(r, e),
    }

    res
}

pub fn scan_any(s: &str) -> ParseResult<'_> {
    fn finish<'a>(state: ReState<'a>, s: &'a str, i: usize, res: &mut ParseResult<'a>) {
        match (state.finish(s, i), res) {
            (Ok(r), Ok(v)) => v.push(r),
            (Ok(_), Err(_)) => (),
            (Err(e), r) => push_err(r, e),
        }
    }

    let mut res = Ok(vec![]);
    let mut state = State::Message;

    for (i, c) in s.char_indices() {
        state = match (mem::replace(&mut state, State::Poison), c) {
            (State::Message, '/') => State::Start(Delim::Slash),
            (State::Message, '`') => State::Start(Delim::Tick),
            (State::Message, _)
            | (State::Start(Delim::Slash), '/')
            | (State::Start(Delim::Tick), '`') => State::Message,
            (State::Start(Delim::Tick), '/') => State::Start(Delim::TickSlash),
            (State::Start(Delim::TickSlash), '/') => State::TickFinishBlank,
            (State::Start(delim), c) => State::Re(delim, ReState::new(i).shift(s, i, c, &mut res)),
            (State::Re(Delim::TickSlash, s), '/') => State::TickFinish(s, i),
            (State::Re(Delim::Slash, state), '/') | (State::Re(Delim::Tick, state), '`') => {
                finish(state, s, i, &mut res);
                State::Message
            },
            (State::TickFinish(state, i), '`') => {
                finish(state, s, i, &mut res);
                State::Message
            },
            (State::Re(d, t), c) => State::Re(d, t.shift(s, i, c, &mut res)),
            (State::TickFinish(..) | State::TickFinishBlank, _) => State::TickTrail(Slice(i..i)),
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
        State::Re(_, t) | State::TickFinish(t, _) => {
            push_err(&mut res, ParseError::Unclosed(t.0.get(s)));
        },
        State::TickTrail(t) => push_err(&mut res, tick_trail_err(t.get(s))),
        State::Poison => unreachable!("Lexer finished in poisoned state!"),
    }

    res
}

#[cfg(test)]
mod test {
    use hashbrown::HashMap;
    use proptest::prelude::*;

    use crate::{
        free::Free,
        re::{
            kleene::{self, Regex},
            run::Run,
        },
    };

    fn stringify_re(re: &Regex<std::str::Chars<'_>>, s: &mut String) {
        match re {
            Regex::Alt(v) if v.is_empty() => s.push('ϵ'),
            Regex::Alt(v) => {
                for (i, r) in v.iter().enumerate() {
                    if i != 0 {
                        s.push('|');
                    }

                    stringify_re(r, s);
                }
            },
            Regex::Cat(v) if v.is_empty() => s.push_str("()"),
            Regex::Cat(v) => {
                for r in v {
                    s.push('(');
                    stringify_re(r, s);
                    s.push(')');
                }
            },
            Regex::Star(r) => {
                s.push('(');
                stringify_re(r, s);
                s.push_str(")*");
            },
            Regex::Lit(l) => match l.as_str() {
                "" => s.push_str("()"),
                l => s.push_str(l),
            },
        }
    }

    fn assert_parse_eq(expected: Regex<Run<char, String>>, s: &str) {
        let actual = super::scan_one(s).unwrap();

        assert_eq!(
            actual
                .clone()
                .into_iter()
                .map(|r| r.map(|l| match l {
                    Run::Run(c) => Run::Run(c.as_str().into()),
                    Run::Set(s) => Run::Set(s),
                }))
                .collect::<Vec<_>>()
                .as_slice(),
            &[expected],
        );
    }

    #[inline]
    fn lit(s: &str) -> Regex<Run<char, String>> { Regex::Lit(Run::Run(s.into())) }

    #[test]
    fn bnnuy() {
        let expected = Regex::Cat(vec![
            lit("b"),
            Regex::Alt(vec![
                Regex::Cat(vec![
                    Regex::Star(lit("n").into()),
                    lit("u"),
                    lit("n"),
                    Regex::Star(lit("n").into()),
                    Regex::Star(
                        Regex::Cat(vec![
                            Regex::Star(lit("n").into()),
                            lit("u"),
                            lit("n"),
                            Regex::Star(lit("n").into()),
                        ])
                        .into(),
                    ),
                ]),
                Regex::Cat(vec![
                    lit("n"),
                    Regex::Star(lit("n").into()),
                    lit("u"),
                    Regex::Star(lit("u").into()),
                    Regex::Star(
                        Regex::Cat(vec![
                            lit("n"),
                            Regex::Star(lit("n").into()),
                            lit("u"),
                            Regex::Star(lit("u").into()),
                        ])
                        .into(),
                    ),
                ]),
            ]),
            lit("y"),
        ]);

        assert_parse_eq(expected.clone(), "b((n*un+)+|(n+u+)+)y");
    }

    proptest! {
        #[test]
        fn test_one(r in kleene::re(
            8,
            64,
            8,
            0..16,
            crate::prop::symbol_safe(),
        ).prop_filter("Regex::BOTTOM cannot be parsed", |r| *r != Regex::BOTTOM)) {
            // TODO: this Sucks
            let mut strings = HashMap::<u64, String>::new();
            let mut free = Free::from(0);
            let r = r.map(|v| {
                let id = free.fresh();
                strings.insert(id, v.into_iter().collect());
                id
            });
            let r = r.map(|i| strings[&i].chars());

            let mut s = String::new();
            stringify_re(&r, &mut s);
            let mut vec = super::scan_one(&s).unwrap_or_else(|e| panic!("Error parsing {s:?}: {e:?}"));
            let _parsed = vec.pop().unwrap();
            assert!(vec.is_empty());
            // TODO: this needs an e-graph
            // let l = parsed.map(|l| l.as_str());
            // let r = r.map(|l| l.as_str());
            // assert_eq!(l, r);
        }

        #[test]
        fn test_random_one(s in any::<String>()) {
            match super::scan_one(&s) {
                Ok(v) => assert_eq!(v.len(), 1),
                Err(e) => assert!(!e.is_empty()),
            }
        }

        #[test]
        fn test_random_any(s in any::<String>()) {
            match super::scan_one(&s) {
                Ok(_) => (),
                Err(e) => assert!(!e.is_empty()),
            }
        }
    }
}
