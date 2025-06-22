use std::mem;

use super::kleene;

mod derivative;
mod dfa_builder;

#[derive(Debug, Clone, PartialEq)]
pub enum Regex<L> {
    Con(Vec<Regex<L>>),
    Dis(Vec<Regex<L>>),
    Not(Box<Regex<L>>),
    Cat(Vec<Regex<L>>),
    Star(Box<Regex<L>>),
    Lit(L),
}

impl<L> Regex<L> {
    pub const BOTTOM: Regex<L> = Regex::Dis(Vec::new());
    pub const EMPTY: Regex<L> = Regex::Cat(Vec::new());
    pub const TOP: Regex<L> = Regex::Con(Vec::new());
}

impl<L> From<kleene::Regex<L>> for Regex<L> {
    fn from(value: kleene::Regex<L>) -> Self {
        match value {
            kleene::Regex::Alt(v) => Self::Dis(v.into_iter().map(Into::into).collect()),
            kleene::Regex::Cat(v) => Self::Cat(v.into_iter().map(Into::into).collect()),
            kleene::Regex::Star(r) => Self::Star(Box::new((*r).into())),
            kleene::Regex::Lit(l) => Self::Lit(l),
        }
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Conversion from Kleene regex to Brzozowski regex is not trivial")]
pub struct TryFromNonTrivial;

impl<L> TryFrom<Regex<L>> for kleene::Regex<L> {
    type Error = TryFromNonTrivial;

    fn try_from(value: Regex<L>) -> Result<Self, Self::Error> {
        Ok(match value {
            Regex::Dis(v) => kleene::Regex::Alt(
                v.into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
            ),
            Regex::Cat(v) => kleene::Regex::Cat(
                v.into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
            ),
            Regex::Star(r) => kleene::Regex::Star(Box::new((*r).try_into()?)),
            Regex::Lit(l) => kleene::Regex::Lit(l),
            Regex::Con(_) | Regex::Not(_) => return Err(TryFromNonTrivial),
        })
    }
}

#[expect(unused, clippy::only_used_in_recursion, reason = "WIP")]
impl<L: IntoIterator<Item: Ord>> Regex<L> {
    // TODO: honestly, it might just make more sense to work with this directly in an e-graph
    fn deriv_atomic(self, prefix: &L::Item) -> Regex<L> {
        match self {
            Self::Con(v) => Self::Con(v.into_iter().map(|r| r.deriv_atomic(prefix)).collect()),
            Self::Dis(v) => Self::Dis(v.into_iter().map(|r| r.deriv_atomic(prefix)).collect()),
            Self::Not(r) => Self::Not(Box::new((*r).deriv_atomic(prefix))),
            Self::Cat(mut v) => {
                match v.get_mut(0) {
                    Some(r) => {
                        let re = mem::replace(r, Self::BOTTOM);
                        *r = re.deriv_atomic(prefix);
                        // Self::Cat(v)
                        todo!("D_a(P)Q + d(P)D_a(Q)")
                    },
                    None => Self::BOTTOM,
                }
            },
            Self::Star(r) => todo!(),
            Self::Lit(l) => todo!(),
        }
    }

    fn compile_atomic() { todo!() }
}
