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

use std::io::{self, Read};

use shrec::{dfa::Scanner, re::Regex};

fn main() {
    let re = Regex::Cat(vec![
        Regex::Alt(vec![
            Regex::Cat(vec![
                Regex::Lit("k".chars()),
                Regex::Alt(vec![Regex::Lit("i".chars()), Regex::Lit("a".chars())]),
                Regex::Alt(vec![Regex::Lit("m".chars()), Regex::Lit("t".chars())]),
            ]),
            Regex::Lit("ban".chars()),
        ]),
        Regex::Alt(vec![
            Regex::Cat(vec![
                Regex::Lit("o".chars()),
                Regex::Star(Regex::Lit("no".chars()).into()),
            ]),
            Regex::Cat(vec![
                Regex::Lit("a".chars()),
                Regex::Star(Regex::Lit("na".chars()).into()),
            ]),
        ]),
    ]);

    let non_dfa = re.compile();
    let dfa = non_dfa.compile().copied();
    let (dfa, states) = dfa.atomize_nodes::<usize>();
    println!("{dfa:?}");
    println!("{states:?}");

    let mut s = String::new();
    io::stdin().read_to_string(&mut s).unwrap();
    let scanner = Scanner::new(&dfa, s.chars());

    for tok in scanner {
        println!("{tok:?}");
    }
}
