use super::kleene::{Regex, RegexBag};
use crate::dfa::Dfa;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Token {
    Pipe,
    Star,
    LPar,
    RPar,
}

#[must_use]
pub fn token_re() -> RegexBag<[char; 1], Token> {
    RegexBag::from_iter([
        (Regex::Lit(['|']), Token::Pipe),
        (Regex::Lit(['*']), Token::Star),
        (Regex::Lit(['(']), Token::LPar),
        (Regex::Lit([')']), Token::RPar),
    ])
}

#[must_use]
pub fn token_dfa() -> Dfa<char, u64, Token> {
    let non_dfa = token_re().compile_atomic();
    let (dfa, _states) = non_dfa.compile().atomize_nodes::<u64>();
    dfa.try_map_token(|t| {
        let mut it = t.iter();
        it.next()
            .and_then(|f| it.next().is_none().then_some(*f))
            .ok_or(t)
    })
    .unwrap_or_else(|t| unreachable!("Found ambiguous tokens: {t:?}"))
}
