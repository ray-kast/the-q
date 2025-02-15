//! Convert a regular expression to a maximal-munch scanner

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    clippy::clone_on_ref_ptr,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(
    unused,
    reason = "TODO: Testing code, currently it's all hard-coded and comment-toggled"
)]

use std::io::{self, Read};

use shrec::{
    dfa::Scanner,
    re::{
        kleene::{Regex, RegexBag},
        syntax::token_dfa,
    },
};

fn main() {
    let re = RegexBag::from_iter([
        (Regex::Lit("for".chars()), "for"),
        (Regex::Lit("each".chars()), "each"),
        (Regex::Lit("ea".chars()), "ea"),
        (Regex::Lit("foreach".chars()), "foreach"),
    ]);
    // let re = Regex::Cat(vec![
    //     Regex::Lit("for".chars()),
    //     Regex::Lit("foreach".chars()),
    //     // Regex::Alt(vec![
    //     //     Regex::Cat(vec![
    //     //         Regex::Lit("k".chars()),
    //     //         Regex::Alt(vec![Regex::Lit("i".chars()), Regex::Lit("a".chars())]),
    //     //         Regex::Alt(vec![Regex::Lit("m".chars()), Regex::Lit("t".chars())]),
    //     //     ]),
    //     //     Regex::Lit("ban".chars()),
    //     // ]),
    //     // Regex::Alt(vec![
    //     //     Regex::Cat(vec![
    //     //         Regex::Lit("o".chars()),
    //     //         Regex::Star(Regex::Lit("no".chars()).into()),
    //     //     ]),
    //     //     Regex::Cat(vec![
    //     //         Regex::Lit("a".chars()),
    //     //         Regex::Star(Regex::Lit("na".chars()).into()),
    //     //     ]),
    //     // ]),
    // ]);
    // let dfa = token_dfa();
    let non_dfa = re.compile_atomic();
    let dfa = non_dfa.compile();
    let (dfa, states) = dfa.atomize_nodes::<u64>();

    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let scanner = Scanner::new(&dfa, s.chars());

    for tok in scanner {
        println!("{tok:?}");
    }
}
