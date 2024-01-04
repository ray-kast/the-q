//! Convert a regular expression to a Graphviz DOT document

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

use shrec::re::{Regex, RegexBag};

fn main() {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Proto;

    // let re = Regex::Cat(vec![
    //     Regex::Alt(vec![
    //         Regex::Cat(vec![
    //             Regex::Lit("k".chars()),
    //             Regex::Alt(vec![Regex::Lit("i".chars()), Regex::Lit("a".chars())]),
    //             Regex::Alt(vec![Regex::Lit("m".chars()), Regex::Lit("t".chars())]),
    //         ]),
    //         Regex::Lit("ban".chars()),
    //     ]),
    //     Regex::Alt(vec![
    //         Regex::Cat(vec![
    //             Regex::Lit("o".chars()),
    //             Regex::Star(Regex::Lit("no".chars()).into()),
    //         ]),
    //         Regex::Cat(vec![
    //             Regex::Lit("a".chars()),
    //             Regex::Star(Regex::Lit("na".chars()).into()),
    //         ]),
    //     ]),
    // ]);
    // let re = shrec::re::syntax::token_re();
    let re: RegexBag<_, _> = vec![(
        Regex::Cat(vec![
            Regex::Lit("pro".chars()),
            Regex::Star(
                Regex::Cat(vec![
                    Regex::Alt(vec![Regex::Lit("".chars()), Regex::Lit("ta".chars())]),
                    Regex::Lit("to".chars()),
                ])
                .into(),
            ),
            Regex::Lit("gen".chars()),
        ]),
        Proto,
    )]
    .into();

    let non_dfa = re.compile();
    let dfa = non_dfa.compile().copied();
    let (dfa, states) = dfa.atomize_nodes::<u64>();
    eprintln!("{dfa:?}");
    eprintln!("{states:?}");

    if false {
        println!(
            "{}",
            non_dfa.dot(
                |i| format!("{i:?}").into(),
                |n| format!("{n:?}").into(),
                |()| None,
                |t| Some(format!("{t:?}").into()),
            )
        );
    } else {
        println!(
            "{}",
            dfa.dot(
                |i| format!("{i:?}").into(),
                |n| format!("{n:?}").into(),
                |()| None,
                |t| Some(format!("{t:?}").into()),
            )
        );
    }
}
