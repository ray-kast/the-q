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

    let non_dfa = re.compile();
    let dfa = non_dfa.compile();

    println!(
        "{}",
        dfa.dot(
            |s| format!("{s}").into(),
            |i| format!("{i:?}").into(),
            |t| Some(format!("{t:?}").into()),
            |e| Some(format!("{e:?}").into()),
        )
    );

    let mut t = DotTracer::rich(|dot::Snapshot { graph }| println!("{graph}"));
    let (dfa, _, cm) = optimize::run::<_, _, _, reference::EGraph<_, _>, _>(
        &dfa,
        reference::EGraph::new(),
        &mut t,
    );

    t.flush();

    println!(
        "{}",
        dfa.dot(
            |s| format!("{:?}", cm[&s]).into(),
            |i| format!("{i:?}").into(),
            |t| Some(format!("{t:?}").into()),
            |e| Some(format!("{e:?}").into()),
        )
    );
}
