use super::Regex;
use crate::{
    free::Succ,
    nfa::{Nfa, NFA_START},
    partition_map::Partition,
    re::run::IntoSymbols,
};

pub struct NfaBuilder<I, T, E>(Nfa<I, Option<T>, E>);

impl<I: Clone + Ord + Succ, T: Ord, E: Default + Clone + Ord> NfaBuilder<I, T, E> {
    #[inline]
    fn new() -> Self { Self(Nfa::new()) }

    pub fn build<B: IntoIterator<Item = (Regex<L>, T)>, L: IntoSymbols<Atom = I>>(
        tok_bag: B,
    ) -> Self {
        let mut me = Self::new();
        for (regex, tok) in tok_bag {
            let accept = me.0.push_accept(Some(tok));
            me.build_in(regex, NFA_START, accept);
        }
        me
    }

    #[inline]
    fn connect(&mut self, from: usize, to: usize, by: Option<Partition<I>>, out: E) {
        assert!(self.0.connect(from, to, by, out));
    }

    fn build_in<L: IntoSymbols<Atom = I>>(&mut self, regex: Regex<L>, head: usize, tail: usize) {
        match regex {
            Regex::Alt(a) => {
                for re in a {
                    let h = self.0.push();
                    let t = self.0.push();

                    self.build_in(re, h, t);
                    self.connect(head, h, None, E::default());
                    self.connect(t, tail, None, E::default());
                }
            },
            Regex::Cat(c) => {
                self.build_cat_in(c, head, tail, |s, re, h, t| {
                    s.build_in(re, h, t);
                });
            },
            Regex::Star(r) => {
                let h = self.0.push();
                let t = self.0.push();

                self.build_in(*r, h, t);
                self.connect(head, h, None, E::default());
                self.connect(t, tail, None, E::default());
                self.connect(head, tail, None, E::default());
                self.connect(t, h, None, E::default());
            },
            Regex::Lit(l) => {
                self.build_cat_in(l.into_symbols(), head, tail, |s, i, h, t| {
                    for part in i {
                        s.0.connect(h, t, Some(part), E::default());
                    }
                });
            },
        }
    }

    #[inline]
    fn build_cat_in<J: IntoIterator>(
        &mut self,
        it: J,
        head: usize,
        tail: usize,
        f: impl Fn(&mut Self, J::Item, usize, usize),
    ) {
        let mut h = head;
        let mut prev = None;
        for el in it {
            if let Some(el) = prev.replace(el) {
                let t = self.0.push();
                f(self, el, h, t);
                h = t;
            }
        }

        if let Some(el) = prev {
            f(self, el, h, tail);
        } else {
            self.connect(head, tail, None, E::default());
        }
    }

    #[inline]
    pub fn finish(self) -> Nfa<I, Option<T>, E> { self.0 }
}
