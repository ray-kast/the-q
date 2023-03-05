use super::Dfa;

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Scanner entered trap state with no accepting prefix")]
pub struct TrapError;

#[derive(Debug)]
pub struct Scanner<'a, I, N, J> {
    dfa: &'a Dfa<I, N, ()>,
    input: J,
    state: N,
    last_accept: Option<(N, J)>,
}

impl<'a, I, N: Copy + Ord, J: Clone> Scanner<'a, I, N, J> {
    #[must_use]
    pub fn new<K: IntoIterator<IntoIter = J>>(dfa: &'a Dfa<I, N, ()>, input: K) -> Self {
        let mut me = Self {
            state: dfa.start,
            dfa,
            input: input.into_iter(),
            last_accept: None,
        };
        me.set_state(dfa.start);
        me
    }

    fn set_state(&mut self, to: N) {
        self.state = to;
        if self.dfa.accept.contains(&self.state) {
            self.last_accept = Some((self.state, self.input.clone()));
        }
    }
}

impl<'a, I: Ord, N: Copy + Ord, J: Clone + Iterator<Item = I>> Iterator for Scanner<'a, I, N, J> {
    type Item = Result<N, TrapError>;

    fn next(&mut self) -> Option<Self::Item> {
        let trapped = loop {
            let Some(input) = self.input.next() else { break false };

            let Some(&(next, ())) = self
                .dfa
                .states
                .get(&self.state)
                .unwrap_or_else(|| unreachable!())
                .0
                .get(&input)
            else {
                break true;
            };

            self.set_state(next);
        };

        if let Some((state, rewind)) = self.last_accept.take() {
            self.input = rewind;
            self.set_state(self.dfa.start);
            Some(Ok(state))
        } else if trapped {
            Some(Err(TrapError))
        } else {
            None
        }
    }
}
