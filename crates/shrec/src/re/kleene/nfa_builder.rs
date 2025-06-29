use super::Regex;
use crate::{
    free::{Free, Succ},
    nfa::Nfa,
    partition_map::Partition,
    re::run::IntoSymbols,
};

pub struct NfaBuilder<I, E, T> {
    nfa: Nfa<I, u64, E, T>,
    free: Free<u64>,
}

impl<I: Clone + Ord + Succ, E: Clone + Ord, T: Ord> NfaBuilder<I, E, T> {
    fn new() -> Self {
        let mut free = Free::default();
        let start = free.fresh();
        let nfa = Nfa::new(start);

        Self { nfa, free }
    }

    pub fn build<B: IntoIterator<Item = (Regex<L>, T)>, L: IntoSymbols<Atom = I>>(
        tok_bag: B,
    ) -> Self {
        let mut me = Self::new();
        for (regex, tok) in tok_bag {
            let accept = me.free.fresh();
            assert!(me.nfa.insert_accept(accept, tok).is_none());
            me.build_in(regex, *me.nfa.start(), accept);
        }
        me
    }

    #[inline]
    fn fresh_node(&mut self) -> u64 {
        let fresh = self.free.fresh();
        assert!(self.nfa.insert(fresh).is_none());
        fresh
    }

    #[inline]
    fn connect(&mut self, from: u64, to: u64, by: Option<Partition<I>>, out: Option<E>) {
        assert!(self.nfa.connect(&from, to, by, out));
    }

    fn build_in<L: IntoSymbols<Atom = I>>(&mut self, regex: Regex<L>, head: u64, tail: u64) {
        match regex {
            Regex::Alt(a) => {
                for re in a {
                    let h = self.fresh_node();
                    let t = self.fresh_node();

                    self.build_in(re, h, t);
                    self.connect(head, h, None, None);
                    self.connect(t, tail, None, None);
                }
            },
            Regex::Cat(c) => {
                self.build_cat_in(c, head, tail, |s, re, h, t| {
                    s.build_in(re, h, t);
                });
            },
            Regex::Star(r) => {
                let h = self.fresh_node();
                let t = self.fresh_node();

                self.build_in(*r, h, t);
                self.connect(head, h, None, None);
                self.connect(t, tail, None, None);
                self.connect(head, tail, None, None);
                self.connect(t, h, None, None);
            },
            Regex::Lit(l) => {
                self.build_cat_in(l.into_symbols(), head, tail, |s, i, h, t| {
                    for part in i {
                        s.connect(h, t, Some(part), None);
                    }
                });
            },
        }
    }

    #[inline]
    fn build_cat_in<J: IntoIterator>(
        &mut self,
        it: J,
        head: u64,
        tail: u64,
        f: impl Fn(&mut Self, J::Item, u64, u64),
    ) {
        let mut h = head;
        let mut prev = None;
        for el in it {
            if let Some(el) = prev.replace(el) {
                let t = self.fresh_node();
                f(self, el, h, t);
                h = t;
            }
        }

        if let Some(el) = prev {
            f(self, el, h, tail);
        } else {
            self.connect(head, tail, None, None);
        }
    }

    #[inline]
    pub fn finish(self) -> Nfa<I, u64, E, T> { self.nfa }
}
