use shrec::{
    dfa::optimize,
    egraph::{
        reference,
        trace::{dot, DotTracer},
    },
    re::kleene::Regex::{Alt, Cat, Lit, Star},
};

fn main() {
    let re = Cat(vec![
        Lit(vec!['b']),
        Cat(vec![
            Alt(vec![
                Cat(vec![Cat(vec![
                    Cat(vec![Lit(vec!['u']), Star(Lit(vec!['n']).into())]),
                    Star(Cat(vec![Lit(vec!['u']), Star(Lit(vec!['n']).into())]).into()),
                ])]),
                Cat(vec![
                    Lit(vec![]),
                    Cat(vec![Lit(vec!['n']), Star(Lit(vec!['n']).into())]),
                    Star(Cat(vec![Lit(vec!['u']), Star(Lit(vec!['n']).into())]).into()),
                ]),
            ]),
            Cat(vec![
                Lit(vec![]),
                Alt(vec![Cat(vec![]), Lit(vec!['u'])]),
                Lit(vec!['y']),
            ]),
        ]),
    ]);

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
