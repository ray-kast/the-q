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

use shrec::{
    dfa::optimize,
    egraph::{
        reference,
        trace::{dot, DotTracer},
    },
    re::kleene::Regex::{Alt, Lit, Star},
};

fn main() {
    let re = Star(Alt(vec![Lit(vec!['0']), Lit(vec!['1'])]).into());

    let non_dfa = re.compile_atomic();
    let dfa = non_dfa.compile();
    let (dfa, _) = dfa.atomize_nodes::<u64>();

    println!(
        "{}",
        dfa.dot(
            |i| format!("{i:?}").into(),
            |n| format!("{n}").into(),
            |t| Some(format!("{t:?}").into()),
        )
    );

    let mut t = DotTracer::rich(|dot::Snapshot { graph }| println!("{graph}"));
    let (dfa, ..) = optimize::run::<_, _, _, reference::EGraph<_, _>, _>(
        &dfa,
        reference::EGraph::new(),
        &mut t,
    );

    t.flush();

    println!(
        "{}",
        dfa.dot(
            |i| format!("{i:?}").into(),
            |n| format!("{}", n.id()).into(),
            |t| Some(format!("{t:?}").into()),
        )
    );
}
