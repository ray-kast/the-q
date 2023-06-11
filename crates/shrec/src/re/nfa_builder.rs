use std::mem;

use super::Regex;
use crate::{free::Free, nfa::Nfa};

pub struct NfaBuilder<I, T> {
    nfa: Nfa<I, u64, (), T>,
    free: Free<u64>,
}

impl<I: Ord, T: Ord> NfaBuilder<I, T> {
    fn new() -> Self {
        let mut free = Free::default();
        let start = free.fresh();
        let nfa = Nfa::new(start);

        Self { nfa, free }
    }

    pub fn build<B: IntoIterator<Item = (Regex<L>, T)>, L: IntoIterator<Item = I>>(
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
    fn connect(&mut self, from: u64, to: u64, by: Option<I>) {
        assert!(self.nfa.connect(&from, to, by, ()).is_none());
    }

    fn build_in<L: IntoIterator<Item = I>>(&mut self, regex: Regex<L>, head: u64, tail: u64) {
        match regex {
            Regex::Alt(a) => {
                for re in a {
                    let h = self.fresh_node();
                    let t = self.fresh_node();

                    self.build_in(re, h, t);
                    self.connect(head, h, None);
                    self.connect(t, tail, None);
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
                self.connect(head, h, None);
                self.connect(t, tail, None);
                self.connect(head, tail, None);
                self.connect(t, h, None);
            },
            Regex::Lit(l) => {
                self.build_cat_in(l, head, tail, |s, i, h, t| s.connect(h, t, Some(i)));
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
            if let Some(el) = mem::replace(&mut prev, Some(el)) {
                let t = self.fresh_node();
                f(self, el, h, t);
                h = t;
            }
        }

        if let Some(el) = prev {
            f(self, el, h, tail);
        } else {
            self.connect(head, tail, None);
        }
    }

    #[inline]
    pub fn finish(self) -> Nfa<I, u64, (), T> { self.nfa }
}
