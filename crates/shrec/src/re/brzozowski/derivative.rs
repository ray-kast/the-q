use super::Regex;
use crate::range_set::RangeSet;

type BoxRe<S> = Box<CanonRe<S>>;
#[derive(Debug, Clone, PartialEq)]
pub enum CanonRe<S> {
    Cat(BoxRe<S>, BoxRe<S>),
    Star(BoxRe<S>),
    Con(BoxRe<S>, BoxRe<S>),
    Dis(BoxRe<S>, BoxRe<S>),
    Not(BoxRe<S>),
    Sym(RangeSet<S>),
    Nil,
}

impl<S> CanonRe<S> {
    pub const BOTTOM: Self = Self::Sym(RangeSet::EMPTY);

    #[must_use]
    pub fn cat(self, rhs: Self) -> Self { Self::Cat(self.into(), rhs.into()) }

    #[must_use]
    pub fn star(self) -> Self { Self::Star(self.into()) }

    #[must_use]
    pub fn con(self, rhs: Self) -> Self { Self::Con(self.into(), rhs.into()) }

    #[must_use]
    pub fn dis(self, rhs: Self) -> Self { Self::Dis(self.into(), rhs.into()) }

    #[must_use]
    pub fn not(self) -> Self { Self::Not(self.into()) }

    #[must_use]
    pub fn matches_empty(&self) -> bool {
        match self {
            Self::Cat(l, r) | Self::Con(l, r) => l.matches_empty() && r.matches_empty(),
            Self::Dis(l, r) => l.matches_empty() || r.matches_empty(),
            Self::Not(c) => !c.matches_empty(),
            Self::Sym(_) => false,
            Self::Star(_) | Self::Nil => true,
        }
    }

    #[must_use]
    #[inline]
    pub fn delta<T>(&self) -> CanonRe<T> {
        if self.matches_empty() {
            CanonRe::Nil
        } else {
            CanonRe::BOTTOM
        }
    }
}

impl<S: Ord> CanonRe<S> {
    // TODO: does this need to consume self?
    #[must_use]
    pub fn derivative(self, sym: &S) -> Self
    where S: Clone + Eq {
        match self {
            Self::Cat(l, r) => l
                .delta()
                .cat(r.clone().derivative(sym))
                .dis(l.derivative(sym).cat(*r)),
            Self::Star(r) => r.as_ref().clone().derivative(sym).cat(r.star()),
            Self::Con(l, r) => l.derivative(sym).con(r.derivative(sym)),
            Self::Dis(l, r) => l.derivative(sym).dis(r.derivative(sym)),
            Self::Not(r) => r.derivative(sym).not(),
            Self::Sym(s) if s.contains(sym) => Self::Nil,
            Self::Sym(_) | Self::Nil => Self::BOTTOM,
        }
    }
}

impl<S: Clone + Ord> CanonRe<S> {
    pub fn start_set(&self) -> StartSet<S> {
        match self {
            CanonRe::Cat(l, r) => {
                let set = l.start_set();
                if set.nil {
                    set.union(&r.start_set())
                } else {
                    set
                }
            },
            CanonRe::Star(r) => r.start_set().union(&StartSet::NIL),
            CanonRe::Con(l, r) => l.start_set().intersect(&r.start_set()),
            CanonRe::Dis(l, r) => l.start_set().union(&r.start_set()),
            CanonRe::Not(r) => r.inv_start_set(),
            // TODO: this clone could probably be avoided?
            CanonRe::Sym(s) => StartSet::from(s.clone()),
            CanonRe::Nil => StartSet::NIL,
        }
    }

    // Compute Self::Not(self).start_set()
    fn inv_start_set(&self) -> StartSet<S> {
        match self {
            CanonRe::Cat(l, r) => {
                let set = l.inv_start_set();
                if set.nil {
                    set.union(&r.inv_start_set())
                } else {
                    set
                }
            },
            CanonRe::Star(r) => r.inv_start_set().intersect(&StartSet::NON_NIL),
            CanonRe::Con(l, r) => l.inv_start_set().union(&r.inv_start_set()),
            CanonRe::Dis(l, r) => l.inv_start_set().intersect(&r.inv_start_set()),
            CanonRe::Not(r) => r.start_set(),
            // TODO: this clone could probably be avoided?
            CanonRe::Sym(s) => StartSet::from(s.clone()).invert(),
            CanonRe::Nil => StartSet::NON_NIL,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct StartSet<S> {
    nil: bool,
    set: RangeSet<S>,
}

impl<S> StartSet<S> {
    pub const BOTTOM: Self = Self {
        nil: false,
        set: RangeSet::EMPTY,
    };
    pub const NIL: Self = Self {
        nil: true,
        set: RangeSet::EMPTY,
    };
    /// The inverse of [`StartSet::NIL`]
    pub const NON_NIL: Self = Self {
        nil: false,
        set: RangeSet::FULL,
    };
    /// The inverse of [`StartSet::NEVER`]
    pub const TOP: Self = Self {
        nil: true,
        set: RangeSet::FULL,
    };
}

impl<S: Clone + Ord> StartSet<S> {
    pub fn union(self, other: &Self) -> Self {
        let Self {
            nil: l_nil,
            set: l_set,
        } = self;
        let Self {
            nil: r_nil,
            set: r_set,
        } = other;

        Self {
            nil: l_nil || *r_nil,
            set: l_set.unioned(r_set),
        }
    }

    pub fn intersect(self, other: &Self) -> Self {
        let Self {
            nil: l_nil,
            set: l_set,
        } = self;
        let Self {
            nil: r_nil,
            set: r_set,
        } = other;

        Self {
            nil: l_nil && *r_nil,
            set: l_set.intersected(r_set),
        }
    }

    pub fn invert(self) -> Self {
        let Self { nil, set } = self;

        Self {
            nil: !nil,
            set: set.inverted(),
        }
    }
}

impl<S> From<RangeSet<S>> for StartSet<S> {
    fn from(set: RangeSet<S>) -> Self { Self { nil: false, set } }
}

impl<L: IntoIterator, S> From<Regex<L>> for CanonRe<S>
where L::Item: Into<RangeSet<S>>
{
    fn from(re: Regex<L>) -> Self {
        match re {
            Regex::Con(v) => v
                .into_iter()
                .map(Into::into)
                .reduce(CanonRe::con)
                .unwrap_or(CanonRe::BOTTOM.not()),
            Regex::Dis(v) => v
                .into_iter()
                .map(Into::into)
                .reduce(CanonRe::dis)
                .unwrap_or(CanonRe::BOTTOM),
            Regex::Not(r) => CanonRe::not((*r).into()),
            Regex::Cat(v) => v
                .into_iter()
                .map(Into::into)
                .reduce(CanonRe::cat)
                .unwrap_or(CanonRe::Nil),
            Regex::Star(r) => CanonRe::star((*r).into()),
            Regex::Lit(l) => l
                .into_iter()
                .map(|l| CanonRe::Sym(l.into()))
                .reduce(CanonRe::cat)
                .unwrap_or(CanonRe::Nil),
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use proptest::{collection::hash_set, prelude::*};

//     use super::*;
//     use crate::free::Succ;

//     fn prop_char() -> impl Strategy<Value = char> { prop::char::range('a', 'z') }

//     fn arb_re<S: Strategy + 'static>(strat: S) -> impl Strategy<Value = CanonRe<S::Value>>
//     where S::Value: Clone + Ord + Succ {
//         let leaf = prop_oneof![
//             strat.prop_map(|c| CanonRe::Sym([c.clone()..c.succ()].into_iter().collect())),
//             Just(CanonRe::Nil),
//             Just(CanonRe::BOTTOM),
//         ];

//         leaf.prop_recursive(8, 64, 2, |inner| {
//             prop_oneof![
//                 (inner.clone(), inner.clone()).prop_map(|(l, r)| l.cat(r)),
//                 inner.clone().prop_map(CanonRe::star),
//                 (inner.clone(), inner.clone()).prop_map(|(l, r)| l.con(r)),
//                 (inner.clone(), inner.clone()).prop_map(|(l, r)| l.dis(r)),
//                 inner.prop_map(CanonRe::not),
//             ]
//         })
//     }

//     proptest! {
//         #[test]
//         fn test_start_set(
//             chars in hash_set(prop_char(), 0..16),
//             re in arb_re(prop_char())
//         ) {
//             let start_set = re.start_set();

//             assert_eq!(
//                 re.matches_empty(),
//                 start_set.nil,
//                 "Nil mismatch in starting set, start_set = {start_set:?}",
//             );

//             for c in &chars {
//                 let deriv = re.clone().derivative(c);
//                 assert_eq!(
//                     deriv.matches_empty(),
//                     start_set.set.contains(c),
//                     "c = {c:?}, start_set = {start_set:?}, deriv = {deriv:?}",
//                 );
//             }
//         }
//     }
// }
