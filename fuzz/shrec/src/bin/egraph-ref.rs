use fuzz_shrec::egraph::{run_reference, SlowGraph, Tree};

fn main() {
    afl::fuzz!(|data: (Tree, Vec<(usize, usize)>)| {
        let (nodes, merges) = data;
        let len = nodes.count();

        if len == 0 {
            return;
        }

        let merges = merges
            .into_iter()
            .map(|(a, b)| (a % len, b % len))
            .collect();

        run_reference::<SlowGraph>(&nodes, &merges);
    });
}
