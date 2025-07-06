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

use std::io::{self, Read};

use clap::Parser;
use hashbrown::{HashMap, HashSet};
use shrec::{
    egraph::{prelude::*, trace::dot::ClosureFormatter},
    re::{
        kleene::{
            syntax::{pretty, scan_one},
            Regex::{self, Alt, Cat, Lit, Star},
            RegexBag,
        },
        run::Run,
    },
};

#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    output: Output,

    #[arg(required = true)]
    regex: Vec<String>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Output {
    Re,
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

    let Opts { output, regex } = Opts::parse();

    let regex: RegexBag<_, _> = regex
        .iter()
        .enumerate()
        .flat_map(|(i, s)| scan_one(s).unwrap().into_iter().map(move |r| (r, i)))
        .collect();

    'comp: {
        if matches!(output, Output::Re) {
            println!(
                "{}",
                regex.dot(
                    |l| match l {
                        Run::Run(s) => s.as_str().into(),
                        Run::Set(s) => s
                            .ranges()
                            .map(|p| pretty(p.copied()))
                            .collect::<Vec<_>>()
                            .join(",")
                            .into(),
                    },
                    |t| format!("{t}").into()
                )
            );
            break 'comp;
        }

        let non_dfa = regex.compile();

        if matches!(output, Output::Nfa) {
            println!(
                "{}",
                non_dfa.dot(
                    |s| format!("{s:?}").into(),
                    |i| pretty(i.copied()),
                    |t| Some(format!("{t:?}").into()),
                    |e| Some(format!("{e:?}").into()),
                )
            );
            break 'comp;
        }

        let dfa = non_dfa.compile_moore();

        if matches!(output, Output::DfaUnopt) {
            println!(
                "{}",
                dfa.dot(
                    |s| format!("{s:?}").into(),
                    |i| pretty(i.copied()),
                    |t| Some(format!("{t:?}").into()),
                    |e| Some(format!("{e:?}").into()),
                )
            );
            break 'comp;
        }

        let (dfa_opt, eg, ..) = dfa.optimize();

        match output {
            Output::Dfa => println!(
                "{}",
                dfa_opt.dot(
                    |s| format!("{s:?}").into(),
                    |i| pretty(i.copied()),
                    |t| Some(format!("{t:?}").into()),
                    |e| Some(format!("{e:?}").into()),
                )
            ),
            Output::Eg => println!(
                "{}",
                eg.dot(ClosureFormatter::new(
                    |n, f| f.write_fmt(format_args!("{n:?}")),
                    |n, i, f| match n {
                        shrec::dfa::optimize::Op::Node { accept: _, edges } =>
                            f.write_fmt(format_args!("{:?}", edges.iter().nth(i).unwrap())),
                        shrec::dfa::optimize::Op::Impostor(_) => Ok(()),
                    }
                )),
            ),
            _ => unreachable!(),
        }
    }
}
