use fuzz_shrec::egraph::{FastGraph, Input, SlowGraph};

fn main() {
    afl::fuzz!(
        |data: Input| data.run_differential::<SlowGraph, FastGraph<_>, _>(FastGraph::with_hasher)
    );
}
