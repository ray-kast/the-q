//! Print a trace of the DFA minimization for a regex

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    clippy::clone_on_ref_ptr,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]

use clap::Parser;
use shrec::{
    dfa::optimize,
    egraph::{
        fast, reference,
        trace::{dot, DotTracer},
    },
    re::kleene::{
        syntax::{pretty, scan_one},
        RegexBag,
    },
};

#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    #[arg(long, short)]
    reference: bool,

    #[arg(required = true)]
    regex: Vec<String>,
}

fn main() {
    let Opts { reference, regex } = Opts::parse();

    let re: RegexBag<_, _> = regex
        .iter()
        .enumerate()
        .flat_map(|(i, s)| scan_one(s).unwrap().into_iter().map(move |r| (r, i)))
        .collect();

    let non_dfa = re.compile();
    let (dfa, cm) = non_dfa.compile_moore();

    println!(
        "{}",
        dfa.dot(
            |s| format!("{:?}", cm[s]).into(),
            |i| pretty(i.copied()),
            |t| Some(format!("{t:?}").into()),
            |e| Some(format!("{e:?}").into()),
        )
    );

    let mut t = DotTracer::rich(|dot::Snapshot { graph }| println!("{graph}"));
    let dfa_opt;
    let cm;
    if reference {
        (dfa_opt, _, cm) = optimize::run::<_, _, _, reference::EGraph<_, _>, _>(
            &dfa,
            reference::EGraph::new(),
            &mut t,
        );
    } else {
        (dfa_opt, _, cm) =
            optimize::run::<_, _, _, fast::EGraph<_, _>, _>(&dfa, fast::EGraph::new(), &mut t);
    }

    t.flush();

    println!(
        "{}",
        dfa_opt.dot(
            |s| format!("{:?}", cm[s]).into(),
            |i| pretty(i.copied()),
            |t| Some(format!("{t:?}").into()),
            |e| Some(format!("{e:?}").into()),
        )
    );
}
