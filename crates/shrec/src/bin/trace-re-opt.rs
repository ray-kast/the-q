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

    let mut t = DotTracer::rich();
    let (dfa, ..) = optimize::run::<_, _, _, reference::EGraph<_, _>, _>(&dfa, &mut t);

    t.flush(|dot::Snapshot { graph }| println!("{graph}"));

    println!(
        "{}",
        dfa.dot(
            |i| format!("{i:?}").into(),
            |n| format!("{n}").into(),
            |t| Some(format!("{t:?}").into()),
        )
    );
}
