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
use hashbrown::{HashMap, HashSet};
use shrec::{
    egraph::{prelude::*, trace::dot::ClosureFormatter},
    re::kleene::{
        Regex::{self, Alt, Cat, Lit, Star},
        RegexBag,
    },
};

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

    // let re = Cat(vec![
    //     Alt(vec![
    //         Cat(vec![
    //             Lit("k".chars()),
    //             Alt(vec![
    //                 Lit("im".chars()),
    //                 Lit("em".chars()),
    //                 Lit("at".chars()),
    //             ]),
    //         ]),
    //         Lit("ban".chars()),
    //     ]),
    //     Alt(vec![
    //         Cat(vec![
    //             Lit("o".chars()),
    //             Star(Lit("no".chars()).into()),
    //         ]),
    //         Cat(vec![
    //             Lit("a".chars()),
    //             Star(Lit("na".chars()).into()),
    //         ]),
    //     ]),
    // ]);
    // let re = shrec::re::syntax::token_re();
    // let re = RegexBag::from_iter([
    //     (
    //         Cat(vec![
    //             Lit("pro".chars()),
    //             Star(
    //                 Cat(vec![
    //                     Alt(vec![Lit("".chars()), Lit("ta".chars())]),
    //                     Lit("to".chars()),
    //                 ])
    //                 .into(),
    //             ),
    //             Lit("gen".chars()),
    //         ]),
    //         Proto::Proto,
    //     ),
    //     (
    //         Cat(vec![
    //             Lit("p".chars()),
    //             Alt(vec![
    //                 Lit("".chars()),
    //                 Cat(vec![Lit("r".chars()), Star(Lit("o".chars()).into())]),
    //             ]),
    //             Lit("otat".chars()),
    //             Alt(vec![
    //                 Lit("".chars()),
    //                 Cat(vec![
    //                     Lit("o".chars()),
    //                     Star(Lit("to".chars()).into()),
    //                     Alt(vec![Lit("".chars()), Lit("t".chars()), Lit("gen".chars())]),
    //                 ]),
    //             ]),
    //         ]),
    //         Proto::Potato,
    //     ),
    //     (
    //         Cat(vec![
    //             Lit("proo".chars()),
    //             Star(Lit("o".chars()).into()),
    //             Lit("t".chars()),
    //         ]),
    //         Proto::Proot,
    //     ),
    // ]);
    let re = Alt(vec![
        Cat(vec![
            Lit(vec!['b']),
            Lit(vec!['n']),
            Star(Lit(vec!['n']).into()),
            Star(Lit(vec!['u']).into()),
            Lit(vec!['y']),
        ]),
        Cat(vec![
            Lit(vec!['b']),
            Alt(vec![Cat(vec![]), Lit(vec!['n'])]),
            Lit(vec!['u']),
            Star(Lit(vec!['u']).into()),
            Lit(vec!['n']),
            Star(Lit(vec!['n']).into()),
            Lit(vec!['y']),
        ]),
        Cat(vec![
            Lit(vec!['b']),
            Lit(vec!['u']),
            Star(Lit(vec!['u']).into()),
            Lit(vec!['n']),
            Star(Lit(vec!['n']).into()),
            Lit(vec!['b', 'y']),
        ]),
        Cat(vec![
            Lit(vec!['b', 'o']),
            Lit(vec!['u']),
            Star(Lit(vec!['u']).into()),
            Lit(vec!['n']),
            Star(Lit(vec!['n']).into()),
            Lit(vec!['y']),
        ]),
    ]);
    // let re = Cat(vec![
    //     Lit(vec!['b']),
    //     Cat(vec![
    //         Alt(vec![
    //             Cat(vec![Cat(vec![
    //                 Cat(vec![Lit(vec!['u']), Star(Lit(vec!['n']).into())]),
    //                 Star(Cat(vec![Lit(vec!['u']), Star(Lit(vec!['n']).into())]).into()),
    //             ])]),
    //             Cat(vec![
    //                 Lit(vec![]),
    //                 Cat(vec![Lit(vec!['n']), Star(Lit(vec!['n']).into())]),
    //                 Star(Cat(vec![Lit(vec!['u']), Star(Lit(vec!['n']).into())]).into()),
    //             ]),
    //         ]),
    //         Cat(vec![
    //             Lit(vec![]),
    //             Alt(vec![Cat(vec![]), Lit(vec!['u'])]),
    //             Lit(vec!['y']),
    //         ]),
    //     ]),
    // ]);

    let non_dfa = re.compile_atomic();

    // let cm = cm.into_iter().fold(HashMap::new(), |mut m, (k, v)| {
    //     m.entry(eg.find(v).unwrap())
    //         .or_insert_with(HashSet::new)
    //         .insert(k);
    //     m
    // });

    match output {
        Output::Nfa => println!(
            "{}",
            non_dfa.dot(
                |i| format!("{i:?}").into(),
                |n| format!("{n:?}").into(),
                |t| Some(format!("{t:?}").into()),
            )
        ),
        Output::Dfa => {
            let dfa = non_dfa.compile();
            let (dfa, _) = dfa.atomize_nodes::<u64>();
            let (dfa_opt, eg, cm) = dfa.optimize();

            println!(
                "{}",
                dfa_opt.dot(
                    |i| format!("{i:?}").into(),
                    |n| format!("{n:?}").into(),
                    |t| Some(format!("{t:?}").into()),
                )
            );
        },
        Output::DfaUnopt => {
            let dfa = non_dfa.compile();

            println!(
                "{}",
                dfa.dot(
                    |i| format!("{i:?}").into(),
                    |n| format!("{n:?}").into(),
                    |t| Some(format!("{t:?}").into()),
                )
            );
        },
        Output::Eg => {
            let dfa = non_dfa.compile();
            let (dfa, _) = dfa.atomize_nodes::<u64>();
            let (dfa_opt, eg, cm) = dfa.optimize();

            println!(
                "{}",
                eg.dot(ClosureFormatter::new(
                    |n, f| f.write_fmt(format_args!("{n:?}")),
                    |n, i, f| match n {
                        shrec::dfa::optimize::Op::Node { accept: _, edges } =>
                            f.write_fmt(format_args!("{:?}", edges.iter().nth(i).unwrap())),
                        shrec::dfa::optimize::Op::Impostor(_) => Ok(()),
                    }
                )),
            );
        },
    }
}
