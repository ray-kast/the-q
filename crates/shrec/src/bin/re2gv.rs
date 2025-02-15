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
#![allow(
    unused,
    reason = "TODO: Testing code, currently it's all hard-coded and comment-toggled"
)]

use clap::Parser;
use shrec::re::kleene::{Regex, RegexBag};

#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    output: Output,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Output {
    Nfa,
    Dfa,
    DfaUnopt,
    Eg,
}

fn main() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    enum Proto {
        #[expect(clippy::enum_variant_names)]
        Proto,
        Potato,
        Proot,
    }

    let Opts { output } = Opts::parse();

    // let re = Regex::Cat(vec![
    //     Regex::Alt(vec![
    //         Regex::Cat(vec![
    //             Regex::Lit("k".chars()),
    //             Regex::Alt(vec![
    //                 Regex::Lit("im".chars()),
    //                 Regex::Lit("em".chars()),
    //                 Regex::Lit("at".chars()),
    //             ]),
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
    let re = RegexBag::from_iter([
        (
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
            Proto::Proto,
        ),
        (
            Regex::Cat(vec![
                Regex::Lit("p".chars()),
                Regex::Alt(vec![
                    Regex::Lit("".chars()),
                    Regex::Cat(vec![
                        Regex::Lit("r".chars()),
                        Regex::Star(Regex::Lit("o".chars()).into()),
                    ]),
                ]),
                Regex::Lit("otat".chars()),
                Regex::Alt(vec![
                    Regex::Lit("".chars()),
                    Regex::Cat(vec![
                        Regex::Lit("o".chars()),
                        Regex::Star(Regex::Lit("to".chars()).into()),
                        Regex::Alt(vec![
                            Regex::Lit("".chars()),
                            Regex::Lit("t".chars()),
                            Regex::Lit("gen".chars()),
                        ]),
                    ]),
                ]),
            ]),
            Proto::Potato,
        ),
        (
            Regex::Cat(vec![
                Regex::Lit("proo".chars()),
                Regex::Star(Regex::Lit("o".chars()).into()),
                Regex::Lit("t".chars()),
            ]),
            Proto::Proot,
        ),
    ]);

    let non_dfa = re.compile_atomic();
    let dfa = non_dfa.compile();
    let (dfa, _) = dfa.atomize_nodes::<u64>();
    let (dfa_opt, eg) = dfa.optimize();

    match output {
        Output::Nfa => println!(
            "{}",
            non_dfa.dot(
                |i| format!("{i:?}").into(),
                |n| format!("{n:?}").into(),
                |t| Some(format!("{t:?}").into()),
            )
        ),
        Output::Dfa => println!(
            "{}",
            dfa_opt.dot(
                |i| format!("{i:?}").into(),
                |n| format!("{n:?}").into(),
                |t| Some(format!("{t:?}").into()),
            )
        ),
        Output::DfaUnopt => println!(
            "{}",
            dfa.dot(
                |i| format!("{i:?}").into(),
                |n| format!("{n:?}").into(),
                |t| Some(format!("{t:?}").into()),
            )
        ),
        Output::Eg => println!(
            "{}",
            eg.dot(
                |n| format!("{n:?}").into(),
                |n, i| match n {
                    shrec::dfa::optimize::Op::Node { accept, edges } =>
                        Some(format!("{:?}", edges.iter().nth(i).unwrap()).into()),
                    shrec::dfa::optimize::Op::Impostor(_) => None,
                }
            ),
        ),
    }
}
